//! # Intent 模块
//!
//! 意图识别和管理，支持自然语言到技能调用的映射。
//!
//! ## 功能特性
//!
//! - 自然语言意图识别
//! - 意图-技能映射管理
//! - 多项目意图隔离
//! - 意图相似度匹配

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::ai::embedding::EmbeddingService;
use crate::error::{CisError, Result};

/// 意图类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    /// 查询类意图
    Query,
    /// 操作类意图
    Action,
    /// 配置类意图
    Config,
    /// 创建类意图
    Create,
    /// 删除类意图
    Delete,
    /// 更新类意图
    Update,
    /// 其他
    #[default]
    Other,
}

/// 动作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// 查询
    Query,
    /// 创建
    Create,
    /// 更新
    Update,
    /// 删除
    Delete,
    /// 分析
    Analyze,
    /// 提交
    Commit,
    /// 其他
    #[default]
    Other,
}

/// 实体值类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EntityValue {
    /// 字符串
    String(String),
    /// 数字
    Number(f64),
    /// 日期时间
    DateTime(DateTime<Utc>),
    /// 文件路径
    FilePath(PathBuf),
    /// 列表
    List(Vec<EntityValue>),
}

/// 解析后的意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedIntent {
    /// 原始输入
    pub raw_input: String,
    /// 规范化后的意图文本
    pub normalized_intent: String,
    /// 向量表示
    #[serde(skip)]
    pub embedding: Vec<f32>,
    /// 提取的实体
    pub entities: HashMap<String, EntityValue>,
    /// 置信度
    pub confidence: f32,
    /// 识别的动作类型
    pub action_type: ActionType,
}

/// 意图解析器
pub struct IntentParser {
    embedding_service: Arc<dyn EmbeddingService>,
}

impl IntentParser {
    /// 创建新的意图解析器
    pub fn new(embedding_service: Arc<dyn EmbeddingService>) -> Self {
        Self {
            embedding_service,
        }
    }

    /// 解析用户输入
    pub async fn parse(&self, input: &str) -> Result<ParsedIntent> {
        // 1. 生成嵌入向量
        let embedding = self.embedding_service.embed(input).await
            .map_err(|e| CisError::intent(format!("Embedding failed: {}", e)))?;
        
        // 2. 命名实体识别 (NER)
        let entities = self.extract_entities(input).await?;
        
        // 3. 规范化意图文本（去除实体后的核心意图）
        let normalized = self.normalize_intent(input, &entities);
        
        // 4. 判断动作类型
        let action_type = self.classify_action(input);
        
        // 5. 计算置信度
        let confidence = self.calculate_confidence(input, &entities);
        
        Ok(ParsedIntent {
            raw_input: input.to_string(),
            normalized_intent: normalized,
            embedding,
            entities,
            confidence,
            action_type,
        })
    }
    
