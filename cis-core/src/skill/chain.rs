//! # Skill Chain
//!
//! 技能链编排系统，支持管道式的技能组合。
//!
//! ## 功能特性
//!
//! - 链式技能执行
//! - 输入输出映射
//! - 条件执行
//! - 错误处理
//! - 自动兼容性发现

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;
use crate::skill::semantics::SkillSemanticsExt;

/// 链步骤执行结果（用于 chain 模块）
/// 
/// 注意：这与 router 模块中的 ChainStepResult 是独立的类型
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 技能链
#[derive(Debug, Clone)]
pub struct SkillChain {
    /// 执行步骤
    steps: Vec<ChainStep>,
    /// 执行上下文
    context: ChainContext,
    /// 元数据
    metadata: ChainMetadata,
}

/// 链步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    /// 技能ID
    pub skill_id: String,
    /// 输入映射 (from_output, to_param)
    pub input_mapping: Vec<(String, String)>,
    /// 执行条件
    pub condition: Option<String>,
    /// 重试次数
    pub max_retries: u32,
    /// 超时（秒）
    pub timeout_secs: u64,
}

impl ChainStep {
    /// 创建新的链步骤
    pub fn new(skill_id: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            input_mapping: Vec::new(),
            condition: None,
            max_retries: 0,
            timeout_secs: 30,
        }
    }

    /// 设置输入映射
    pub fn with_mapping(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.input_mapping.push((from.into(), to.into()));
        self
    }

    /// 设置执行条件
    pub fn with_condition(mut self, condition: impl Into<String>) -> Self {
        self.condition = Some(condition.into());
        self
    }

    /// 设置重试次数
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// 链上下文
#[derive(Debug, Clone)]
pub struct ChainContext {
    /// 初始输入
    pub initial_input: Value,
    /// 中间结果
    pub intermediate_results: Vec<StepResult>,
    /// 项目路径
    pub project_path: Option<PathBuf>,
}

/// 步骤执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// 步骤索引
    pub step_index: usize,
    /// 技能ID
    pub skill_id: String,
    /// 输出
    pub output: Value,
    /// 是否成功
    pub success: bool,
    /// 执行时间（毫秒）
    pub execution_time_ms: u64,
}

/// 链元数据
#[derive(Debug, Clone, Default)]
pub struct ChainMetadata {
    /// 链名称
    pub name: Option<String>,
    /// 描述
    pub description: Option<String>,
    /// 创建时间
    pub created_at: Option<i64>,
    /// 版本
    pub version: String,
}

/// 技能兼容性记录（用于数据库）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCompatibilityRecord {
    /// 源技能ID
    pub source_skill_id: String,
    /// 目标技能ID
    pub target_skill_id: String,
    /// 兼容性评分 (0.0 - 1.0)
    pub compatibility_score: f64,
    /// 支持的数据流类型（JSON 数组）
    pub data_flow_types: String,
    /// 发现时间
    pub discovered_at: i64,
}

/// 链发现结果
#[derive(Debug, Clone)]
pub struct ChainDiscoveryResult {
    /// 发现的链
    pub chain: SkillChain,
    /// 置信度
    pub confidence: f32,
    /// 发现的理由
    pub reason: String,
}

impl SkillChain {
    /// 创建链构建器
    pub fn builder() -> ChainBuilder {
        ChainBuilder::new()
    }

    /// 创建新的技能链
    pub fn new(initial_input: Value) -> Self {
        Self {
            steps: Vec::new(),
            context: ChainContext {
                initial_input,
                intermediate_results: Vec::new(),
                project_path: None,
            },
            metadata: ChainMetadata {
                name: None,
                description: None,
                created_at: Some(chrono::Utc::now().timestamp()),
                version: "1.0".to_string(),
            },
        }
    }

