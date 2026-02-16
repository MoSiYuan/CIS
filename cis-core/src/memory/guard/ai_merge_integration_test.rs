//! # AI Merge 集成测试
//!
//! 测试 AIMerge 与冲突解决流程的集成

#[cfg(test)]
mod integration_tests {
    use crate::memory::guard::ai_merge::{AIMerger, AIMergeConfig, AIMergeStrategy};
    use crate::memory::guard::conflict_guard::ConflictVersion;
    use crate::memory::guard::conflict_resolution::{
        apply_resolution_strategy, apply_resolution_strategy_async, ConflictResolutionChoice,
    };

    fn create_test_version(node_id: &str, value: &str, timestamp: i64) -> ConflictVersion {
        ConflictVersion {
            node_id: node_id.to_string(),
            vector_clock: vec![],
            value: value.as_bytes().to_vec(),
            timestamp,
        }
    }

    #[test]
    fn test_ai_merge_config_default() {
        let config = AIMergeConfig::default();
        assert_eq!(config.strategy, AIMergeStrategy::SmartMerge);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_ai_merger_creation() {
        let merger = AIMerger::new(AIMergeConfig::default());
        assert_eq!(merger.config.strategy, AIMergeStrategy::SmartMerge);
    }

    #[test]
    fn test_ai_merger_default() {
        let merger = AIMerger::default();
        assert_eq!(merger.config.strategy, AIMergeStrategy::SmartMerge);
    }

    #[test]
    fn test_sync_aimerge_falls_back_to_keep_local() {
        let local = create_test_version("node-a", "local value", 1000);
        let remote = create_test_version("node-b", "remote value", 2000);

        // 同步版本的 AIMerge 应该回退到 KeepLocal
        let result = apply_resolution_strategy(
            &ConflictResolutionChoice::AIMerge,
            &local,
            &[remote],
            "test/key",
            &[],
        )
        .unwrap();

        assert_eq!(result, b"local value");
    }

    #[tokio::test]
    async fn test_async_aimerge_without_merger_falls_back() {
        let local = create_test_version("node-a", "local value", 1000);
        let remote = create_test_version("node-b", "remote value", 2000);

        // 异步版本，但没有提供 AI Merger，应该回退到 KeepLocal
        let result = apply_resolution_strategy_async(
            &ConflictResolutionChoice::AIMerge,
            &local,
            &[remote],
            "test/key",
            None, // 没有 AI Merger
        )
        .await
        .unwrap();

        assert_eq!(result, b"local value");
    }

    #[tokio::test]
    async fn test_async_keep_local_still_works() {
        let local = create_test_version("node-a", "local value", 1000);

        let result = apply_resolution_strategy_async(
            &ConflictResolutionChoice::KeepLocal,
            &local,
            &[],
            "test/key",
            None,
        )
        .await
        .unwrap();

        assert_eq!(result, b"local value");
    }

    #[tokio::test]
    async fn test_async_keep_remote_still_works() {
        let local = create_test_version("node-a", "local value", 1000);
        let remote = create_test_version("node-b", "remote value", 2000);

        let result = apply_resolution_strategy_async(
            &ConflictResolutionChoice::KeepRemote {
                node_id: "node-b".to_string(),
            },
            &local,
            &[remote],
            "test/key",
            None,
        )
        .await
        .unwrap();

        assert_eq!(result, b"remote value");
    }

    #[test]
    fn test_merge_strategies_are_distinct() {
        let smart_config = AIMergeConfig {
            strategy: AIMergeStrategy::SmartMerge,
            ..Default::default()
        };
        let content_config = AIMergeConfig {
            strategy: AIMergeStrategy::ContentBased,
            ..Default::default()
        };
        let time_config = AIMergeConfig {
            strategy: AIMergeStrategy::TimeBased,
            ..Default::default()
        };

        // 验证不同的策略产生不同的系统提示词
        let smart_merger = AIMerger::new(smart_config);
        let content_merger = AIMerger::new(content_config);
        let time_merger = AIMerger::new(time_config);

        let smart_prompt = smart_merger.build_system_prompt();
        let content_prompt = content_merger.build_system_prompt();
        let time_prompt = time_merger.build_system_prompt();

        assert!(smart_prompt.contains("memory conflict resolution specialist"));
        assert!(content_prompt.contains("data merge specialist"));
        assert!(time_prompt.contains("time-based merge specialist"));

        // 验证它们是不同的
        assert_ne!(smart_prompt, content_prompt);
        assert_ne!(smart_prompt, time_prompt);
        assert_ne!(content_prompt, time_prompt);
    }

    #[test]
    fn test_merge_prompt_construction() {
        let merger = AIMerger::default();

        let local = create_test_version("node-a", "local value", 1000);
        let remote1 = create_test_version("node-b", "remote value 1", 2000);
        let remote2 = create_test_version("node-c", "remote value 2", 3000);

        let prompt = merger.build_merge_prompt("test/key", &local, &[remote1, remote2]);

        // 验证 prompt 包含所有必要信息
        assert!(prompt.contains("test/key"));
        assert!(prompt.contains("node-a"));
        assert!(prompt.contains("node-b"));
        assert!(prompt.contains("node-c"));
        assert!(prompt.contains("local value"));
        assert!(prompt.contains("remote value 1"));
        assert!(prompt.contains("remote value 2"));
        assert!(prompt.contains("1000"));
        assert!(prompt.contains("2000"));
        assert!(prompt.contains("3000"));
        assert!(prompt.contains("Remote Version 1"));
        assert!(prompt.contains("Remote Version 2"));
    }

    #[test]
    fn test_parse_ai_response_cleans_markdown() {
        let merger = AIMerger::default();

        // 测试普通响应
        let response1 = "merged value";
        let parsed1 = merger.parse_ai_response(response1).unwrap();
        assert_eq!(parsed1, b"merged value");

        // 测试带 markdown 代码块的响应
        let response2 = "```\nmerged value\n```";
        let parsed2 = merger.parse_ai_response(response2).unwrap();
        assert_eq!(parsed2, b"merged value");

        // 测试带 JSON 标记的响应
        let response3 = "```json\n{\"key\": \"value\"}\n```";
        let parsed3 = merger.parse_ai_response(response3).unwrap();
        assert_eq!(parsed3, b"{\"key\": \"value\"}");

        // 测试带额外空格的响应
        let response4 = "   \n  merged value  \n   ";
        let parsed4 = merger.parse_ai_response(response4).unwrap();
        assert_eq!(parsed4, b"merged value");
    }
}
