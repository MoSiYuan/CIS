//! # Skill Semantics
//!
//! 技能语义定义和管理，支持基于向量的技能发现和匹配。
//!
//! ## 功能特性
//!
//! - 技能语义描述管理
//! - 意图-能力分离
//! - 语义相似度计算
//! - 项目隔离

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::Result;
use crate::vector::storage::{SkillMatch, SkillSemantics as StorageSkillSemantics, VectorStorage};

/// 技能作用域
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    /// 全局技能
    Global,
    /// 项目级别技能
    #[default]
    Project,
    /// 会话级别技能
    Session,
}

/// 技能IO签名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIoSignature {
    /// 输入类型列表
    pub input_types: Vec<String>,
    /// 输出类型列表
    pub output_types: Vec<String>,
    /// 是否可管道连接
    pub pipeable: bool,
    /// 是否为数据源（产生数据）
    pub source: bool,
    /// 是否为数据汇（消费数据）
    pub sink: bool,
}

impl SkillIoSignature {
    /// 创建新的IO签名
    pub fn new(input_types: Vec<String>, output_types: Vec<String>) -> Self {
        Self {
            input_types,
            output_types,
            pipeable: true,
            source: false,
            sink: false,
        }
    }

    /// 设置为可管道连接
    pub fn with_pipeable(mut self, pipeable: bool) -> Self {
        self.pipeable = pipeable;
        self
    }

    /// 设置为数据源
    pub fn with_source(mut self, source: bool) -> Self {
        self.source = source;
        self
    }

    /// 设置为数据汇
    pub fn with_sink(mut self, sink: bool) -> Self {
        self.sink = sink;
        self
    }
}

/// 技能语义（主线C - Vector Intelligence）
/// 
/// 注意：这是扩展版本的技能语义定义，用于路由和编排。
/// 向量存储使用 `crate::vector::storage::SkillSemantics`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSemanticsExt {
    /// 技能ID
    pub skill_id: String,
    /// 技能名称
    pub skill_name: String,
    /// 描述
    pub description: String,
    /// 示例意图
    pub example_intents: Vec<String>,
    /// 参数schema
    pub parameter_schema: Option<serde_json::Value>,
    /// IO签名
    pub io_signature: Option<SkillIoSignature>,
    /// 作用域
    pub scope: SkillScope,
}