    /// 创建带有名称的技能链
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.metadata.name = Some(name.into());
        self
    }

    /// 添加步骤
    pub fn add_step(&mut self, skill_id: String) -> &mut Self {
        self.steps.push(ChainStep::new(skill_id));
        self
    }

    /// 添加带配置的步骤
    pub fn add_step_with_config(&mut self, step: ChainStep) -> &mut Self {
        self.steps.push(step);
        self
    }

    /// 为指定步骤添加输入映射
    pub fn add_input_mapping(&mut self, step_idx: usize, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        if let Some(step) = self.steps.get_mut(step_idx) {
            step.input_mapping.push((from.into(), to.into()));
        }
        self
    }

    /// 获取步骤列表
    pub fn steps(&self) -> &[ChainStep] {
        &self.steps
    }

    /// 获取步骤数量
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// 执行链
    pub async fn execute<F, Fut>(&mut self, executor: F) -> Result<Vec<StepResult>>
    where
        F: Fn(&str, Value) -> Fut,
        Fut: std::future::Future<Output = Result<Value>>,
    {
        let start_time = std::time::Instant::now();
        
        for (idx, step) in self.steps.iter().enumerate() {
            let step_start = std::time::Instant::now();
            
            // 准备输入
            let input = self.prepare_input(step);

            // 检查条件
            if let Some(cond) = &step.condition {
                if !self.evaluate_condition(cond) {
                    tracing::debug!("Step {} condition '{}' not met, skipping", idx, cond);
                    continue;
                }
            }

            // 执行（带重试）
            let mut last_error = None;
            let mut success = false;
            let mut output = Value::Null;
            
            for attempt in 0..=step.max_retries {
                match executor(&step.skill_id, input.clone()).await {
                    Ok(result) => {
                        output = result;
                        success = true;
                        break;
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        if attempt < step.max_retries {
                            tracing::warn!(
                                "Step {} execution failed (attempt {}), retrying: {}",
                                idx,
                                attempt + 1,
                                e
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(100 * (attempt + 1) as u64)).await;
                        }
                    }
                }
            }

            let execution_time_ms = step_start.elapsed().as_millis() as u64;

            if !success {
                self.context.intermediate_results.push(StepResult {
                    step_index: idx,
                    skill_id: step.skill_id.clone(),
                    output: Value::String(last_error.clone().unwrap_or_default()),
                    success: false,
                    execution_time_ms,
                });
                
                // 链失败处理：中断执行
                tracing::error!(
                    "Chain execution failed at step {} (skill: {}): {:?}",
                    idx,
                    step.skill_id,
                    last_error
                );
                break;
            } else {
                self.context.intermediate_results.push(StepResult {
                    step_index: idx,
                    skill_id: step.skill_id.clone(),
                    output: output.clone(),
                    success: true,
                    execution_time_ms,
                });
            }
        }

        let total_time = start_time.elapsed().as_millis() as u64;
        tracing::info!(
            "Chain execution completed: {} steps, total time: {}ms",
            self.context.intermediate_results.len(),
            total_time
        );

        Ok(self.context.intermediate_results.clone())
    }

    /// 同步执行链（用于非异步上下文）
    pub fn execute_sync<F>(&mut self, executor: F) -> Result<Vec<StepResult>>
    where
        F: Fn(&str, Value) -> Result<Value>,
    {
        for (idx, step) in self.steps.iter().enumerate() {
            let step_start = std::time::Instant::now();
            
            // 准备输入
            let input = self.prepare_input(step);

            // 检查条件
            if let Some(cond) = &step.condition {
                if !self.evaluate_condition(cond) {
                    continue;
                }
            }

            // 执行
            match executor(&step.skill_id, input) {
                Ok(output) => {
                    self.context.intermediate_results.push(StepResult {
                        step_index: idx,
                        skill_id: step.skill_id.clone(),
                        output,
                        success: true,
                        execution_time_ms: step_start.elapsed().as_millis() as u64,
                    });
                }
                Err(e) => {
                    self.context.intermediate_results.push(StepResult {
                        step_index: idx,
                        skill_id: step.skill_id.clone(),
                        output: Value::String(e.to_string()),
                        success: false,
                        execution_time_ms: step_start.elapsed().as_millis() as u64,
                    });
                    // 链失败处理：可以继续或中断
                    break;
                }
            }
        }

        Ok(self.context.intermediate_results.clone())
    }

    /// 准备步骤输入
    fn prepare_input(&self, step: &ChainStep) -> Value {
        let mut input = serde_json::Map::new();

        // 映射前序步骤的输出
        for (from_output, to_param) in &step.input_mapping {
            if let Some(prev_result) = self.context.intermediate_results.last() {
                if let Some(value) = prev_result.output.get(from_output) {
                    input.insert(to_param.clone(), value.clone());
                }
            }
        }

        // 如果没有映射，使用初始输入
        if input.is_empty() {
            return self.context.initial_input.clone();
        }

        serde_json::Value::Object(input)
    }

    /// 评估条件
    fn evaluate_condition(&self, _condition: &str) -> bool {
        // 简化实现：始终执行
        // TODO: 实现条件表达式解析
        true
    }

    /// 获取执行结果
    pub fn results(&self) -> &[StepResult] {
        &self.context.intermediate_results
    }

    /// 获取最后一步结果
    pub fn last_result(&self) -> Option<&StepResult> {
        self.context.intermediate_results.last()
    }

    /// 获取最后成功的结果
    pub fn last_success_result(&self) -> Option<&StepResult> {
        self.context.intermediate_results.iter().rev().find(|r| r.success)
    }

    /// 设置项目路径
    pub fn with_project_path(mut self, path: PathBuf) -> Self {
        self.context.project_path = Some(path);
        self
    }

    /// 获取项目路径
    pub fn project_path(&self) -> Option<&PathBuf> {
        self.context.project_path.as_ref()
    }

    /// 转换为 JSON
    pub fn to_json(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "steps": self.steps,
            "metadata": {
                "name": self.metadata.name,
                "description": self.metadata.description,
                "created_at": self.metadata.created_at,
                "version": self.metadata.version,
            },
            "results": self.context.intermediate_results,
        }))
    }
}

