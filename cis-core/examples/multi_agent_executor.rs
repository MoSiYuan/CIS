//! # MultiAgentDagExecutor 使用示例
//!
//! 演示如何使用 MultiAgentDagExecutor 执行多 Agent DAG 任务。

use cis_core::agent::persistent::{AgentPool, PoolConfig, RuntimeType};
use cis_core::scheduler::{
    DagNode, MultiAgentDagExecutor, MultiAgentExecutorConfig, RuntimeType as SchedulerRuntimeType,
    TaskDag,
};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== MultiAgentDagExecutor 使用示例 ===\n");

    // 1. 创建 Agent Pool
    let pool_config = PoolConfig {
        max_agents: 10,
        default_timeout: Duration::from_secs(300),
        health_check_interval: Duration::from_secs(30),
        auto_cleanup: true,
        idle_timeout: Duration::from_secs(600),
    };
    let agent_pool = AgentPool::new(pool_config);

    // 2. 创建执行器配置
    let executor_config = MultiAgentExecutorConfig::new()
        .with_default_runtime(RuntimeType::Claude)
        .with_auto_cleanup(true)
        .with_task_timeout(Duration::from_secs(300))
        .with_context_injection(true)
        .with_max_concurrent(4);

    // 3. 创建 MultiAgentDagExecutor
    let executor = MultiAgentDagExecutor::with_pool(agent_pool, executor_config)?;

    println!("✓ 执行器创建成功\n");

    // 4. 创建示例 DAG
    let mut dag = TaskDag::new();

    // 添加任务节点
    // 任务1: 代码分析（使用 Claude）
    let mut node1 = DagNode::new("analyze".to_string(), vec![]);
    node1.agent_runtime = Some(SchedulerRuntimeType::Claude);
    node1.keep_agent = false;
    dag.add_node_with_level("analyze".to_string(), vec![], node1.level)?;

    // 任务2: 代码重构（复用 Claude Agent）
    let mut node2 = DagNode::new("refactor".to_string(), vec!["analyze".to_string()]);
    node2.agent_runtime = Some(SchedulerRuntimeType::Claude);
    node2.reuse_agent = Some("claude-analyzer".to_string());
    dag.add_node_with_level("refactor".to_string(), vec!["analyze".to_string()], node2.level)?;

    // 任务3: 测试生成（使用 Kimi）
    let mut node3 = DagNode::new("generate_tests".to_string(), vec!["refactor".to_string()]);
    node3.agent_runtime = Some(SchedulerRuntimeType::Kimi);
    node3.keep_agent = true;
    dag.add_node_with_level(
        "generate_tests".to_string(),
        vec!["refactor".to_string()],
        node3.level,
    )?;

    // 任务4: 文档生成（使用 OpenCode，并行执行）
    let mut node4 = DagNode::new("generate_docs".to_string(), vec!["refactor".to_string()]);
    node4.agent_runtime = Some(SchedulerRuntimeType::OpenCode);
    dag.add_node_with_level(
        "generate_docs".to_string(),
        vec!["refactor".to_string()],
        node4.level,
    )?;

    // 任务5: 最终审查（依赖测试和文档）
    let mut node5 = DagNode::new("review".to_string(), vec!["generate_tests".to_string(), "generate_docs".to_string()]);
    node5.agent_runtime = Some(SchedulerRuntimeType::Claude);
    dag.add_node_with_level(
        "review".to_string(),
        vec!["generate_tests".to_string(), "generate_docs".to_string()],
        node5.level,
    )?;

    println!("✓ DAG 创建成功: {} 个任务", dag.node_count());
    println!("  - analyze (Claude): 分析代码");
    println!("  - refactor (Claude, 复用): 重构代码");
    println!("  - generate_tests (Kimi): 生成测试");
    println!("  - generate_docs (OpenCode): 生成文档");
    println!("  - review (Claude): 最终审查\n");

    // 5. 创建任务命令映射
    let mut task_commands = HashMap::new();
    task_commands.insert("analyze".to_string(), "分析项目代码结构，识别关键模块".to_string());
    task_commands.insert("refactor".to_string(), "根据分析结果重构代码".to_string());
    task_commands.insert("generate_tests".to_string(), "为重构后的代码生成单元测试".to_string());
    task_commands.insert("generate_docs".to_string(), "生成 API 文档".to_string());
    task_commands.insert("review".to_string(), "审查测试和文档质量".to_string());

    // 6. 创建运行
    let run_id = executor.create_run_with_commands(dag, task_commands).await?;
    println!("✓ DAG 运行已创建: {}\n", run_id);

    // 7. 执行 DAG（这里仅演示，实际需要注册 Runtime）
    println!("开始执行 DAG...");
    println!("注意: 执行需要预先注册相应的 Agent Runtime\n");

    // 示例：获取运行状态
    if let Ok(Some(status)) = executor.get_run_status(&run_id).await {
        println!("运行状态: {:?}", status);
    }

    // 示例：获取运行统计
    if let Ok((completed, failed, skipped)) = executor.get_run_stats(&run_id).await {
        println!("任务统计: {} 完成, {} 失败, {} 跳过", completed, failed, skipped);
    }

    println!("\n=== 示例完成 ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_config() {
        let config = MultiAgentExecutorConfig::new()
            .with_default_runtime(RuntimeType::Claude)
            .with_max_concurrent(8);

        assert_eq!(config.max_concurrent_tasks, 8);
        assert!(config.enable_context_injection);
    }

    #[test]
    fn test_dag_construction() {
        let mut dag = TaskDag::new();
        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.add_node("task2".to_string(), vec!["task1".to_string()]).unwrap();

        assert_eq!(dag.node_count(), 2);
        assert_eq!(dag.root_nodes().len(), 1);
    }
}
