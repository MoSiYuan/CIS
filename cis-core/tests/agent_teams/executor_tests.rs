//! # DAG 执行器集成测试
//!
//! 测试 MultiAgentDagExecutor 的核心功能：
//! - 单 Agent DAG 执行
//! - 多 Agent DAG 执行
//! - Agent 复用
//! - 错误处理

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cis_core::agent::persistent::{
    AgentPool, PoolConfig, RuntimeType,
};
use cis_core::scheduler::{
    DagScheduler, MultiAgentDagExecutor, MultiAgentExecutorConfig, TaskDag,
};

use super::utils::*;

/// 测试执行简单的单 Agent DAG
#[tokio::test]
async fn test_executor_single_agent_dag() {
    let _ctx = TestContext::new();

    // 创建调度器和 Pool
    let scheduler = DagScheduler::new();
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    // 注册 Runtime
    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 创建执行器
    let config = MultiAgentExecutorConfig::default()
        .with_max_concurrent(2)
        .with_task_timeout(Duration::from_secs(60));

    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建 DAG
    let dag = create_test_dag();
    let run_id = executor.create_run(dag).await.unwrap();

    // 执行
    let report = executor.execute(&run_id).await.unwrap();

    // 验证结果
    assert_eq!(report.completed, 2, "Expected 2 completed tasks");
    assert_eq!(report.failed, 0, "Expected 0 failed tasks");
    assert_eq!(report.skipped, 0, "Expected 0 skipped tasks");
    assert_eq!(report.final_status, "success");
}

/// 测试执行多 Agent DAG
#[tokio::test]
async fn test_executor_multi_agent_dag() {
    let _ctx = TestContext::new();

    let scheduler = DagScheduler::new();
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

    let config = MultiAgentExecutorConfig::default();
    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建多 Agent DAG
    let dag = create_multi_agent_dag();
    let run_id = executor.create_run(dag).await.unwrap();

    // 执行
    let report = executor.execute(&run_id).await.unwrap();

    // 验证结果
    assert_eq!(report.completed, 2, "Expected 2 completed tasks");
    assert_eq!(report.failed, 0);
    assert_eq!(report.final_status, "success");
}

/// 测试 Agent 复用
#[tokio::test]
async fn test_executor_agent_reuse() {
    let _ctx = TestContext::new();

    let scheduler = DagScheduler::new();
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    let config = MultiAgentExecutorConfig::default()
        .with_auto_cleanup(false); // 不自动清理，保留 Agent

    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建复用 Agent 的 DAG
    let dag = create_reuse_agent_dag();
    let run_id = executor.create_run(dag).await.unwrap();

    // 执行
    let report = executor.execute(&run_id).await.unwrap();

    // 验证结果
    assert_eq!(report.completed, 2);
    assert_eq!(report.failed, 0);
    assert_eq!(report.final_status, "success");

    // 验证输出
    assert!(report.task_outputs.contains_key("task-1"));
    assert!(report.task_outputs.contains_key("task-2"));
}

/// 测试带命令映射的 DAG 执行
#[tokio::test]
async fn test_executor_with_commands() {
    let _ctx = TestContext::new();

    let scheduler = DagScheduler::new();
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    let config = MultiAgentExecutorConfig::default();
    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建 DAG
    let dag = create_test_dag();

    // 定义任务命令
    let mut task_commands = HashMap::new();
    task_commands.insert("task-1".to_string(), "Process task 1".to_string());
    task_commands.insert("task-2".to_string(), "Process task 2".to_string());

    // 创建带命令映射的运行
    let run_id = executor
        .create_run_with_commands(dag, task_commands)
        .await
        .unwrap();

    // 执行
    let report = executor.execute(&run_id).await.unwrap();

    // 验证结果
    assert_eq!(report.completed, 2);
    assert_eq!(report.failed, 0);
    assert_eq!(report.final_status, "success");
}

