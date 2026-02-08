//! # SkillVectorRouter
//!
//! 基于向量的技能路由系统，支持意图到技能的智能匹配。
//!
//! ## 功能特性
//!
//! - 基于向量相似度的技能路由
//! - 全局技能注册
//! - 技能链自动检测与执行
//! - 参数提取
//! - 技能兼容性自动发现
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::skill::router::SkillVectorRouter;
//! use cis_core::skill::SkillManager;
//! use cis_core::vector::VectorStorage;
//! use cis_core::storage::db::DbManager;
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let storage = Arc::new(VectorStorage::open_default()?);
//! let embedding = storage.embedding_service().clone();
//! let db_manager = Arc::new(DbManager::new()?);
//! let skill_manager = Arc::new(SkillManager::new(db_manager.clone())?);
//! let router = SkillVectorRouter::new(storage, embedding, skill_manager, db_manager);
//!
//! // 路由用户意图到技能
//! let result = router.route_by_intent("分析今天的销售数据").await?;
//! println!("最佳匹配: {} (置信度: {:.2})", 
//!     result.primary_skill.skill_name, 
//!     result.overall_confidence
//! );
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::ai::embedding::EmbeddingService;
use crate::error::{CisError, Result};
use crate::intent::{ActionType, EntityValue, ParsedIntent};
use crate::scheduler::{DagScheduler, SkillDagExecutor};
use crate::skill::chain::{SkillChain, SkillCompatibilityRecord};
use crate::skill::compatibility_db::SkillCompatibilityDb;
use crate::skill::SkillManager;
use crate::storage::db::DbManager;
use crate::vector::storage::{SkillMatch, VectorStorage};

use super::semantics::SkillSemanticsExt;

/// 路由结果
#[derive(Debug, Clone)]
pub struct RouteResult {
    /// 技能ID
    pub skill_id: String,
    /// 技能名称
    pub skill_name: String,
    /// 置信度
    pub confidence: f32,
    /// 提取的参数
    pub extracted_params: serde_json::Value,
    /// 建议的技能链
    pub suggested_chain: Option<Vec<String>>,
}

/// 技能路由结果（包含链式执行信息）
#[derive(Debug, Clone)]
pub struct SkillRoutingResult {
    /// 主技能
    pub primary_skill: RouteResult,
    /// 发现的技能链
    pub skill_chain: Option<SkillChain>,
    /// 是否需要链式执行
    pub requires_chain: bool,
    /// 总体置信度
    pub overall_confidence: f32,
}

/// 解析后的参数
#[derive(Debug, Clone)]
pub struct ResolvedParameters {
    /// 初始参数
    pub initial: Value,
    /// 步骤间参数映射
    pub step_mappings: HashMap<usize, HashMap<String, String>>,
}

impl ResolvedParameters {
    /// 创建新的解析参数
    pub fn new(initial: Value) -> Self {
        Self {
            initial,
            step_mappings: HashMap::new(),
        }
    }

    /// 添加步骤映射
    pub fn with_mapping(mut self, step_idx: usize, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.step_mappings
            .entry(step_idx)
            .or_default()
            .insert(from.into(), to.into());
        self
    }
}

/// 链执行结果
#[derive(Debug, Clone)]
pub struct ChainExecutionResult {
    /// 执行步骤结果
    pub step_results: Vec<ChainStepResult>,
    /// 最终结果
    pub final_output: Value,
    /// 是否全部成功
    pub all_succeeded: bool,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
}