impl SkillSemanticsExt {
    /// 创建新的技能语义
    pub fn new(skill_id: impl Into<String>, skill_name: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            skill_name: skill_name.into(),
            description: String::new(),
            example_intents: Vec::new(),
            parameter_schema: None,
            io_signature: None,
            scope: SkillScope::Project,
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// 添加示例意图
    pub fn with_examples(mut self, examples: Vec<String>) -> Self {
        self.example_intents = examples;
        self
    }

    /// 设置参数schema
    pub fn with_schema(mut self, schema: serde_json::Value) -> Self {
        self.parameter_schema = Some(schema);
        self
    }

    /// 设置IO签名
    pub fn with_io_signature(mut self, signature: SkillIoSignature) -> Self {
        self.io_signature = Some(signature);
        self
    }

    /// 设置作用域
    pub fn with_scope(mut self, scope: SkillScope) -> Self {
        self.scope = scope;
        self
    }

    /// 生成意图描述文本（用于向量化匹配）
    pub fn to_intent_description(&self) -> String {
        let mut desc = format!("{}: {}", self.skill_name, self.description);
        if !self.example_intents.is_empty() {
            desc.push_str("\nExamples: ");
            desc.push_str(&self.example_intents.join(", "));
        }
        desc
    }
}

// 注意：使用 SkillSemanticsExt 而不是 SkillSemantics 以避免与 vector::storage::SkillSemantics 冲突

/// 技能语义描述（传统版本，与SkillSemantics并存）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSemanticDescription {
    /// 技能ID
    pub skill_id: String,
    /// 技能名称
    pub skill_name: String,
    /// 技能版本
    pub version: String,
    /// 意图描述（自然语言描述技能可以响应什么类型的请求）
    pub intent_description: String,
    /// 能力描述（自然语言描述技能可以执行什么操作）
    pub capability_description: String,
    /// 使用示例
    pub examples: Vec<String>,
    /// 关联项目
    pub project: Option<String>,
    /// 标签
    pub tags: Vec<String>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

impl SkillSemanticDescription {
    /// 创建新的语义描述
    pub fn new(
        skill_id: impl Into<String>,
        skill_name: impl Into<String>,
        intent_description: impl Into<String>,
        capability_description: impl Into<String>,
    ) -> Self {
        Self {
            skill_id: skill_id.into(),
            skill_name: skill_name.into(),
            version: "0.1.0".to_string(),
            intent_description: intent_description.into(),
            capability_description: capability_description.into(),
            examples: Vec::new(),
            project: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// 设置版本
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// 添加示例
    pub fn with_examples(mut self, examples: Vec<String>) -> Self {
        self.examples = examples;
        self
    }

    /// 设置项目
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// 添加标签
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    /// 转换为存储用的 SkillSemantics
    pub fn to_semantics(&self) -> StorageSkillSemantics {
        StorageSkillSemantics {
            skill_id: self.skill_id.clone(),
            skill_name: self.skill_name.clone(),
            intent_description: self.intent_description.clone(),
            capability_description: self.capability_description.clone(),
            project: self.project.clone(),
        }
    }
}

/// 技能语义注册表
pub struct SkillSemanticRegistry {
    /// 本地缓存的语义描述
    descriptions: HashMap<String, SkillSemanticDescription>,
    /// 向量存储（可选）
    vector_storage: Option<VectorStorage>,
}

impl SkillSemanticRegistry {
    /// 创建新的注册表（无向量存储）
    pub fn new() -> Self {
        Self {
            descriptions: HashMap::new(),
            vector_storage: None,
        }
    }

    /// 创建带有向量存储的注册表
    pub fn with_vector_storage(storage: VectorStorage) -> Self {
        Self {
            descriptions: HashMap::new(),
            vector_storage: Some(storage),
        }
    }

    /// 注册技能语义（异步版本）
    pub async fn register(&mut self, description: SkillSemanticDescription) -> Result<()> {
        let skill_id = description.skill_id.clone();
        
        // 如果有向量存储，同步到向量数据库
        if let Some(storage) = &self.vector_storage {
            let semantics = description.to_semantics();
            storage.register_skill(&semantics).await?;
        }
        
        // 更新本地缓存
        self.descriptions.insert(skill_id, description);
        
        Ok(())
    }

    /// 注册技能语义（同步版本，仅更新本地缓存）
    pub fn register_sync(&mut self, description: SkillSemanticDescription) {
        let skill_id = description.skill_id.clone();
        self.descriptions.insert(skill_id, description);
    }

    /// 取消注册技能语义
    pub fn unregister(&mut self, skill_id: &str) -> Result<bool> {
        // 从向量存储中删除
        if let Some(storage) = &self.vector_storage {
            let _ = storage.delete_skill_index(skill_id);
        }
        
        // 从本地缓存删除
        Ok(self.descriptions.remove(skill_id).is_some())
    }

    /// 获取技能语义描述
    pub fn get(&self, skill_id: &str) -> Option<&SkillSemanticDescription> {
        self.descriptions.get(skill_id)
    }

    /// 使用向量搜索匹配技能
    pub async fn search_by_vector(
        &self,
        query: &str,
        project: Option<&str>,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SkillMatch>> {
        match &self.vector_storage {
            Some(storage) => {
                storage.search_skills(query, project, limit, threshold).await
            }
            None => {
                // 如果没有向量存储，使用本地关键词匹配
                Ok(self.search_by_keywords(query, project, limit, threshold))
            }
        }
    }

    /// 使用关键词搜索匹配技能（备用方案）
    pub fn search_by_keywords(
        &self,
        query: &str,
        project: Option<&str>,
        limit: usize,
        threshold: Option<f32>,
    ) -> Vec<SkillMatch> {
        let threshold = threshold.unwrap_or(0.3);
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        for desc in self.descriptions.values() {
            // 项目过滤
            if let Some(proj) = project {
                if desc.project.as_deref() != Some(proj) {
                    continue;
                }
            }

            // 计算匹配分数
            let mut intent_score = 0.0f32;
            let mut cap_score = 0.0f32;

            // 意图描述匹配
            let intent_lower = desc.intent_description.to_lowercase();
            if intent_lower.contains(&query_lower) {
                intent_score = 0.7;
            }
            // 关键词部分匹配
            for word in query_lower.split_whitespace() {
                if intent_lower.contains(word) {
                    intent_score += 0.1;
                }
            }

            // 能力描述匹配
            let cap_lower = desc.capability_description.to_lowercase();
            if cap_lower.contains(&query_lower) {
                cap_score = 0.7;
            }
            for word in query_lower.split_whitespace() {
                if cap_lower.contains(word) {
                    cap_score += 0.1;
                }
            }

            // 示例匹配
            for example in &desc.examples {
                if example.to_lowercase().contains(&query_lower) {
                    intent_score += 0.2;
                    break;
                }
            }

            // 标签匹配
            for tag in &desc.tags {
                if tag.to_lowercase().contains(&query_lower) {
                    intent_score += 0.3;
                }
            }

            // 归一化
            intent_score = intent_score.min(1.0);
            cap_score = cap_score.min(1.0);
            let combined = (intent_score + cap_score) / 2.0;

            if combined >= threshold {
                matches.push(SkillMatch {
                    skill_id: desc.skill_id.clone(),
                    skill_name: desc.skill_name.clone(),
                    intent_similarity: intent_score,
                    capability_similarity: cap_score,
                    combined_score: combined,
                });
            }
        }

        // 排序并限制结果
        matches.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        matches.truncate(limit);
        matches
    }

    /// 根据项目列出技能
    pub fn list_by_project(&self, project: &str) -> Vec<&SkillSemanticDescription> {
        self.descriptions
            .values()
            .filter(|desc| desc.project.as_deref() == Some(project))
            .collect()
    }

    /// 列出所有技能
    pub fn list_all(&self) -> Vec<&SkillSemanticDescription> {
        self.descriptions.values().collect()
    }

    /// 获取技能数量
    pub fn count(&self) -> usize {
        self.descriptions.len()
    }

    /// 清空注册表
    pub fn clear(&mut self) {
        self.descriptions.clear();
        // 不清除向量存储，只清除本地缓存
    }
}

impl Default for SkillSemanticRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 技能语义匹配器
pub struct SkillSemanticMatcher;

impl SkillSemanticMatcher {
    /// 计算两个描述的相似度（简单实现）
    pub fn calculate_similarity(
        desc1: &SkillSemanticDescription,
        desc2: &SkillSemanticDescription,
    ) -> f32 {
        let mut score = 0.0f32;

        // 标签重叠度
        let tag_overlap: f32 = desc1
            .tags
            .iter()
            .filter(|t| desc2.tags.contains(t))
            .count() as f32;
        if !desc1.tags.is_empty() || !desc2.tags.is_empty() {
            score += tag_overlap
                / ((desc1.tags.len() + desc2.tags.len()) as f32 / 2.0 + 1.0);
        }

        // 意图描述关键词重叠
        let intent_words1: Vec<_> = desc1.intent_description.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
        let intent_words2: Vec<_> = desc2.intent_description.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
        let intent_overlap: f32 = intent_words1
            .iter()
            .filter(|w| intent_words2.contains(w))
            .count() as f32;
        if !intent_words1.is_empty() || !intent_words2.is_empty() {
            score += intent_overlap
                / ((intent_words1.len() + intent_words2.len()) as f32 / 2.0 + 1.0);
        }

        score.min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_description(id: &str, name: &str) -> SkillSemanticDescription {
        SkillSemanticDescription::new(
            id,
            name,
            format!("Intent for {}", name),
            format!("Can perform {} operations", name),
        )
        .with_examples(vec![format!("Use {} to do something", name)])
        .with_tags(vec![name.to_lowercase(), "test".to_string()])
    }

    #[test]
    fn test_semantic_registry() {
        let mut registry = SkillSemanticRegistry::new();

        // 注册技能（无向量存储，同步操作）
        let desc1 = create_test_description("skill1", "Search");
        registry.register_sync(desc1);

        let desc2 = create_test_description("skill2", "Delete")
            .with_project("myproject");
        registry.register_sync(desc2);

        // 测试获取
        assert!(registry.get("skill1").is_some());
        assert_eq!(registry.get("skill1").unwrap().skill_name, "Search");

        // 测试项目过滤
        let project_skills = registry.list_by_project("myproject");
        assert_eq!(project_skills.len(), 1);

        // 测试关键词搜索
        let matches = registry.search_by_keywords("search operation", None, 10, None);
        assert!(!matches.is_empty());

        // 测试取消注册
        assert!(registry.unregister("skill1").unwrap());
        assert!(registry.get("skill1").is_none());
    }

    #[test]
    fn test_similarity_calculation() {
        let desc1 = create_test_description("skill1", "Test")
            .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);
        let desc2 = create_test_description("skill2", "Test2")
            .with_tags(vec!["tag1".to_string(), "tag3".to_string()]);

        let similarity = SkillSemanticMatcher::calculate_similarity(&desc1, &desc2);
        assert!(similarity > 0.0);
        assert!(similarity <= 1.0);
    }
}
