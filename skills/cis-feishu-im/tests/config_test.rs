//! 配置管理模块测试

use cis_feishu_im::{FeishuImConfig, TriggerMode};
use std::path::PathBuf;

#[test]
fn test_default_config() {
    let config = FeishuImConfig::default();

    assert_eq!(config.trigger_mode, TriggerMode::PrivateAndAtMention);
    assert!(config.verify_signature);
    assert_eq!(config.webhook.port, 8080);
    assert_eq!(config.webhook.bind_address, "0.0.0.0");
    assert_eq!(config.webhook.path, "/webhook/feishu");
    assert!(config.context_config.persist_context);
    assert_eq!(config.context_config.max_turns, 20);
    assert_eq!(config.context_config.context_timeout_secs, 1800);
    assert!(config.context_config.sync_to_memory);
}

#[test]
fn test_trigger_mode_serialization() {
    let modes = vec![
        TriggerMode::AtMentionOnly,
        TriggerMode::PrivateAndAtMention,
        TriggerMode::All,
    ];

    for mode in modes {
        let json = serde_json::to_string(&mode).unwrap();
        let decoded: TriggerMode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, mode);
    }
}

#[test]
fn test_config_serialization() {
    // 测试配置序列化和反序列化
    let config = FeishuImConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let _decoded: FeishuImConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.trigger_mode, TriggerMode::PrivateAndAtMention);
    assert!(config.verify_signature);
}

#[test]
fn test_context_config_default_keywords() {
    let config = FeishuImConfig::default();

    assert!(!config.context_config.memory_keywords.is_empty());
    assert!(config.context_config.memory_keywords.contains(&"记住".to_string()));
    assert!(config.context_config.memory_keywords.contains(&"重要".to_string()));
}

#[test]
fn test_expand_path() {
    use cis_feishu_im::expand_path;

    // Test home directory expansion
    let home = std::env::var("HOME").unwrap();
    let path = PathBuf::from("~/test.db");
    let expanded = expand_path(&path);

    assert!(!expanded.starts_with("~"));
    assert!(expanded.starts_with(&home));

    // Test regular path (no expansion needed)
    let path = PathBuf::from("/var/test.db");
    let expanded = expand_path(&path);
    assert_eq!(expanded, PathBuf::from("/var/test.db"));
}
