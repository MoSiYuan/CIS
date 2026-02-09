//! # Agent Pool 集成测试
//!
//! 测试 Agent Pool 的核心功能：
//! - Agent 获取和释放
//! - Agent 复用
//! - 并发获取
//! - 生命周期管理

use std::sync::Arc;
use std::time::Duration;

use cis_core::agent::persistent::{
    AgentAcquireConfig, AgentPool, PoolConfig, RuntimeType,
};

use super::utils::*;

/// 测试基本获取和释放
#[tokio::test]
async fn test_pool_acquire_and_release() {
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    // 注册 Mock Runtime
    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 获取 Agent
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("test-agent"));

    let agent = pool.acquire(config).await.unwrap();
    // 验证获取到了 Agent（通过 ID 不为空）
    assert!(!agent.agent_id().is_empty());

    // 执行任务
    let result = agent
        .execute(test_task_request("test-task", "Hello"))
        .await
        .unwrap();
    assert!(result.success);

    // 释放 Agent (不保留)
    pool.release(agent, false).await.unwrap();

    // 验证 Agent 已被移除
    assert_eq!(pool.agent_count().await, 0);
}

/// 测试 Agent 复用
#[tokio::test]
async fn test_pool_agent_reuse() {
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 获取 Agent
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("test-agent"));

    let agent1 = pool.acquire(config).await.unwrap();
    let agent_id = agent1.agent_id().to_string();

    // 释放但保持
    pool.release(agent1, true).await.unwrap();

    // 验证 Agent 仍在池中
    assert_eq!(pool.agent_count().await, 1);

    // 复用同一个 Agent
    let reuse_config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_reuse_agent_id(&agent_id);

    let agent2 = pool.acquire(reuse_config).await.unwrap();
    assert_eq!(agent2.agent_id(), agent_id);

    // 清理
    pool.release(agent2, false).await.unwrap();
}

/// 测试并发获取
#[tokio::test]
async fn test_pool_concurrent_acquire() {
    let pool = Arc::new(AgentPool::new(PoolConfig {
        max_agents: 10,
        ..Default::default()
    }));

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 并发获取多个 Agent
    let mut handles = vec![];
    for i in 0..5 {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            let config = AgentAcquireConfig::new(RuntimeType::Claude)
                .with_agent_config(test_agent_config(format!("agent-{}", i)));
            pool.acquire(config).await
        });
        handles.push(handle);
    }

    // 等待所有获取完成
    let mut agents = vec![];
    for handle in handles {
        let agent = handle.await.unwrap().unwrap();
        assert!(!agent.agent_id().is_empty());
        agents.push(agent);
    }

    // 验证所有 Agent 都在池中
    assert_eq!(pool.agent_count().await, 5);

    // 清理
    for agent in agents {
        pool.release(agent, false).await.unwrap();
    }
}

/// 测试池大小限制
#[tokio::test]
async fn test_pool_limit() {
    let pool = Arc::new(AgentPool::new(PoolConfig {
        max_agents: 3,
        ..Default::default()
    }));

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 获取最大数量的 Agent
    let mut agents = vec![];
    for i in 0..3 {
        let config = AgentAcquireConfig::new(RuntimeType::Claude)
            .with_agent_config(test_agent_config(format!("agent-{}", i)));
        let agent = pool.acquire(config).await.unwrap();
        agents.push(agent);
    }

    // 尝试获取第四个 Agent（应该失败）
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("overflow-agent"));
    let result = pool.acquire(config).await;
    assert!(result.is_err());

    // 释放一个 Agent
    let agent = agents.pop().unwrap();
    pool.release(agent, false).await.unwrap();

    // 现在可以获取新的 Agent
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("new-agent"));
    let new_agent = pool.acquire(config).await.unwrap();
    agents.push(new_agent);

    // 清理
    for agent in agents {
        pool.release(agent, false).await.unwrap();
    }
}

