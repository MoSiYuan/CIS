//! AI 调用接口
//!
//! 提供统一的 AI 调用封装

use serde::{Deserialize, Serialize};

/// 消息角色
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// 对话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

/// AI 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRequest {
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

/// AI 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<serde_json::Value>,
    pub usage: TokenUsage,
}

/// Token 使用量
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt: u32,
    pub completion: u32,
    pub total: u32,
}

/// AI 工具/函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
    pub id: String,
}

/// AI 调用便捷方法
pub struct Ai;

impl Ai {
    /// 简单对话
    pub fn chat(prompt: &str) -> crate::error::Result<String> {
        crate::host::Host::ai_chat(prompt)
    }
    
    /// 生成结构化数据
    pub fn generate_json(
        prompt: &str,
        schema: &str,
    ) -> crate::error::Result<serde_json::Value> {
        crate::host::Host::ai_generate_json(prompt, schema)
    }
    
    /// 摘要生成
    pub fn summarize(text: &str, max_length: usize) -> crate::error::Result<String> {
        let prompt = format!(
            "请将以下文本总结为不超过 {} 个字的摘要:\n\n{}",
            max_length, text
        );
        Self::chat(&prompt)
    }
    
    /// 提取关键词
    pub fn extract_keywords(text: &str, count: usize) -> crate::error::Result<Vec<String>> {
        let prompt = format!(
            "请从以下文本中提取 {} 个关键词，用逗号分隔:\n\n{}",
            count, text
        );
        let result = Self::chat(&prompt)?;
        Ok(result
            .split([',', '，', '\n'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }
    
    /// 情感分析
    pub fn sentiment(text: &str) -> crate::error::Result<Sentiment> {
        let prompt = format!(
            r#"请分析以下文本的情感，返回 JSON: {{"sentiment": "positive|negative|neutral", "score": 0.0-1.0}}

文本: {}"#,
            text
        );
        let schema = r#"{"type":"object","properties":{"sentiment":{"type":"string","enum":["positive","negative","neutral"]},"score":{"type":"number"}},"required":["sentiment","score"]}"#;
        let result = crate::host::Host::ai_generate_json(&prompt, schema)?;
        
        let sentiment = result
            .get("sentiment")
            .and_then(|v| v.as_str())
            .unwrap_or("neutral");
            
        let score = result
            .get("score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
            
        Ok(Sentiment {
            polarity: match sentiment {
                "positive" => SentimentPolarity::Positive,
                "negative" => SentimentPolarity::Negative,
                _ => SentimentPolarity::Neutral,
            },
            score: score as f32,
        })
    }
    
    /// 分类
    pub fn classify(text: &str, categories: &[&str]) -> crate::error::Result<String> {
        let categories_str = categories.join(", ");
        let prompt = format!(
            "请将以下文本分类到 [{}] 中的一个类别:\n\n{}",
            categories_str, text
        );
        let result = Self::chat(&prompt)?;
        // 找到匹配的类别
        for cat in categories {
            if result.to_lowercase().contains(&cat.to_lowercase()) {
                return Ok(cat.to_string());
            }
        }
        Ok(categories.first().unwrap_or(&"unknown").to_string())
    }
}

/// 情感分析结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SentimentPolarity {
    Positive,
    Negative,
    Neutral,
}

/// 情感
#[derive(Debug, Clone, Copy)]
pub struct Sentiment {
    pub polarity: SentimentPolarity,
    pub score: f32, // 0.0 - 1.0
}

impl Sentiment {
    pub fn is_positive(&self) -> bool {
        self.polarity == SentimentPolarity::Positive
    }
    
    pub fn is_negative(&self) -> bool {
        self.polarity == SentimentPolarity::Negative
    }
}