/// 链构建器
pub struct ChainBuilder {
    steps: Vec<ChainStep>,
    metadata: ChainMetadata,
}

impl ChainBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            metadata: ChainMetadata {
                name: None,
                description: None,
                created_at: Some(chrono::Utc::now().timestamp()),
                version: "1.0".to_string(),
            },
        }
    }

    /// 设置链名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.metadata.name = Some(name.into());
        self
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.metadata.description = Some(description.into());
        self
    }

    /// 添加步骤
    pub fn then(mut self, skill_id: &str) -> Self {
        self.steps.push(ChainStep::new(skill_id));
        self
    }

    /// 添加带配置的步骤
    pub fn then_with_config(mut self, step: ChainStep) -> Self {
        self.steps.push(step);
        self
    }

    /// 添加上一步到当前步骤的管道映射
    pub fn pipe(mut self, from_output: &str, to_param: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.input_mapping.push((from_output.to_string(), to_param.to_string()));
        }
        self
    }

    /// 添加条件
    pub fn when(mut self, condition: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.condition = Some(condition.to_string());
        }
        self
    }

    /// 设置当前步骤重试次数
    pub fn retry(mut self, times: u32) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.max_retries = times;
        }
        self
    }

    /// 设置当前步骤超时
    pub fn timeout(mut self, secs: u64) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.timeout_secs = secs;
        }
        self
    }

    /// 构建链
    pub fn build(self, initial_input: Value) -> SkillChain {
        SkillChain {
            steps: self.steps,
            context: ChainContext {
                initial_input,
                intermediate_results: Vec::new(),
                project_path: None,
            },
            metadata: self.metadata,
        }
    }
}

impl Default for ChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 链编排器 - 高级链管理
pub struct ChainOrchestrator {
    /// 已注册的链模板
    templates: HashMap<String, ChainTemplate>,
    /// 兼容性缓存
    compatibility_cache: HashMap<(String, String), f32>,
}

/// 链模板
#[derive(Debug, Clone)]
pub struct ChainTemplate {
    /// 模板名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 步骤定义
    pub steps: Vec<ChainStep>,
    /// 适用场景标签
    pub tags: Vec<String>,
}

