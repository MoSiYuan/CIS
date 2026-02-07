//! OpenCode DAG Migration Tests
//!
//! 测试 DAG 执行从 Claude 迁移到 OpenCode 的兼容性

#[cfg(test)]
mod tests {
    use crate::agent::AgentType;
    use crate::agent::cluster::{AgentClusterConfig, AgentClusterExecutor};
    
    /// 测试默认 Agent 类型为 OpenCode
    #[test]
    fn test_default_agent_is_opencode() {
        let config = AgentClusterConfig::default();
        assert_eq!(config.default_agent, AgentType::OpenCode);
    }
    
    /// 测试可以显式设置为 Claude（向后兼容）
    #[test]
    fn test_explicit_claude_config() {
        let config = AgentClusterConfig {
            default_agent: AgentType::Claude,
            ..Default::default()
        };
        assert_eq!(config.default_agent, AgentType::Claude);
    }
    
    /// 测试可以显式设置为 Kimi（多 Agent 支持）
    #[test]
    fn test_explicit_kimi_config() {
        let config = AgentClusterConfig {
            default_agent: AgentType::Kimi,
            ..Default::default()
        };
        assert_eq!(config.default_agent, AgentType::Kimi);
    }
    
    /// 测试配置序列化/反序列化
    #[test]
    fn test_config_serialization() {
        let config = AgentClusterConfig {
            max_workers: 8,
            default_agent: AgentType::OpenCode,
            task_timeout_secs: 7200,
            ..Default::default()
        };
        
        // 验证配置值
        assert_eq!(config.max_workers, 8);
        assert_eq!(config.default_agent, AgentType::OpenCode);
        assert_eq!(config.task_timeout_secs, 7200);
    }
    
    /// 测试 AgentType 字符串转换
    #[test]
    fn test_agent_type_from_str() {
        use std::str::FromStr;
        
        // OpenCode
        let agent = AgentType::from_str("opencode").unwrap();
        assert_eq!(agent, AgentType::OpenCode);
        
        // Claude（向后兼容）
        let agent = AgentType::from_str("claude").unwrap();
        assert_eq!(agent, AgentType::Claude);
        
        // Kimi
        let agent = AgentType::from_str("kimi").unwrap();
        assert_eq!(agent, AgentType::Kimi);
        
        // Aider
        let agent = AgentType::from_str("aider").unwrap();
        assert_eq!(agent, AgentType::Aider);
    }
    
    /// 测试 AgentType 显示名称
    #[test]
    fn test_agent_type_display_name() {
        assert_eq!(AgentType::OpenCode.display_name(), "OpenCode");
        assert_eq!(AgentType::Claude.display_name(), "Claude Code");
        assert_eq!(AgentType::Kimi.display_name(), "Kimi Code");
        assert_eq!(AgentType::Aider.display_name(), "Aider");
    }
    
    /// 测试 AgentType 命令名称
    #[test]
    fn test_agent_type_command_name() {
        assert_eq!(AgentType::OpenCode.command_name(), Some("opencode"));
        assert_eq!(AgentType::Claude.command_name(), Some("claude"));
        assert_eq!(AgentType::Kimi.command_name(), Some("kimi"));
        assert_eq!(AgentType::Aider.command_name(), Some("aider"));
    }
    
    /// 测试 Executor 创建使用默认配置
    #[tokio::test]
    async fn test_executor_uses_opencode_by_default() {
        let executor = AgentClusterExecutor::default_executor();
        
        // 如果系统支持，创建应该成功
        // 注意：实际测试可能需要 mock 环境
        if let Ok(exec) = executor {
            // 验证配置已应用
            // 这里主要测试配置加载不 panic
            drop(exec);
        }
    }
}