/// 链步骤执行结果
#[derive(Debug, Clone)]
pub struct ChainStepResult {
    /// 步骤索引
    pub step_index: usize,
    /// 技能ID
    pub skill_id: String,
    /// 输出
    pub output: Value,
    /// 是否成功
    pub success: bool,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// Skill 兼容性记录
#[derive(Debug, Clone)]
pub struct SkillCompatibility {
    /// 源技能ID
    pub source_skill_id: String,
    /// 目标技能ID
    pub target_skill_id: String,
    /// 兼容性评分 (0.0 - 1.0)
    pub compatibility_score: f32,
    /// 支持的数据流类型
    pub data_flow_types: Vec<String>,
    /// 发现时间
    pub discovered_at: i64,
}

/// 技能向量路由器
///
/// 基于向量相似度进行技能路由，支持意图解析、技能匹配和链式执行。
///
/// ## 使用场景
///
/// - 自然语言调用技能
/// - 技能链发现和执行
/// - 技能兼容性分析
///
/// ## 示例
///
/// ```rust,no_run
/// use cis_core::skill::router::SkillVectorRouter;
/// use cis_core::skill::semantics::SkillSemanticsExt;
/// use cis_core::skill::SkillManager;
/// use cis_core::vector::VectorStorage;
/// use cis_core::storage::db::DbManager;
/// use std::sync::Arc;
///
/// # async fn example() -> anyhow::Result<()> {
/// let storage = Arc::new(VectorStorage::open_default()?);
/// let embedding = storage.embedding_service().clone();
/// let db_manager = Arc::new(DbManager::new()?);
/// let skill_manager = Arc::new(SkillManager::new(db_manager.clone())?);
/// let mut router = SkillVectorRouter::new(storage, embedding, skill_manager, db_manager);
///
/// // 注册全局技能
/// let semantics = SkillSemanticsExt::new("data-analyzer", "Data Analyzer")
///     .with_description("分析数据并生成报告");
/// router.register_global_skill(semantics);
///
/// // 执行路由
/// let result = router.route_by_intent("分析销售数据").await?;
/// # Ok(())
/// # }
/// ```
pub struct SkillVectorRouter {
    /// 向量存储
    storage: Arc<VectorStorage>,
    /// 嵌入服务
    embedding_service: Arc<dyn EmbeddingService>,
    /// 全局技能列表
    global_skills: Vec<SkillSemanticsExt>,
    /// 技能兼容性缓存
    compatibility_cache: HashMap<(String, String), SkillCompatibility>,
    /// Skill 管理器（用于执行）
    skill_manager: Arc<SkillManager>,
    /// 数据库管理器
    db_manager: Arc<DbManager>,
}

impl SkillVectorRouter {
    /// 创建新的技能向量路由器
    ///
    /// # 参数
    /// - `storage`: 向量存储实例
    /// - `embedding`: 嵌入服务
    /// - `skill_manager`: Skill 管理器
    /// - `db_manager`: 数据库管理器
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::skill::router::SkillVectorRouter;
    /// use cis_core::skill::SkillManager;
    /// use cis_core::vector::VectorStorage;
    /// use cis_core::storage::db::DbManager;
    /// use std::sync::Arc;
    ///
    /// # fn example() -> anyhow::Result<()> {
    /// let storage = Arc::new(VectorStorage::open_default()?);
    /// let embedding = storage.embedding_service().clone();
    /// let db_manager = Arc::new(DbManager::new()?);
    /// let skill_manager = Arc::new(SkillManager::new(db_manager.clone())?);
    /// let router = SkillVectorRouter::new(storage, embedding, skill_manager, db_manager);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        storage: Arc<VectorStorage>,
        embedding: Arc<dyn EmbeddingService>,
        skill_manager: Arc<SkillManager>,
        db_manager: Arc<DbManager>,
    ) -> Self {
        Self {
            storage,
            embedding_service: embedding,
            global_skills: Vec::new(),
            compatibility_cache: HashMap::new(),
            skill_manager,
            db_manager,
        }
    }

    /// 注册全局技能
    pub fn register_global_skill(&mut self, semantics: SkillSemanticsExt) {
        self.global_skills.push(semantics);
    }

    /// 自然语言调用 Skill (核心方法)
    ///
    /// 根据用户输入自动路由到最佳技能，并发现和执行技能链。
    ///
    /// # 参数
    /// - `user_input`: 用户输入的自然语言描述
    ///
    /// # 返回
    /// - `Result<SkillRoutingResult>`: 包含路由结果、技能链和置信度
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::skill::router::SkillVectorRouter;
    ///
    /// # async fn example(router: &SkillVectorRouter) -> anyhow::Result<()> {
    /// let result = router.route_by_intent("分析今天的销售数据并生成报告").await?;
    ///
    /// println!("匹配技能: {}", result.primary_skill.skill_name);
    /// println!("置信度: {:.2}", result.overall_confidence);
    ///
    /// if result.requires_chain {
    ///     println!("需要链式执行");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn route_by_intent(&self, user_input: &str) -> Result<SkillRoutingResult> {
        let start_time = std::time::Instant::now();
        
        // 1. 解析用户意图
        let parsed_intent = self.parse_intent(user_input).await?;
        
        // 2. 路由到最佳技能
        let route_results = self.route(&parsed_intent, None).await?;
        
        if route_results.is_empty() {
            return Err(CisError::skill("No matching skill found"));
        }
        
        let primary_skill = route_results[0].clone();
        
        // 3. 发现技能链
        let skill_chain = self.discover_skill_chain(&primary_skill.skill_id, &parsed_intent).await?;
        let requires_chain = skill_chain.steps().len() > 1;
        
        // 4. 计算总体置信度
        let overall_confidence = if requires_chain {
            // 链式执行的置信度需要考虑所有步骤
            let chain_confidence = self.calculate_chain_confidence(&skill_chain).await?;
            (primary_skill.confidence + chain_confidence) / 2.0
        } else {
            primary_skill.confidence
        };
        
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        
        tracing::info!(
            "Routed '{}' to skill '{}' with confidence {} (chain: {}, time: {}ms)",
            user_input,
            primary_skill.skill_id,
            overall_confidence,
            requires_chain,
            execution_time_ms
        );
        