impl ChainOrchestrator {
    /// 创建新的编排器
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            compatibility_cache: HashMap::new(),
        }
    }

    /// 注册链模板
    pub fn register_template(&mut self, template: ChainTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    /// 根据标签查找模板
    pub fn find_templates_by_tag(&self, tag: &str) -> Vec<&ChainTemplate> {
        self.templates
            .values()
            .filter(|t| t.tags.contains(&tag.to_string()))
            .collect()
    }

    /// 基于意图匹配最佳模板
    pub fn match_template(&self, intent: &str) -> Option<&ChainTemplate> {
        // 简单的关键词匹配
        // TODO: 使用向量相似度匹配
        self.templates.values().find(|t| {
            t.description.to_lowercase().contains(&intent.to_lowercase()) ||
            t.tags.iter().any(|tag| intent.to_lowercase().contains(&tag.to_lowercase()))
        })
    }

    /// 自动发现技能链
    ///
    /// 基于技能语义自动发现可执行的链式组合
    pub async fn auto_discover_chains(
        &self,
        skills: &[SkillSemanticsExt],
        max_depth: usize,
    ) -> Vec<ChainDiscoveryResult> {
        let mut discovered = Vec::new();
        
        for skill in skills {
            // 如果技能是 sink，跳过（作为链的终点）
            if let Some(io) = &skill.io_signature {
                if io.sink {
                    continue;
                }
            }
            
            // 尝试构建链
            if let Some(chain) = self.build_chain_from_skill(skill, skills, max_depth).await {
                let confidence = self.calculate_chain_confidence(&chain, skills);
                discovered.push(ChainDiscoveryResult {
                    chain,
                    confidence,
                    reason: format!("Auto-discovered from skill {}", skill.skill_id),
                });
            }
        }
        
        // 按置信度排序
        discovered.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        
        discovered
    }

    /// 从指定技能构建链
    async fn build_chain_from_skill(
        &self,
        start_skill: &SkillSemanticsExt,
        all_skills: &[SkillSemanticsExt],
        max_depth: usize,
    ) -> Option<SkillChain> {
        let mut chain = SkillChain::new(serde_json::json!({}));
        chain.add_step(start_skill.skill_id.clone());
        
        let mut current_skill = start_skill;
        let mut depth = 1;
        
        while depth < max_depth {
            // 查找兼容的下一个技能
            let next_skill = self.find_best_next_skill(current_skill, all_skills)?;
            
            // 设置输入映射
            let step_idx = chain.len();
            if let Some((from, to)) = self.infer_io_mapping(current_skill, &next_skill) {
                chain.add_input_mapping(step_idx, from, to);
            }
            
            chain.add_step(next_skill.skill_id.clone());
            
            // 如果下一个技能是 sink，结束链
            if let Some(io) = &next_skill.io_signature {
                if io.sink {
                    break;
                }
            }
            
            current_skill = all_skills.iter().find(|s| s.skill_id == next_skill.skill_id)?;
            depth += 1;
        }
        
        if chain.len() > 1 {
            Some(chain)
        } else {
            None
        }
    }

    /// 查找最佳下一个技能
    fn find_best_next_skill(
        &self,
        current: &SkillSemanticsExt,
        candidates: &[SkillSemanticsExt],
    ) -> Option<SkillSemanticsExt> {
        let current_io = current.io_signature.as_ref()?;
        
        let mut best_match: Option<(SkillSemanticsExt, f32)> = None;
        
        for candidate in candidates {
            if candidate.skill_id == current.skill_id {
                continue;
            }
            
            let candidate_io = match &candidate.io_signature {
                Some(io) => io,
                None => continue,
            };
            
            // 计算兼容性得分
            let score = self.calculate_compatibility_score(current_io, candidate_io);
            
            if score > 0.5 {
                if best_match.as_ref().map_or(true, |(_, s)| score > *s) {
                    best_match = Some((candidate.clone(), score));
                }
            }
        }
        
        best_match.map(|(skill, _)| skill)
    }

    /// 计算兼容性得分
    fn calculate_compatibility_score(
        &self,
        source: &crate::skill::semantics::SkillIoSignature,
        target: &crate::skill::semantics::SkillIoSignature,
    ) -> f32 {
        if source.output_types.is_empty() || target.input_types.is_empty() {
            return 0.0;
        }
        
        let mut matches = 0;
        for out_type in &source.output_types {
            for in_type in &target.input_types {
                if self.is_type_compatible(out_type, in_type) {
                    matches += 1;
                }
            }
        }
        
        let max_possible = source.output_types.len() * target.input_types.len();
        let base_score = matches as f32 / max_possible as f32;
        
        // 可管道连接加分
        if source.pipeable && target.pipeable {
            (base_score * 1.2).min(1.0)
        } else {
            base_score
        }
    }

    /// 检查类型兼容性
    fn is_type_compatible(&self, output_type: &str, input_type: &str) -> bool {
        if output_type == input_type {
            return true;
        }
        
        let compatible_pairs = [
            ("json", "object"),
            ("object", "json"),
            ("text", "string"),
            ("string", "text"),
            ("csv", "table"),
            ("table", "csv"),
            ("file", "path"),
            ("path", "file"),
            ("analysis_result", "report_input"),
            ("data", "input"),
        ];
        
        compatible_pairs.iter().any(|(out, inp)| {
            (output_type.eq_ignore_ascii_case(out) && input_type.eq_ignore_ascii_case(inp)) ||
            (output_type.eq_ignore_ascii_case(inp) && input_type.eq_ignore_ascii_case(out))
        })
    }

    /// 推断 IO 映射
    fn infer_io_mapping(
        &self,
        source: &SkillSemanticsExt,
        target: &SkillSemanticsExt,
    ) -> Option<(String, String)> {
        let source_io = source.io_signature.as_ref()?;
        let target_io = target.io_signature.as_ref()?;
        
        for out_type in &source_io.output_types {
            for in_type in &target_io.input_types {
                if self.is_type_compatible(out_type, in_type) {
                    return Some((
                        format!("output_{}", out_type),
                        format!("input_{}", in_type),
                    ));
                }
            }
        }
        
        None
    }

    /// 计算链的置信度
    fn calculate_chain_confidence(&self, chain: &SkillChain, skills: &[SkillSemanticsExt]) -> f32 {
        let steps = chain.steps();
        if steps.len() <= 1 {
            return 1.0;
        }
        
        let mut total_score = 1.0f32;
        
        for i in 0..steps.len() - 1 {
            let current_skill = skills.iter().find(|s| s.skill_id == steps[i].skill_id);
            let next_skill = skills.iter().find(|s| s.skill_id == steps[i + 1].skill_id);
            
            if let (Some(current), Some(next)) = (current_skill, next_skill) {
                if let (Some(current_io), Some(next_io)) = (&current.io_signature, &next.io_signature) {
                    let score = self.calculate_compatibility_score(current_io, next_io);
                    total_score *= score;
                }
            }
        }
        
        // 长度惩罚
        let length_penalty = 1.0 - (steps.len() as f32 - 1.0) * 0.1;
        
        total_score * length_penalty.max(0.5)
    }
}