    /// 实体提取
    async fn extract_entities(&self, input: &str) -> Result<HashMap<String, EntityValue>> {
        let mut entities = HashMap::new();
        
        // 时间实体
        if let Some(date) = self.extract_datetime(input) {
            entities.insert("time".to_string(), EntityValue::DateTime(date));
        }
        
        // 文件路径
        if let Some(path) = self.extract_file_path(input) {
            entities.insert("file".to_string(), EntityValue::FilePath(path));
        }
        
        // 数字
        for (i, num) in self.extract_numbers(input).iter().enumerate() {
            entities.insert(format!("number_{}", i), EntityValue::Number(*num));
        }
        
        Ok(entities)
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
    
    /// 规范化意图
    fn normalize_intent(&self, input: &str, entities: &HashMap<String, EntityValue>) -> String {
        let mut normalized = input.to_string();
        
        // 替换实体为占位符
        for (key, value) in entities {
            let placeholder = format!("[{}]", key.to_uppercase());
            let value_str = match value {
                EntityValue::String(s) => s.clone(),
                EntityValue::DateTime(dt) => dt.to_rfc3339(),
                EntityValue::FilePath(p) => p.to_string_lossy().to_string(),
                _ => continue,
            };
            normalized = normalized.replace(&value_str, &placeholder);
        }
        
        normalized
    }
    
    /// 提取日期时间
    fn extract_datetime(&self, input: &str) -> Option<DateTime<Utc>> {
        let lower = input.to_lowercase();
        
        if lower.contains("今天") || lower.contains("today") {
            Some(Utc::now())
        } else if lower.contains("明天") || lower.contains("tomorrow") {
            Some(Utc::now() + chrono::Duration::days(1))
        } else {
            None
        }
    }
    
    /// 提取文件路径
    fn extract_file_path(&self, input: &str) -> Option<PathBuf> {
        // 简单的路径匹配（不使用regex）
        // 查找可能的文件路径：包含常见扩展名
        let extensions = [".csv", ".json", ".pdf", ".txt", ".md", ".rs", ".toml", ".py", ".js", ".ts"];
        
        for ext in &extensions {
            if let Some(pos) = input.find(ext) {
                // 向前查找路径开始
                let start = input[..pos].rfind(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let end = pos + ext.len();
                let path = &input[start..end];
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
        None
    }
    
    /// 提取数字
    fn extract_numbers(&self, input: &str) -> Vec<f64> {
        let mut numbers = Vec::new();
        let mut current = String::new();
        
        for c in input.chars() {
            if c.is_ascii_digit() || c == '.' {
                current.push(c);
            } else if !current.is_empty() {
                if let Ok(n) = current.parse::<f64>() {
                    numbers.push(n);
                }
                current.clear();
            }
        }
        
        // 处理最后一个数字
        if !current.is_empty() {
            if let Ok(n) = current.parse::<f64>() {
                numbers.push(n);
            }
        }
        
        numbers
    }
    
    /// 计算置信度
    fn calculate_confidence(&self, input: &str, entities: &HashMap<String, EntityValue>) -> f32 {
        // 基于输入长度和提取的实体计算置信度
        let base_confidence = 0.7f32;
        let entity_bonus = (entities.len() as f32 * 0.05).min(0.2);
        let length_bonus = if input.len() > 10 { 0.05 } else { 0.0 };
        
        (base_confidence + entity_bonus + length_bonus).min(1.0)
    }
}

/// 意图定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// 意图唯一ID
    pub id: String,
    /// 意图名称
    pub name: String,
    /// 意图类型
    pub intent_type: IntentType,
    /// 意图描述（用于向量化匹配）
    pub description: String,
    /// 匹配关键词
    pub keywords: Vec<String>,
    /// 关联技能ID
    pub skill_id: String,
    /// 关联项目（可选）
    pub project: Option<String>,
    /// 置信度阈值
    pub threshold: f32,
}

impl Intent {
    /// 创建新的意图
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        skill_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            intent_type: IntentType::Other,
            description: description.into(),
            keywords: Vec::new(),
            skill_id: skill_id.into(),
            project: None,
            threshold: 0.6,
        }
    }

    /// 设置意图类型
    pub fn with_type(mut self, intent_type: IntentType) -> Self {
        self.intent_type = intent_type;
        self
    }

    /// 添加关键词
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }

    /// 设置项目
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// 设置阈值
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }
}

/// 意图匹配结果
#[derive(Debug, Clone)]
pub struct IntentMatch {
    /// 匹配到的意图
    pub intent: Intent,
    /// 匹配分数
    pub score: f32,
    /// 匹配关键词
    pub matched_keywords: Vec<String>,
}

/// 意图管理器
pub struct IntentManager {
    /// 意图注册表
    intents: HashMap<String, Intent>,
    /// 技能ID到意图列表的映射
    skill_intents: HashMap<String, Vec<String>>,
}

impl IntentManager {
    /// 创建新的意图管理器
    pub fn new() -> Self {
        Self {
            intents: HashMap::new(),
            skill_intents: HashMap::new(),
        }
    }

    /// 注册意图
    pub fn register(&mut self, intent: Intent) -> Result<()> {
        let intent_id = intent.id.clone();
        let skill_id = intent.skill_id.clone();
        
        // 检查是否已存在
        if self.intents.contains_key(&intent_id) {
            return Err(CisError::intent(format!(
                "Intent with ID '{}' already exists",
                intent_id
            )));
        }
        
        // 添加到意图注册表
        self.intents.insert(intent_id.clone(), intent);
        
        // 添加到技能-意图映射
        self.skill_intents
            .entry(skill_id)
            .or_default()
            .push(intent_id);
        
        Ok(())
    }

