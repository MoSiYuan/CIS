//! 错误类型定义

use thiserror::Error;

/// Feishu IM Skill 错误
#[derive(Error, Debug)]
pub enum FeishuImError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("轮询错误: {0}")]
    Polling(String),

    #[error("AI 调用错误: {0}")]
    Ai(String),

    #[error("数据库错误: {0}")]
    Database(String),

    #[error("飞书 API 错误: {0}")]
    FeishuApi(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}