impl Default for ChainOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// 预定义技能链模板
pub struct ChainTemplates;

impl ChainTemplates {
    /// 分析-提交链
    pub fn analyze_and_commit(input: Value) -> SkillChain {
        SkillChain::builder()
            .with_name("analyze_and_commit")
            .with_description("Analyze changes and create a commit")
            .then("cis-local:read")
            .then("cis-local:analyze")
            .pipe("analysis", "input")
            .then("cis-local:commit")
            .build(input)
    }

    /// 文件列表-分析链
    pub fn list_and_analyze(input: Value) -> SkillChain {
        SkillChain::builder()
            .with_name("list_and_analyze")
            .with_description("List files and analyze them")
            .then("cis-local:file-list")
            .pipe("files", "file_list")
            .then("cis-local:analyze")
            .build(input)
    }

    /// 完整工作流链
    pub fn full_workflow(input: Value) -> SkillChain {
        SkillChain::builder()
            .with_name("full_workflow")
            .with_description("Complete workflow: list, read, analyze, and commit")
            .then("cis-local:file-list")
            .then("cis-local:read")
            .pipe("content", "input")
            .then("cis-local:analyze")
            .pipe("analysis", "input")
            .then("cis-local:commit")
            .build(input)
    }

    /// 数据分析-报告生成链
    pub fn analyze_and_report(input: Value) -> SkillChain {
        SkillChain::builder()
            .with_name("analyze_and_report")
            .with_description("Analyze data and generate a report")
            .then("data-analyzer")
            .pipe("analysis_result", "input")
            .then("report-gen")
            .build(input)
    }