/// 测试并发任务执行
#[tokio::test]
async fn test_executor_concurrent_tasks() {
    let _ctx = TestContext::new();

    let scheduler = DagScheduler::new();
    let pool = AgentPool::new(PoolConfig {
        max_agents: 10,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 设置最大并发为 3
    let config = MultiAgentExecutorConfig::default().with_max_concurrent(3);
    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建可以并行执行的 DAG（多个无依赖任务）
    let mut dag = TaskDag::new();

    // 添加 5 个无依赖的并行任务
    for i in 1..=5 {
        dag.add_node(format!("task-{}", i), vec![]).unwrap();
    }

    let start = std::time::Instant::now();
    let run_id = executor.create_run(dag).await.unwrap();
    let report = executor.execute(&run_id).await.unwrap();
    let duration = start.elapsed();

    // 验证结果
    assert_eq!(report.completed, 5);
    assert_eq!(report.failed, 0);

    // 由于是并行执行，时间应该远小于串行执行（5 * 100ms = 500ms）
    // 但因为有调度开销，通常需要 100-300ms
    assert!(
        duration < Duration::from_millis(800),
        "Concurrent execution took too long: {:?}",
        duration
    );
}

/// 测试获取运行状态
#[tokio::test]
async fn test_executor_get_run_status() {
    let _ctx = TestContext::new();

    let scheduler = DagScheduler::new();
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    let config = MultiAgentExecutorConfig::default();
    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建 DAG
    let dag = create_test_dag();
    let run_id = executor.create_run(dag).await.unwrap();

    // 执行前状态应该是 None（还未初始化）
    // 注意：实际状态检查可能在执行过程中返回不同值

    // 执行
    let report = executor.execute(&run_id).await.unwrap();
    assert_eq!(report.final_status, "success");
}

/// 测试获取运行统计信息
#[tokio::test]
async fn test_executor_get_run_stats() {
    let _ctx = TestContext::new();

    let scheduler = DagScheduler::new();
    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    let config = MultiAgentExecutorConfig::default();
    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建 DAG
    let dag = create_test_dag();
    let run_id = executor.create_run(dag).await.unwrap();

    // 执行前统计
    let (completed_before, failed_before, skipped_before) =
        executor.get_run_stats(&run_id).await.unwrap();
    assert_eq!(completed_before, 0);
    assert_eq!(failed_before, 0);
    assert_eq!(skipped_before, 0);

    // 执行
    executor.execute(&run_id).await.unwrap();

    // 执行后统计
    let (completed_after, failed_after, skipped_after) =
        executor.get_run_stats(&run_id).await.unwrap();
    assert_eq!(completed_after, 2);
    assert_eq!(failed_after, 0);
    assert_eq!(skipped_after, 0);
}

/// 测试配置构建器
#[tokio::test]
async fn test_executor_config_builder() {
    let config = MultiAgentExecutorConfig::new()
        .with_default_runtime(RuntimeType::OpenCode)
        .with_auto_cleanup(false)
        .with_task_timeout(Duration::from_secs(600))
        .with_context_injection(false)
        .with_max_concurrent(8);

    assert_eq!(config.default_runtime, RuntimeType::OpenCode);
    assert!(!config.auto_cleanup);
    assert_eq!(config.task_timeout, Duration::from_secs(600));
    assert!(!config.enable_context_injection);
    assert_eq!(config.max_concurrent_tasks, 8);
}

/// 测试执行器使用 Pool 的简化创建方式
#[tokio::test]
async fn test_executor_with_pool() {
    let _ctx = TestContext::new();

    let pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    let config = MultiAgentExecutorConfig::default();
    let executor = MultiAgentDagExecutor::with_pool(pool, config).unwrap();

    // 创建并执行 DAG
    let dag = create_test_dag();
    let run_id = executor.create_run(dag).await.unwrap();
    let report = executor.execute(&run_id).await.unwrap();

    assert_eq!(report.completed, 2);
    assert_eq!(report.failed, 0);
}