        Ok(SkillRoutingResult {
            primary_skill,
            skill_chain: Some(skill_chain),
            requires_chain,
            overall_confidence,
        })
    }

    /// 基于意图路由到最佳技能
    pub async fn route(
        &self,
        intent: &ParsedIntent,
        project: Option<&str>,
    ) -> Result<Vec<RouteResult>> {
        let mut candidates = Vec::new();

        // 1. 搜索向量存储中的技能
        let storage_results = self
            .storage
            .search_skills(&intent.normalized_intent, project, 5, Some(0.6))
            .await?;

        for sr in storage_results {
            candidates.push(RouteResult {
                skill_id: sr.skill_id.clone(),
                skill_name: sr.skill_name,
                confidence: sr.combined_score * intent.confidence,
                extracted_params: self.extract_params(intent, &sr.skill_id).await?,
                suggested_chain: None,
            });
        }

        // 2. 检查全局技能
        for global in &self.global_skills {
            // 计算与全局技能的相似度
            let intent_text = intent.normalized_intent.clone();
            let skill_text = global.to_intent_description();
            let similarity = self.calculate_similarity(&intent_text, &skill_text).await?;

            if similarity > 0.6 {
                candidates.push(RouteResult {
                    skill_id: global.skill_id.clone(),
                    skill_name: global.skill_name.clone(),
                    confidence: similarity * intent.confidence,
                    extracted_params: self.extract_params_from_semantic(intent, global).await?,
                    suggested_chain: self.detect_chain(intent, global).await.ok().flatten(),
                });
            }
        }

        // 按置信度排序
        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        Ok(candidates)
    }

    /// 发现 Skill 链 (多步编排)
    ///
    /// 根据主技能和解析后的意图，自动发现可链式执行的后续技能。
    ///
    /// # 参数
    /// - `primary_skill_id`: 主技能 ID
    /// - `parsed_intent`: 解析后的意图
    ///
    /// # 返回
    /// - `Result<SkillChain>`: 发现的技能链
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::skill::router::SkillVectorRouter;
    /// use cis_core::intent::ParsedIntent;
    ///
    /// # async fn example(router: &SkillVectorRouter, intent: &ParsedIntent) -> anyhow::Result<()> {
    /// let chain = router.discover_skill_chain("data-analyzer", intent).await?;
    ///
    /// println!("发现 {} 个步骤", chain.steps().len());
    /// for (i, step) in chain.steps().iter().enumerate() {
    ///     println!("{}: {}", i + 1, step.skill_id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn discover_skill_chain(
        &self,
        primary_skill_id: &str,
        parsed_intent: &ParsedIntent,
    ) -> Result<SkillChain> {
        let mut chain = SkillChain::new(self.build_initial_input(parsed_intent)?);
        
        // 1. 获取主技能的 IO 签名
        let primary_skill = self.find_skill_semantics(primary_skill_id).await?;
        
        // 2. 添加主技能作为第一步
        chain.add_step(primary_skill_id.to_string());
        
        // 3. 如果主技能已经是 sink，无需链式
        if let Some(io) = &primary_skill.io_signature {
            if io.sink {
                tracing::debug!("Primary skill {} is a sink, no chaining needed", primary_skill_id);
                return Ok(chain);
            }
        }
        
        // 4. 查找兼容的后续 Skills
        let compatible_skills = self.find_compatible_skills(primary_skill_id).await?;
        
        // 5. 根据意图匹配度排序
        let mut scored_skills = Vec::new();
        for skill in compatible_skills {
            let intent_text = parsed_intent.normalized_intent.clone();
            let skill_text = skill.to_intent_description();
            let similarity = self.calculate_similarity(&intent_text, &skill_text).await?;
            
            // 检查缓存中的兼容性评分
            let compat_score = self.get_compatibility_score(primary_skill_id, &skill.skill_id).await;
            
            let combined_score = (similarity + compat_score) / 2.0;
            scored_skills.push((skill, combined_score));
        }
        
        // 按综合评分排序
        scored_skills.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        // 6. 构建 Chain - 添加最匹配的后续技能
        for (skill, score) in scored_skills.iter().take(2) {  // 最多链式2个后续技能
            if *score > 0.5 {
                chain.add_step(skill.skill_id.clone());
                
                // 设置输入输出映射
                if let Some((from, to)) = self.infer_io_mapping(&primary_skill, skill).await? {
                    let step_idx = chain.steps().len() - 1;
                    chain.add_input_mapping(step_idx, from, to);
                }
            }
        }
        
        tracing::info!(
            "Discovered skill chain for {}: {} steps",
            primary_skill_id,
            chain.steps().len()
        );
        
        Ok(chain)
    }

    /// 执行 Skill 链
    ///
    /// 执行发现的技能链，按顺序调用每个技能并传递参数。
    ///
    /// # 参数
    /// - `chain`: 要执行的技能链
    /// - `params`: 解析后的参数
    ///
    /// # 返回
    /// - `Result<ChainExecutionResult>`: 执行结果
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::skill::router::{SkillVectorRouter, ResolvedParameters};
    /// use cis_core::skill::chain::SkillChain;
    ///
    /// # async fn example(
    /// #     router: &SkillVectorRouter,
    /// #     chain: &SkillChain
    /// # ) -> anyhow::Result<()> {
    /// let params = ResolvedParameters::new(serde_json::json!({
    ///     "input": "sales_data.csv"
    /// }));
    ///
    /// let result = router.execute_chain(chain, &params).await?;
    ///
    /// if result.all_succeeded {
    ///     println!("执行成功: {}", result.final_output);
    /// } else {
    ///     println!("执行失败，步骤: {}", result.step_results.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_chain(
        &self,
        chain: &SkillChain,
        params: &ResolvedParameters,
    ) -> Result<ChainExecutionResult> {
        let start_time = std::time::Instant::now();
        let mut step_results = Vec::new();
        let mut all_succeeded = true;
        let mut current_input = params.initial.clone();
        
        // 创建 DAG 执行器
        let scheduler = DagScheduler::new();
        let mut executor = SkillDagExecutor::new(scheduler, self.skill_manager.clone());
        
        for (idx, step) in chain.steps().iter().enumerate() {
            // 应用参数映射
            if let Some(mappings) = params.step_mappings.get(&idx) {
                for (from, to) in mappings {
                    if let Some(value) = current_input.get(from) {
                        current_input[to] = value.clone();
                    }
                }
            }
            
            // 执行技能
            match executor.execute_skill(&step.skill_id, current_input.clone()).await {
                Ok(result) => {
                    if result.success {
                        step_results.push(ChainStepResult {
                            step_index: idx,
                            skill_id: step.skill_id.clone(),
                            output: result.output.clone().unwrap_or_else(|| serde_json::json!({})),
                            success: true,
                            error: None,
                        });
                        
                        // 输出作为下一步的输入
                        current_input = result.output.clone().unwrap_or_else(|| serde_json::json!({}));
                    } else {
                        step_results.push(ChainStepResult {
                            step_index: idx,
                            skill_id: step.skill_id.clone(),
                            output: serde_json::json!({}),
                            success: false,
                            error: result.error,
                        });
                        all_succeeded = false;
                        
                        // 链式失败处理：中断执行
                        tracing::warn!("Chain execution failed at step {}: {:?}", idx, step.skill_id);
                        break;
                    }
                }
                Err(e) => {
                    step_results.push(ChainStepResult {
                        step_index: idx,
                        skill_id: step.skill_id.clone(),
                        output: serde_json::json!({}),
                        success: false,
                        error: Some(e.to_string()),
                    });
                    all_succeeded = false;
                    
                    // 链式失败处理：中断执行
                    tracing::warn!("Chain execution failed at step {}: {}", idx, e);
                    break;
                }
            }
        }
        
        let final_output = step_results
            .last()
            .filter(|r| r.success)
            .map(|r| r.output.clone())
            .unwrap_or_else(|| serde_json::json!({}));
        
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        
        tracing::info!(
            "Chain execution completed: {} steps, success: {}, time: {}ms",
            step_results.len(),
            all_succeeded,
            execution_time_ms
        );
        
        Ok(ChainExecutionResult {
            step_results,
            final_output,
            all_succeeded,
            execution_time_ms,
        })
    }

    /// 自动发现 Skill 兼容性 (后台任务)
    ///
    /// 分析所有已注册技能的 IO 签名，自动发现潜在的链式组合
    pub async fn auto_discover_compatibility(&self) -> Result<()> {
        tracing::info!("Starting automatic skill compatibility discovery...");
        
        let mut discovered_count = 0;
        
        // 1. 获取所有技能
        let all_skills: Vec<SkillSemanticsExt> = self.global_skills.clone();
        
        // 2. 两两比较检查兼容性
        for (i, source) in all_skills.iter().enumerate() {
            for (j, target) in all_skills.iter().enumerate() {
                if i == j {
                    continue; // 跳过自身
                }
                
                // 检查 IO 兼容性
                if let Some((score, flow_types)) = self.check_io_compatibility(source, target).await? {
                    if score > 0.5 {
                        // 添加到数据库
                        self.save_compatibility(
                            &source.skill_id,
                            &target.skill_id,
                            score,
                            flow_types,
                        ).await?;
                        
                        discovered_count += 1;
                        
                        tracing::debug!(
                            "Discovered compatibility: {} -> {} (score: {:.2})",
                            source.skill_id,
                            target.skill_id,
                            score
                        );
                    }
                }
            }
        }
        
        // 3. 从向量存储中也检查技能兼容性
        // 这里可以通过搜索获取更多的技能信息
        
        tracing::info!(
            "Compatibility discovery completed: {} new compatibilities found",
            discovered_count
        );
        
        Ok(())
    }

    /// 计算两个文本的相似度
    async fn calculate_similarity(&self, text1: &str, text2: &str) -> Result<f32> {
        let emb1 = self.embedding_service.embed(text1).await?;
        let emb2 = self.embedding_service.embed(text2).await?;

        // 余弦相似度
        let dot: f32 = emb1.iter().zip(emb2.iter()).map(|(a, b)| a * b).sum();
        let norm1: f32 = emb1.iter().map(|a| a * a).sum::<f32>().sqrt();
        let norm2: f32 = emb2.iter().map(|a| a * a).sum::<f32>().sqrt();

        if norm1 > 0.0 && norm2 > 0.0 {
            Ok(dot / (norm1 * norm2))
        } else {
            Ok(0.0)
        }
    }

    /// 从意图实体提取参数
    async fn extract_params(&self, intent: &ParsedIntent, _skill_id: &str) -> Result<serde_json::Value> {
        let mut params = serde_json::Map::new();

        for (key, value) in &intent.entities {
            let json_value = match value {
                EntityValue::String(s) => serde_json::Value::String(s.clone()),
                EntityValue::Number(n) => serde_json::Value::Number(
                    serde_json::Number::from_f64(*n).unwrap_or(0.into()),
                ),
                EntityValue::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
                EntityValue::FilePath(p) => {
                    serde_json::Value::String(p.to_string_lossy().to_string())
                }
                EntityValue::List(list) => {
                    // 递归转换列表中的值
                    serde_json::Value::Array(
                        list.iter()
                            .map(|v| match v {
                                EntityValue::String(s) => serde_json::Value::String(s.clone()),
                                EntityValue::Number(n) => serde_json::Value::Number(
                                    serde_json::Number::from_f64(*n).unwrap_or(0.into()),
                                ),
                                EntityValue::DateTime(dt) => serde_json::Value::String(dt.to_rfc3339()),
                                EntityValue::FilePath(p) => {
                                    serde_json::Value::String(p.to_string_lossy().to_string())
                                }
                                EntityValue::List(_) => serde_json::Value::Null, // 简化处理，不支持嵌套列表
                            })
                            .collect()
                    )
                }
            };
            params.insert(key.clone(), json_value);
        }

        Ok(serde_json::Value::Object(params))
    }

    /// 从语义描述提取参数
    async fn extract_params_from_semantic(
        &self,
        intent: &ParsedIntent,
        _semantic: &SkillSemanticsExt,
    ) -> Result<serde_json::Value> {
        self.extract_params(intent, &_semantic.skill_id).await
    }

    /// 检测可能的技能链
    async fn detect_chain(
        &self,
        intent: &ParsedIntent,
        semantic: &SkillSemanticsExt,
    ) -> Result<Option<Vec<String>>> {
        // 根据action_type和io_signature建议链
        if let Some(io) = &semantic.io_signature {
            if io.sink {
                // 这是一个sink，建议前面的source
                return Ok(Some(vec![
                    "cis-local:file-list".to_string(),
                    semantic.skill_id.clone(),
                ]));
            }
        }

        match intent.action_type {
            ActionType::Analyze => {
                // 分析通常需要先生成或读取
                Ok(Some(vec![
                    "cis-local:read".to_string(),
                    semantic.skill_id.clone(),
                ]))
            }
            ActionType::Commit => Ok(Some(vec![
                "cis-local:analyze".to_string(),
                "cis-local:commit".to_string(),
            ])),
            _ => Ok(None),
        }
    }

    /// 解析用户意图（简化版）
    async fn parse_intent(&self, user_input: &str) -> Result<ParsedIntent> {
        // 使用 embedding 服务解析意图
        let embedding = self.embedding_service.embed(user_input).await?;
        
        // 简单的实体提取
        let mut entities = HashMap::new();
        
        // 检测时间相关实体
        let input_lower = user_input.to_lowercase();
        if input_lower.contains("今天") || input_lower.contains("today") {
            entities.insert("time".to_string(), EntityValue::DateTime(chrono::Utc::now()));
        }
        
        // 检测文件路径
        let extensions = [".csv", ".json", ".pdf", ".txt", ".md", ".rs", ".toml", ".py", ".js", ".ts"];
        for ext in &extensions {
            if let Some(pos) = user_input.find(ext) {
                let start = user_input[..pos].rfind(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let end = pos + ext.len();
                let path = &user_input[start..end];
                if !path.is_empty() {
                    entities.insert("file".to_string(), EntityValue::FilePath(std::path::PathBuf::from(path)));
                }
            }
        }
        
        // 动作分类
        let action_type = self.classify_action(user_input);
        
        // 计算置信度
        let confidence = 0.8f32.min(0.5 + (entities.len() as f32 * 0.1));
        
        Ok(ParsedIntent {
            raw_input: user_input.to_string(),
            normalized_intent: user_input.to_string(),
            embedding,
            entities,
            confidence,
            action_type,
        })
    }

    /// 动作分类
    fn classify_action(&self, input: &str) -> ActionType {
        let input_lower = input.to_lowercase();
        
        if input_lower.contains("分析") || input_lower.contains("analyze") {
            ActionType::Analyze
        } else if input_lower.contains("创建") || input_lower.contains("create") || input_lower.contains("generate") {
            ActionType::Create
        } else if input_lower.contains("更新") || input_lower.contains("update") {
            ActionType::Update
        } else if input_lower.contains("删除") || input_lower.contains("delete") || input_lower.contains("remove") {
            ActionType::Delete
        } else if input_lower.contains("提交") || input_lower.contains("commit") {
            ActionType::Commit
        } else if input_lower.contains("查询") || input_lower.contains("search") || input_lower.contains("find") || input_lower.contains("query") {
            ActionType::Query
        } else {
            ActionType::Other
        }
    }

    /// 查找技能语义描述
    async fn find_skill_semantics(&self, skill_id: &str) -> Result<SkillSemanticsExt> {
        // 首先在全局技能中查找
        if let Some(skill) = self.global_skills.iter().find(|s| s.skill_id == skill_id) {
            return Ok(skill.clone());
        }
        
        Err(CisError::skill_not_found(format!("Skill not found: {}", skill_id)))
    }

    /// 查找兼容的技能
    async fn find_compatible_skills(&self, source_skill_id: &str) -> Result<Vec<SkillSemanticsExt>> {
        let source = self.find_skill_semantics(source_skill_id).await?;
        let mut compatible = Vec::new();
        
        for skill in &self.global_skills {
            if skill.skill_id == source_skill_id {
                continue;
            }
            
            // 检查 IO 兼容性
            if let Some((score, _)) = self.check_io_compatibility(&source, skill).await? {
                if score > 0.5 {
                    compatible.push(skill.clone());
                }
            }
        }
        
        Ok(compatible)
    }

    /// 检查 IO 兼容性
    async fn check_io_compatibility(
        &self,
        source: &SkillSemanticsExt,
        target: &SkillSemanticsExt,
    ) -> Result<Option<(f32, Vec<String>)>> {
        let source_io = match &source.io_signature {
            Some(io) => io,
            None => return Ok(None),
        };
        
        let target_io = match &target.io_signature {
            Some(io) => io,
            None => return Ok(None),
        };
        
        // 源必须有输出，目标必须有输入
        if source_io.output_types.is_empty() || target_io.input_types.is_empty() {
            return Ok(None);
        }
        
        // 检查类型兼容性
        let mut compatible_types = Vec::new();
        let mut total_score = 0.0f32;
        
        for output_type in &source_io.output_types {
            for input_type in &target_io.input_types {
                if self.is_type_compatible(output_type, input_type) {
                    compatible_types.push(format!("{}->{}", output_type, input_type));
                    total_score += 1.0;
                }
            }
        }
        
        if compatible_types.is_empty() {
            return Ok(None);
        }
        
        // 计算兼容性评分
        let max_possible = (source_io.output_types.len() * target_io.input_types.len()) as f32;
        let score = (total_score / max_possible).min(1.0);
        
        // 考虑 pipeable 属性
        let final_score = if source_io.pipeable && target_io.pipeable {
            score * 1.2 // 可管道连接的技能给予加分
        } else {
            score
        }.min(1.0);
        
        Ok(Some((final_score, compatible_types)))
    }

    /// 检查类型兼容性
    fn is_type_compatible(&self, output_type: &str, input_type: &str) -> bool {
        // 完全匹配
        if output_type == input_type {
            return true;
        }
        
        // 常见类型转换
        let compatible_pairs: Vec<(&str, &str)> = vec![
            ("json", "object"),
            ("object", "json"),
            ("text", "string"),
            ("string", "text"),
            ("csv", "table"),
            ("table", "csv"),
            ("file", "path"),
            ("path", "file"),
        ];
        
        compatible_pairs.iter().any(|(out, inp)| {
            (output_type.eq_ignore_ascii_case(out) && input_type.eq_ignore_ascii_case(inp)) ||
            (output_type.eq_ignore_ascii_case(inp) && input_type.eq_ignore_ascii_case(out))
        })
    }

    /// 推断 IO 映射
    async fn infer_io_mapping(
        &self,
        source: &SkillSemanticsExt,
        target: &SkillSemanticsExt,
    ) -> Result<Option<(String, String)>> {
        let source_io = match &source.io_signature {
            Some(io) => io,
            None => return Ok(None),
        };
        
        let target_io = match &target.io_signature {
            Some(io) => io,
            None => return Ok(None),
        };
        
        // 找到第一个兼容的类型对
        for output_type in &source_io.output_types {
            for input_type in &target_io.input_types {
                if self.is_type_compatible(output_type, input_type) {
                    return Ok(Some((
                        format!("output_{}", output_type),
                        format!("input_{}", input_type),
                    )));
                }
            }
        }
        
        Ok(None)
    }

    /// 获取兼容性评分
    async fn get_compatibility_score(&self, source_id: &str, target_id: &str) -> f32 {
        self.compatibility_cache
            .get(&(source_id.to_string(), target_id.to_string()))
            .map(|c| c.compatibility_score)
            .unwrap_or(0.5) // 默认中等兼容性
    }

    /// 保存兼容性信息
    async fn save_compatibility(
        &self,
        source_id: &str,
        target_id: &str,
        score: f32,
        flow_types: Vec<String>,
    ) -> Result<()> {
        let compatibility = SkillCompatibility {
            source_skill_id: source_id.to_string(),
            target_skill_id: target_id.to_string(),
            compatibility_score: score,
            data_flow_types: flow_types.clone(),
            discovered_at: chrono::Utc::now().timestamp(),
        };
        
        // 保存到内存缓存
        let cache_key = (source_id.to_string(), target_id.to_string());
        let mut cache = self.compatibility_cache.clone();
        cache.insert(cache_key, compatibility.clone());
        
        // 保存到数据库 skill_compatibility 表
        let record = SkillCompatibilityRecord {
            source_skill_id: source_id.to_string(),
            target_skill_id: target_id.to_string(),
            compatibility_score: score as f64,
            data_flow_types: serde_json::to_string(&flow_types)
                .unwrap_or_else(|_| "[]".to_string()),
            discovered_at: chrono::Utc::now().timestamp(),
        };
        
        // 使用 DbManager 获取核心数据库连接
        let core_db = self.db_manager.core();
        if let Ok(guard) = core_db.lock() {
            let compat_db = SkillCompatibilityDb::new(guard.conn());
            if let Err(e) = compat_db.init_table() {
                tracing::warn!("Failed to init skill_compatibility table: {}", e);
            }
            if let Err(e) = compat_db.upsert(&record) {
                tracing::warn!("Failed to save compatibility to database: {}", e);
            } else {
                tracing::debug!(
                    "Saved compatibility to database: {} -> {} (score: {:.2})",
                    source_id, target_id, score
                );
            }
        }
        
        tracing::debug!(
            "Saved compatibility: {} -> {} (score: {:.2})",
            source_id,
            target_id,
            score
        );
        
        Ok(())
    }

    /// 计算链式执行置信度
    async fn calculate_chain_confidence(&self, chain: &SkillChain) -> Result<f32> {
        let steps = chain.steps();
        if steps.len() <= 1 {
            return Ok(1.0);
        }
        
        let mut total_confidence = 1.0f32;
        
        for i in 0..steps.len() - 1 {
            let current = &steps[i].skill_id;
            let next = &steps[i + 1].skill_id;
            
            let compat_score = self.get_compatibility_score(current, next).await;
            total_confidence *= compat_score;
        }
        
        // 考虑链长度惩罚
        let length_penalty = 1.0 - (steps.len() as f32 - 1.0) * 0.1;
        
        Ok(total_confidence * length_penalty.max(0.5))
    }

    /// 构建初始输入
    fn build_initial_input(&self, parsed_intent: &ParsedIntent) -> Result<Value> {
        let mut input = serde_json::Map::new();
        
        // 添加原始输入
        input.insert("query".to_string(), Value::String(parsed_intent.raw_input.clone()));
        input.insert("action".to_string(), Value::String(format!("{:?}", parsed_intent.action_type)));
        
        // 添加实体
        for (key, value) in &parsed_intent.entities {
            let json_value = match value {
                EntityValue::String(s) => Value::String(s.clone()),
                EntityValue::Number(n) => serde_json::Number::from_f64(*n)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
                EntityValue::DateTime(dt) => Value::String(dt.to_rfc3339()),
                EntityValue::FilePath(p) => Value::String(p.to_string_lossy().to_string()),
                EntityValue::List(_) => Value::Array(vec![]),
            };
            input.insert(key.clone(), json_value);
        }
        
        Ok(Value::Object(input))
    }

    /// 执行单个技能（使用 SkillDagExecutor）
    ///
    /// 注意：此方法会在每次调用时创建新的执行器。
    /// 如需复用执行器上下文，请直接使用 SkillDagExecutor。
    async fn execute_skill(&self, skill_id: &str, input: Value) -> Result<Value> {
        let scheduler = DagScheduler::new();
        let mut executor = SkillDagExecutor::new(scheduler, self.skill_manager.clone());
        
        let result = executor.execute_skill(skill_id, input).await?;
        
        if result.success {
            Ok(result.output.unwrap_or_else(|| serde_json::json!({})))
        } else {
            Err(CisError::skill(
                result.error.unwrap_or_else(|| "Skill execution failed".to_string())
            ))
        }
    }
}