    /// 获取所有模板
    pub fn all_templates() -> Vec<(String, fn(Value) -> SkillChain)> {
        vec![
            ("analyze_and_commit".to_string(), Self::analyze_and_commit as fn(Value) -> SkillChain),
            ("list_and_analyze".to_string(), Self::list_and_analyze as fn(Value) -> SkillChain),
            ("full_workflow".to_string(), Self::full_workflow as fn(Value) -> SkillChain),
            ("analyze_and_report".to_string(), Self::analyze_and_report as fn(Value) -> SkillChain),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::semantics::SkillIoSignature;

    #[test]
    fn test_chain_builder() {
        let chain = SkillChain::builder()
            .with_name("test_chain")
            .with_description("Test chain description")
            .then("skill1")
            .then("skill2")
            .pipe("output1", "input1")
            .build(serde_json::json!({"initial": true}));

        assert_eq!(chain.steps().len(), 2);
        assert_eq!(chain.len(), 2);
        assert!(chain.results().is_empty());
    }

    #[test]
    fn test_chain_step_config() {
        let step = ChainStep::new("test-skill")
            .with_mapping("output", "input")
            .with_condition("success")
            .with_retries(3)
            .with_timeout(60);

        assert_eq!(step.skill_id, "test-skill");
        assert_eq!(step.input_mapping.len(), 1);
        assert_eq!(step.max_retries, 3);
        assert_eq!(step.timeout_secs, 60);
    }

    #[test]
    fn test_chain_templates() {
        let input = serde_json::json!({"path": "/tmp"});
        
        let chain1 = ChainTemplates::analyze_and_commit(input.clone());
        assert_eq!(chain1.len(), 3);
        assert!(chain1.last_result().is_none());

        let chain2 = ChainTemplates::list_and_analyze(input.clone());
        assert_eq!(chain2.len(), 2);

        let chain3 = ChainTemplates::full_workflow(input.clone());
        assert_eq!(chain3.len(), 4);

        let chain4 = ChainTemplates::analyze_and_report(input);
        assert_eq!(chain4.len(), 2);
    }

    #[test]
    fn test_chain_orchestrator() {
        let mut orchestrator = ChainOrchestrator::new();
        
        let template = ChainTemplate {
            name: "test_template".to_string(),
            description: "Test template for analysis".to_string(),
            steps: vec![
                ChainStep::new("skill1"),
                ChainStep::new("skill2"),
            ],
            tags: vec!["analysis".to_string(), "test".to_string()],
        };
        
        orchestrator.register_template(template);
        
        let templates = orchestrator.find_templates_by_tag("analysis");
        assert_eq!(templates.len(), 1);
        
        let matched = orchestrator.match_template("analysis workflow");
        assert!(matched.is_some());
    }

    #[test]
    fn test_io_compatibility() {
        let orchestrator = ChainOrchestrator::new();
        
        let source = SkillIoSignature::new(
            vec!["input".to_string()],
            vec!["json".to_string(), "text".to_string()],
        );
        
        let target = SkillIoSignature::new(
            vec!["json".to_string()],
            vec!["output".to_string()],
        );
        
        let score = orchestrator.calculate_compatibility_score(&source, &target);
        assert!(score > 0.0);
    }

    #[test]
    fn test_chain_json_serialization() {
        let chain = SkillChain::builder()
            .with_name("test")
            .then("skill1")
            .then("skill2")
            .build(serde_json::json!({"key": "value"}));
        
        let json = chain.to_json().unwrap();
        assert!(json.get("steps").is_some());
        assert!(json.get("metadata").is_some());
    }
}
