//! 配置管理模块
//!
//! 定义 FeishuImSkill 的配置结构

use cis_core::ai::{AiProviderConfig, ProviderType};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::poller::PollingConfig;

/// 飞书 IM Skill 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuImConfig {
    /// 飞书 App ID
    pub app_id: String,

    /// 飞书 App Secret（用于 API 调用）
    pub app_secret: String,

    /// 对话触发模式
    pub trigger_mode: TriggerMode,

    /// AI Provider 配置
    pub ai_provider: AiProviderConfig,

    /// 对话上下文配置
    pub context_config: ContextConfig,

    /// IM 数据库路径（feishu_im.db）
    pub im_db_path: PathBuf,

    /// 记忆数据库路径（memory.db - 只读，由 cis-core 管理）
    pub memory_db_path: PathBuf,

    /// 消息轮询配置
    pub polling: PollingConfig,
}

/// 对话触发模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// 仅 @ 机器人时响应
    AtMentionOnly,
    /// 私聊自动响应 + @机器人
    PrivateAndAtMention,
    /// 所有消息都响应
    All,
}

/// 对话上下文配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// 是否持久化对话历史到 IM 数据库
    pub persist_context: bool,

    /// 最大对话轮次（超过后清空上下文）
    pub max_turns: usize,

    /// 上下文超时时间（秒）
    pub context_timeout_secs: u64,

    /// 是否将用户消息同步到记忆系统
    pub sync_to_memory: bool,

    /// 同步到记忆的关键词列表
    pub memory_keywords: Vec<String>,
}

impl Default for FeishuImConfig {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            app_secret: String::new(),
            trigger_mode: TriggerMode::PrivateAndAtMention,
            ai_provider: AiProviderConfig::default(),
            context_config: ContextConfig::default(),
            im_db_path: PathBuf::from("~/.cis/data/feishu_im.db"),
            memory_db_path: PathBuf::from("~/.cis/data/memory.db"),
            polling: PollingConfig::default(),
        }
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            persist_context: true,
            max_turns: 20,
            context_timeout_secs: 1800, // 30 分钟
            sync_to_memory: true,
            memory_keywords: vec![
                "记住".to_string(),
                "重要".to_string(),
                "笔记".to_string(),
                "总结".to_string(),
                "任务".to_string(),
                "计划".to_string(),
            ],
        }
    }
}

/// 从文件加载配置
pub fn load_config_from_file(path: &str) -> Result<FeishuImConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: FeishuImConfig = toml::from_str(&content)?;
    Ok(config)
}

/// 保存配置到文件
pub fn save_config_to_file(
    config: &FeishuImConfig,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = toml::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// 展开路径中的 ~
pub fn expand_path(path: &PathBuf) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home) = std::env::var("HOME").ok() {
            return PathBuf::from(path.to_str().unwrap().replace("~", &home));
        }
    }
    path.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FeishuImConfig::default();
        assert_eq!(config.trigger_mode, TriggerMode::PrivateAndAtMention);
        assert_eq!(config.polling.http_interval, 10);
        assert_eq!(config.polling.batch_size, 20);
    }

    #[test]
    fn test_trigger_mode_serialization() {
        let mode = TriggerMode::PrivateAndAtMention;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"private_and_at_mention\"");

        let decoded: TriggerMode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, mode);
    }

    #[test]
    fn test_expand_path() {
        let path = PathBuf::from("~/test.db");
        let expanded = expand_path(&path);
        assert!(!expanded.starts_with("~"));
    }
}