/// 测试列出 Agent
#[tokio::test]
async fn test_pool_list_agents() {
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 初始为空
    let list = pool.list().await;
    assert!(list.is_empty());

    // 获取两个 Agent
    let config1 = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("agent-1"));
    let agent1 = pool.acquire(config1).await.unwrap();

    let config2 = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("agent-2"));
    let agent2 = pool.acquire(config2).await.unwrap();

    // 验证列表
    let list = pool.list().await;
    assert_eq!(list.len(), 2);

    // 清理
    pool.release(agent1, false).await.unwrap();
    pool.release(agent2, false).await.unwrap();
}

/// 测试强制终止 Agent
#[tokio::test]
async fn test_pool_kill_agent() {
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 获取 Agent
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("test-agent"));
    let agent = pool.acquire(config).await.unwrap();
    let agent_id = agent.agent_id().to_string();

    // 释放（保留）
    pool.release(agent, true).await.unwrap();
    assert_eq!(pool.agent_count().await, 1);

    // 强制终止
    pool.kill(&agent_id).await.unwrap();
    assert_eq!(pool.agent_count().await, 0);

    // 终止不存在的 Agent 应该失败
    let result = pool.kill(&agent_id).await;
    assert!(result.is_err());
}

/// 测试健康检查任务可以正常启动和停止
#[tokio::test]
async fn test_pool_health_check_lifecycle() {
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        health_check_interval: Duration::from_millis(50),
        auto_cleanup: false,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 启动健康检查
    pool.start_health_check().await;

    // 创建一个 Agent
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("test-agent"));
    let agent = pool.acquire(config).await.unwrap();
    pool.release(agent, true).await.unwrap();

    // 等待健康检查运行几次
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Agent 应该仍然存在（auto_cleanup=false）
    assert_eq!(pool.agent_count().await, 1);

    // 停止健康检查
    pool.stop_health_check().await;

    // 清理
    pool.shutdown_all().await.unwrap();
}

/// 测试多种 Runtime 类型
#[tokio::test]
async fn test_pool_multiple_runtimes() {
    let pool = AgentPool::new(PoolConfig {
        max_agents: 10,
        ..Default::default()
    });

    // 注册多个 Runtime
    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();
    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::OpenCode)))
        .await
        .unwrap();
    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Kimi)))
        .await
        .unwrap();

    // 获取不同类型的 Agent
    let claude_config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("claude-agent"));
    let claude_agent = pool.acquire(claude_config).await.unwrap();
    assert!(!claude_agent.agent_id().is_empty());

    let opencode_config = AgentAcquireConfig::new(RuntimeType::OpenCode)
        .with_agent_config(test_agent_config("opencode-agent"));
    let opencode_agent = pool.acquire(opencode_config).await.unwrap();
    assert!(!opencode_agent.agent_id().is_empty());

    let kimi_config = AgentAcquireConfig::new(RuntimeType::Kimi)
        .with_agent_config(test_agent_config("kimi-agent"));
    let kimi_agent = pool.acquire(kimi_config).await.unwrap();
    assert!(!kimi_agent.agent_id().is_empty());

    // 清理
    pool.release(claude_agent, false).await.unwrap();
    pool.release(opencode_agent, false).await.unwrap();
    pool.release(kimi_agent, false).await.unwrap();
}

/// 测试获取不存在的 Agent 会创建新的
#[tokio::test]
async fn test_pool_reuse_nonexistent_agent() {
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 尝试复用不存在的 Agent
    let config = AgentAcquireConfig::new(RuntimeType::Claude)
        .with_reuse_agent_id("non-existent-agent")
        .with_agent_config(test_agent_config("fallback-agent"));

    // 应该创建新的 Agent
    let agent = pool.acquire(config).await.unwrap();
    assert_ne!(agent.agent_id(), "non-existent-agent");

    // 清理
    pool.release(agent, false).await.unwrap();
}
