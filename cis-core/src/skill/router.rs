//! # Skill Vector Router
//!
//! 基于向量的技能路由系统，支持意图到技能的智能匹配。
//!
//! ## 功能特性
//!
//! - 基于向量相似度的技能路由
//! - 全局技能注册
//! - 技能链自动检测
//! - 参数提取

use std::sync::Arc;

use crate::ai::embedding::EmbeddingService;
use crate::error::Result;
use crate::intent::{ActionType, EntityValue, ParsedIntent};
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

/// 技能向量路由器
pub struct SkillVectorRouter {
    /// 向量存储
    storage: Arc<VectorStorage>,
    /// 嵌入服务
    embedding_service: Arc<dyn EmbeddingService>,
    /// 全局技能列表
    global_skills: Vec<SkillSemanticsExt>,
}

impl SkillVectorRouter {
    /// 创建新的路由器
    pub fn new(storage: Arc<VectorStorage>, embedding: Arc<dyn EmbeddingService>) -> Self {
        Self {
            storage,
            embedding_service: embedding,
            global_skills: Vec::new(),
        }
    }

    /// 注册全局技能
    pub fn register_global_skill(&mut self, semantics: SkillSemanticsExt) {
        self.global_skills.push(semantics);
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