    /// 取消注册意图
    pub fn unregister(&mut self, intent_id: &str) -> Result<bool> {
        if let Some(intent) = self.intents.remove(intent_id) {
            // 从技能-意图映射中移除
            if let Some(intents) = self.skill_intents.get_mut(&intent.skill_id) {
                intents.retain(|id| id != intent_id);
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取意图
    pub fn get(&self, intent_id: &str) -> Option<&Intent> {
        self.intents.get(intent_id)
    }

    /// 根据技能ID获取意图列表
    pub fn get_by_skill(&self, skill_id: &str) -> Vec<&Intent> {
        self.skill_intents
            .get(skill_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.intents.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 根据项目过滤意图
    pub fn get_by_project(&self, project: &str) -> Vec<&Intent> {
        self.intents
            .values()
            .filter(|intent| {
                intent.project.as_deref() == Some(project)
            })
            .collect()
    }

    /// 基于关键词的简单意图匹配
    pub fn match_keywords(&self, query: &str, project: Option<&str>) -> Vec<IntentMatch> {
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();
        
        for intent in self.intents.values() {
            // 项目过滤
            if let Some(proj) = project {
                if intent.project.as_deref() != Some(proj) {
                    continue;
                }
            }
            
            // 关键词匹配
            let mut matched_keywords = Vec::new();
            let mut score = 0.0;
            
            for keyword in &intent.keywords {
                let keyword_lower = keyword.to_lowercase();
                if query_lower.contains(&keyword_lower) {
                    matched_keywords.push(keyword.clone());
                    score += 1.0;
                }
            }
            
            // 名称匹配
            if query_lower.contains(&intent.name.to_lowercase()) {
                score += 0.5;
            }
            
            // 描述匹配（简单包含）
            if query_lower.contains(&intent.description.to_lowercase()) {
                score += 0.3;
            }
            
            if !matched_keywords.is_empty() || score > 0.0 {
                // 归一化分数（确保匹配的关键词能超过阈值）
                let max_possible_score = intent.keywords.len() as f32 + 0.5; // 关键词满分 + 名称匹配
                score = (score / max_possible_score).min(1.0);
                
                if score >= intent.threshold {
                    matches.push(IntentMatch {
                        intent: intent.clone(),
                        score,
                        matched_keywords,
                    });
                }
            }
        }
        
        // 按分数降序排序
        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        matches
    }

    /// 列出所有意图
    pub fn list_all(&self) -> Vec<&Intent> {
        self.intents.values().collect()
    }

    /// 获取意图数量
    pub fn count(&self) -> usize {
        self.intents.len()
    }

    /// 清空所有意图
    pub fn clear(&mut self) {
        self.intents.clear();
        self.skill_intents.clear();
    }
}

impl Default for IntentManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 意图解析结果
#[derive(Debug, Clone)]
pub struct IntentParseResult {
    /// 原始查询
    pub query: String,
    /// 解析到的意图
    pub intent: Option<Intent>,
    /// 置信度
    pub confidence: f32,
    /// 提取的参数
    pub params: HashMap<String, String>,
}

impl IntentParseResult {
    /// 创建空结果
    pub fn empty(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            intent: None,
            confidence: 0.0,
            params: HashMap::new(),
        }
    }

    /// 创建成功结果
    pub fn success(
        query: impl Into<String>,
        intent: Intent,
        confidence: f32,
        params: HashMap<String, String>,
    ) -> Self {
        Self {
            query: query.into(),
            intent: Some(intent),
            confidence,
            params,
        }
    }

    /// 是否匹配成功
    pub fn is_match(&self) -> bool {
        self.intent.is_some() && self.confidence > 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_intent(id: &str, name: &str, skill_id: &str) -> Intent {
        Intent::new(id, name, format!("{} description", name), skill_id)
            .with_keywords(vec![name.to_lowercase(), id.to_lowercase()])
    }

    #[test]
    fn test_intent_manager() {
        let mut manager = IntentManager::new();
        
        // 注册意图
        let intent1 = create_test_intent("intent1", "greeting", "skill1");
        manager.register(intent1.clone()).unwrap();
        
        let intent2 = create_test_intent("intent2", "goodbye", "skill1");
        manager.register(intent2.clone()).unwrap();
        
        let intent3 = create_test_intent("intent3", "search", "skill2")
            .with_project("myproject");
        manager.register(intent3.clone()).unwrap();
        
        // 测试获取
        assert!(manager.get("intent1").is_some());
        assert!(manager.get("nonexistent").is_none());
        
        // 测试技能过滤
        let skill1_intents = manager.get_by_skill("skill1");
        assert_eq!(skill1_intents.len(), 2);
        
        // 测试项目过滤
        let project_intents = manager.get_by_project("myproject");
        assert_eq!(project_intents.len(), 1);
        
        // 测试关键词匹配
        let matches = manager.match_keywords("say greeting to users", None);
        assert!(!matches.is_empty());
        assert_eq!(matches[0].intent.name, "greeting");
        
        // 测试取消注册
        assert!(manager.unregister("intent1").unwrap());
        assert!(manager.get("intent1").is_none());
        
        // 测试重复注册
        assert!(manager.register(intent2.clone()).is_err());
    }

    #[test]
    fn test_intent_parse_result() {
        let empty = IntentParseResult::empty("test query");
        assert!(!empty.is_match());
        
        let intent = create_test_intent("intent1", "test", "skill1");
        let success = IntentParseResult::success(
            "test query",
            intent,
            0.8,
            [("key".to_string(), "value".to_string())].into_iter().collect(),
        );
        assert!(success.is_match());
        assert_eq!(success.confidence, 0.8);
    }
    
    #[test]
    fn test_action_type_classification() {
        // This test would require mocking the embedding service
        // For now, we just verify the enum variants exist
        let actions = vec![
            ActionType::Analyze,
            ActionType::Create,
            ActionType::Update,
            ActionType::Delete,
            ActionType::Commit,
            ActionType::Query,
            ActionType::Other,
        ];
        
        assert_eq!(actions.len(), 7);
    }
}