/// 存储结果包装（用于兼容性）
impl From<SkillMatch> for RouteResult {
    fn from(m: SkillMatch) -> Self {
        Self {
            skill_id: m.skill_id,
            skill_name: m.skill_name,
            confidence: m.combined_score,
            extracted_params: serde_json::Value::Object(serde_json::Map::new()),
            suggested_chain: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
    use crate::skill::semantics::SkillIoSignature;
    use async_trait::async_trait;

    /// 模拟 embedding service（用于测试）
    struct MockEmbeddingService;

    #[async_trait]
    impl EmbeddingService for MockEmbeddingService {
        async fn embed(&self, text: &str) -> Result<Vec<f32>> {
            // 基于单词的向量生成：共享单词的文本会产生相似向量
            let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            let text_lower = text.to_lowercase();
            let words: Vec<&str> = text_lower.split_whitespace().collect();
            
            for word in &words {
                let hash = word.bytes().fold(0u64, |acc, b| {
                    acc.wrapping_mul(31).wrapping_add(b as u64)
                });
                // 每个单词贡献一部分向量
                for i in 0..DEFAULT_EMBEDDING_DIM.min(128) {
                    let idx = i + (hash as usize % (DEFAULT_EMBEDDING_DIM - 128));
                    vec[idx] += ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
                }
            }
            
            // 如果没有单词，使用文本哈希作为后备
            if words.is_empty() {
                let hash = text.bytes().fold(0u64, |acc, b| {
                    acc.wrapping_mul(31).wrapping_add(b as u64)
                });
                for i in 0..DEFAULT_EMBEDDING_DIM {
                    vec[i] = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
                }
            }
            
            // 归一化
            let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut vec {
                    *x /= norm;
                }
            }
            Ok(vec)
        }

        async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                results.push(self.embed(text).await?);
            }
            Ok(results)
        }
    }

    fn create_test_skill(id: &str, name: &str, input_types: Vec<&str>, output_types: Vec<&str>, sink: bool) -> SkillSemanticsExt {
        SkillSemanticsExt::new(id, name)
            .with_description(format!("{} description", name))
            .with_examples(vec![format!("Use {} to do something", name)])
            .with_io_signature(
                SkillIoSignature::new(
                    input_types.into_iter().map(|s| s.to_string()).collect(),
                    output_types.into_iter().map(|s| s.to_string()).collect(),
                )
                .with_sink(sink)
            )
    }

    #[tokio::test]
    async fn test_route_by_intent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let storage = Arc::new(VectorStorage::open_with_service(&db_path, embedding.clone()).unwrap());
        
        let db_manager = Arc::new(crate::storage::db::DbManager::new().unwrap());
        let skill_manager = Arc::new(SkillManager::new(db_manager.clone()).unwrap());
        let mut router = SkillVectorRouter::new(storage, embedding, skill_manager, db_manager);
        
        // 注册测试技能
        router.register_global_skill(create_test_skill(
            "data-analyzer",
            "Data Analyzer",
            vec!["json", "csv"],
            vec!["analysis_result"],
            false,
        ));
        
        router.register_global_skill(create_test_skill(
            "report-gen",
            "Report Generator",
            vec!["analysis_result"],
            vec!["report"],
            true,
        ));
        
        // 测试路由 - 使用与技能描述匹配的查询
        let result = router.route_by_intent("data analyzer analysis").await;
        assert!(result.is_ok(), "route_by_intent failed: {:?}", result.err());
        
        let routing = result.unwrap();
        assert!(routing.overall_confidence > 0.0);
        assert!(routing.skill_chain.is_some());
    }

    #[tokio::test]
    async fn test_discover_skill_chain() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let storage = Arc::new(VectorStorage::open_with_service(&db_path, embedding.clone()).unwrap());
        
        let db_manager = Arc::new(crate::storage::db::DbManager::new().unwrap());
        let skill_manager = Arc::new(SkillManager::new(db_manager.clone()).unwrap());
        let mut router = SkillVectorRouter::new(storage, embedding, skill_manager, db_manager);
        
        // 注册测试技能
        router.register_global_skill(create_test_skill(
            "data-analyzer",
            "Data Analyzer",
            vec!["json", "csv"],
            vec!["analysis_result"],
            false,
        ));
        
        router.register_global_skill(create_test_skill(
            "report-gen",
            "Report Generator",
            vec!["analysis_result"],
            vec!["report"],
            true,
        ));
        
        // 解析意图
        let parsed = ParsedIntent {
            raw_input: "分析并生成报告".to_string(),
            normalized_intent: "分析并生成报告".to_string(),
            embedding: vec![0.0; 768],
            entities: HashMap::new(),
            confidence: 0.9,
            action_type: ActionType::Analyze,
        };
        
        // 发现技能链
        let chain = router.discover_skill_chain("data-analyzer", &parsed).await.unwrap();
        let steps = chain.steps();
        
        // 应该至少有 data-analyzer，可能还有 report-gen
        assert!(!steps.is_empty());
        assert_eq!(steps[0].skill_id, "data-analyzer");
    }

    #[tokio::test]
    async fn test_check_io_compatibility() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let storage = Arc::new(VectorStorage::open_with_service(&db_path, embedding.clone()).unwrap());
        
        let db_manager = Arc::new(crate::storage::db::DbManager::new().unwrap());
        let skill_manager = Arc::new(SkillManager::new(db_manager.clone()).unwrap());
        let router = SkillVectorRouter::new(storage, embedding, skill_manager, db_manager);
        
        let source = create_test_skill(
            "source",
            "Source",
            vec!["input"],
            vec!["json", "text"],
            false,
        );
        
        let target = create_test_skill(
            "target",
            "Target",
            vec!["json"],
            vec!["output"],
            false,
        );
        
        let result = router.check_io_compatibility(&source, &target).await.unwrap();
        assert!(result.is_some());
        
        let (score, types) = result.unwrap();
        assert!(score > 0.0);
        assert!(!types.is_empty());
    }
}
