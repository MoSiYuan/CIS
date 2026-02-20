# TASK 7.7: 多 Agent 集成测试

> **Phase**: 7 - 多 Agent 架构 (P3 可选)
> **状态**: 🔄 进行中 (占位符已创建)
> **负责人**: TBD
> **周期**: Week 18

---

## 任务概述

编写端到端测试验证多 Agent 系统的完整功能，包括 Agent 协作、P2P 通信、DAG 编排和记忆隔离。

## 工作内容

### 1. 测试框架搭建

**目录结构**:

```
cis-core/tests/
├── integration/
│   ├── mod.rs
│   ├── agent_tests.rs          # Agent 相关测试
│   ├── dag_tests.rs            # DAG 编排测试
│   ├── p2p_tests.rs            # P2P 通信测试
│   └── memory_tests.rs         # 记忆隔离测试
└── e2e/
    ├── multi_agent_workflow.rs # 完整工作流测试
    └── cross_device_test.rs    # 跨设备测试
```

**依赖配置**: `cis-core/Cargo.toml`

```toml
[dev-dependencies]
# 现有测试依赖...
tokio-test = "0.4"
criterion = { version = "0.5", features = ["async_tokio"] }

# 集成测试依赖
serial_test = "3.0"  # 串行测试（避免并发问题）
tempfile = "3"
```

### 2. Agent 协作测试

**文件**: `cis-core/tests/integration/agent_tests.rs`

```rust
use cis_core::agent::{Agent, AgentPool, AgentType, ReceptionistAgent, CoderAgent};
use cis_core::scheduler::{Task, TaskLevel};

#[tokio::test]
#[serial_test::serial]
async fn test_receptionist_to_coder_handoff() {
    // 1. 创建 Agent Pool
    let pool = AgentPool::new_test().await;
    
    // 2. 获取 Receptionist Agent
    let mut receptionist = pool.acquire(AgentType::Receptionist).await.unwrap();
    
    // 3. 用户请求："帮我写一个函数"
    let user_message = "帮我写一个快速排序函数";
    let response = receptionist.turn(user_message).await.unwrap();
    
    // 4. 验证 Receptionist 识别任务类型并委托给 Coder
    assert!(response.contains("我来帮你写代码"));
    assert!(response.contains("快速排序"));
    
    // 5. 验证 Coder Agent 被调用
    let coder_tasks = pool.get_completed_tasks(AgentType::Coder).await;
    assert!(!coder_tasks.is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn test_multi_agent_collaboration() {
    // 测试 Receptionist → Coder → Doc 完整协作流程
    
    let pool = AgentPool::new_test().await;
    
    // 1. Receptionist 分析任务
    let mut receptionist = pool.acquire(AgentType::Receptionist).await.unwrap();
    let task = "帮我实现一个 REST API 并生成文档";
    
    let plan = receptionist.turn(task).await.unwrap();
    assert!(plan.contains("API 实现"));
    assert!(plan.contains("文档生成"));
    
    // 2. Coder Agent 执行代码编写
    let mut coder = pool.acquire(AgentType::Coder).await.unwrap();
    let code = coder.turn("实现上面的 API").await.unwrap();
    assert!(code.contains("async fn"));
    assert!(code.contains("pub struct"));
    
    // 3. Doc Agent 生成文档
    let mut doc = pool.acquire(AgentType::Doc).await.unwrap();
    let docs = doc.turn(&format!("为以下代码生成文档:\n{}", code)).await.unwrap();
    assert!(docs.contains("#"));
    assert!(docs.contains("API"));
}
```

### 3. DAG 编排集成测试

**文件**: `cis-core/tests/integration/dag_tests.rs`

```rust
use cis_core::scheduler::{DagScheduler, Task, TaskLevel};
use cis_core::agent::{Agent, AgentPool};

#[tokio::test]
#[serial_test::serial]
async fn test_dag_with_agents() {
    // 1. 创建 DAG
    let mut scheduler = DagScheduler::new_test().await;
    
    // 2. 定义任务（四级决策）
    let task1 = Task::new("task-1".into(), "代码审查".into(), "code-review".into())
        .with_skill("ai-review")
        .with_level(TaskLevel::Confirmed);
    
    let task2 = Task::new("task-2".into(), "运行测试".into(), "test".into())
        .with_skill("cargo-test")
        .with_level(TaskLevel::Mechanical { retry: 3 });
    
    let task3 = Task::new("task-3".into(), "部署".into(), "deploy".into())
        .with_skill("deploy")
        .with_level(TaskLevel::Recommended {
            default_action: Action::Execute,
            timeout_secs: 300,
        });
    
    // 3. 构建依赖关系
    task2.dependencies.push(task1.id.clone());
    task3.dependencies.push(task2.id.clone());
    
    // 4. 执行 DAG
    let dag = scheduler.build_dag(vec![task1, task2, task3]).await.unwrap();
    let result = scheduler.execute_dag(dag).await.unwrap();
    
    // 5. 验证结果
    assert!(matches!(result.status, ExecutionStatus::Completed));
    assert_eq!(result.results.len(), 3);
}

#[tokio::test]
#[serial_test::serial]
async fn test_dag_parallel_execution() {
    // 测试 DAG 并行执行
    
    let mut scheduler = DagScheduler::new_test().await;
    
    // 创建并行任务（无依赖）
    let tasks: Vec<Task> = (0..5).map(|i| {
        Task::new(
            format!("task-{}", i),
            format!("并行任务 {}", i),
            "parallel".into(),
        ).with_skill("test-skill")
    }).collect();
    
    let dag = scheduler.build_dag(tasks).await.unwrap();
    let start = std::time::Instant::now();
    let result = scheduler.execute_dag(dag).await.unwrap();
    let duration = start.elapsed();
    
    // 验证并行执行（5 个任务应该在 ~1 个任务时间内完成）
    assert!(matches!(result.status, ExecutionStatus::Completed));
    assert!(duration < std::time::Duration::from_secs(10));
}
```

### 4. 记忆隔离测试

**文件**: `cis-core/tests/integration/memory_tests.rs`

```rust
use cis_core::memory::{MemoryService, MemoryDomain, MemoryCategory};
use cis_core::agent::{Agent, AgentType};

#[tokio::test]
#[serial_test::serial]
async fn test_agent_memory_isolation() {
    // 测试 Agent 之间的记忆隔离
    
    let memory = MemoryService::new_test().await;
    
    // 1. Coder Agent 存储代码片段到私域
    memory.set_with_embedding(
        "agent/coder/snippet-1",
        b"fn hello() { println!(\"Hello\"); }",
        MemoryDomain::Private,
        MemoryCategory::Context,
    ).await.unwrap();
    
    // 2. Doc Agent 不应该访问到 Coder 的私域记忆
    let coder_memory = memory.list_keys(Some(MemoryDomain::Private), None, Some("agent/coder")).await.unwrap();
    assert_eq!(coder_memory.len(), 1);
    
    let doc_memory = memory.list_keys(Some(MemoryDomain::Private), None, Some("agent/doc")).await.unwrap();
    assert_eq!(doc_memory.len(), 0);
    
    // 3. 公域记忆应该可被所有 Agent 访问
    memory.set_with_embedding(
        "project/architecture",
        b"微服务架构",
        MemoryDomain::Public,
        MemoryCategory::Context,
    ).await.unwrap();
    
    let public_memory = memory.list_keys(Some(MemoryDomain::Public), None, None).await.unwrap();
    assert!(public_memory.contains(&"project/architecture".to_string()));
}

#[tokio::test]
#[serial_test::serial]
async fn test_memory_hallucination_reduction() {
    // 测试记忆系统降低 Agent 幻觉
    
    let memory = MemoryService::new_test().await;
    let mut agent = CoderAgent::new_test(memory.clone()).await;
    
    // 1. 预先存储项目约定
    memory.set_with_embedding(
        "project/conventions",
        b"使用 Rust 异步编程，错误处理使用 anyhow",
        MemoryDomain::Public,
        MemoryCategory::Context,
    ).await.unwrap();
    
    // 2. Agent 查询约定
    let conventions = memory.semantic_search("项目编程约定", 5, 0.7).await.unwrap();
    assert!(!conventions.is_empty());
    
    // 3. Agent 基于记忆生成代码（验证不违背约定）
    let code = agent.turn("写一个错误处理函数").await.unwrap();
    assert!(code.contains("anyhow::Result") || code.contains("?"));
}
```

### 5. P2P 跨设备测试

**文件**: `cis-core/tests/integration/p2p_tests.rs`

```rust
use cis_core::p2p::{P2PNetwork, PeerId};
use cis_core::agent::{Agent, AgentPool, AgentType};

#[tokio::test]
#[serial_test::serial]
async fn test_cross_device_agent_call() {
    // 创建两个 P2P 节点
    let node1 = P2PNetwork::new_test_port(7677).await;
    let node2 = P2PNetwork::new_test_port(7678).await;
    
    // 节点 1 注册 Coder Agent
    let pool1 = AgentPool::new_with_network(node1.clone()).await;
    pool1.register_local_agent(CoderAgent::new_test().await).await;
    
    // 节点 2 通过 P2P 调用节点 1 的 Agent
    let pool2 = AgentPool::new_with_network(node2.clone()).await;
    let mut remote_agent = pool2.acquire(AgentType::Coder).await.unwrap();
    
    let response = remote_agent.turn("写一个快排函数").await.unwrap();
    assert!(response.contains("fn"));
    assert!(response.contains("quick"));
    
    // 验证是远程调用
    assert_eq!(remote_agent.runtime_type(), RuntimeType::Remote);
}

#[tokio::test]
#[serial_test::serial]
async fn test_agent_discovery() {
    // 测试 Agent 自动发现
    
    let node1 = P2PNetwork::new_test_port(7679).await;
    let node2 = P2PNetwork::new_test_port(7680).await;
    
    // 节点 1 注册多个 Agent
    let pool1 = AgentPool::new_with_network(node1.clone()).await;
    pool1.register_local_agent(CoderAgent::new_test().await).await;
    pool1.register_local_agent(DocAgent::new_test().await).await;
    
    // 节点 2 发现远程 Agents
    let pool2 = AgentPool::new_with_network(node2.clone()).await;
    pool2.discovery.refresh().await.unwrap();
    
    let available = pool2.available_types();
    assert!(available.contains(&AgentType::Coder));
    assert!(available.contains(&AgentType::Doc));
}
```

### 6. 端到端工作流测试

**文件**: `cis-core/tests/e2e/multi_agent_workflow.rs`

```rust
use cis_core::agent::{Agent, AgentPool, AgentType};
use cis_core::scheduler::{DagScheduler, Task, TaskLevel};
use cis_core::memory::{MemoryService, MemoryDomain, MemoryCategory};

#[tokio::test]
#[serial_test::serial]
async fn test_complete_development_workflow() {
    // 完整开发流程：需求分析 → 编码 → 测试 → 文档
    
    // 1. 初始化服务
    let memory = MemoryService::new_test().await;
    let pool = AgentPool::new_with_memory(memory.clone()).await;
    let mut scheduler = DagScheduler::new_with_pool(pool.clone()).await;
    
    // 2. 预加载项目上下文
    memory.set_with_embedding(
        "project/tech-stack",
        b"Rust + Axum + SQLite",
        MemoryDomain::Public,
        MemoryCategory::Context,
    ).await.unwrap();
    
    // 3. 定义 DAG
    let task_analyze = Task::new(
        "analyze".into(),
        "分析需求：实现用户 CRUD API".into(),
        "requirement".into(),
    ).with_level(TaskLevel::Mechanical { retry: 1 });
    
    let task_code = Task::new(
        "code".into(),
        "编写 API 代码".into(),
        "code".into(),
    ).with_level(TaskLevel::Confirmed)
        .with_dependency("analyze");
    
    let task_test = Task::new(
        "test".into(),
        "编写并运行测试".into(),
        "test".into(),
    ).with_level(TaskLevel::Mechanical { retry: 3 })
        .with_dependency("code");
    
    let task_doc = Task::new(
        "doc".into(),
        "生成 API 文档".into(),
        "doc".into(),
    ).with_level(TaskLevel::Recommended {
        default_action: Action::Execute,
        timeout_secs: 60,
    }).with_dependency("test");
    
    // 4. 执行 DAG
    let dag = scheduler.build_dag(vec![
        task_analyze,
        task_code,
        task_test,
        task_doc,
    ]).await.unwrap();
    
    let result = scheduler.execute_dag(dag).await.unwrap();
    
    // 5. 验证结果
    assert!(matches!(result.status, ExecutionStatus::Completed));
    assert_eq!(result.results.len(), 4);
    
    // 6. 验证记忆存储（上下文保留）
    let context = memory.semantic_search("API 实现", 10, 0.6).await.unwrap();
    assert!(!context.is_empty());
}
```

### 7. 性能基准测试

**文件**: `cis-core/benches/multi_agent_bench.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use cis_core::agent::{AgentPool, AgentType};
use cis_core::scheduler::{DagScheduler, Task};

fn bench_agent_turn(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(AgentPool::new_test());
    let mut agent = rt.block_on(pool.acquire(AgentType::Coder)).unwrap();
    
    c.bench_function("agent_turn", |b| {
        b.to_async(&rt).iter(|| {
            agent.turn(black_box("写一个函数"))
        })
    });
}

fn bench_dag_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut scheduler = rt.block_on(DagScheduler::new_test());
    
    let tasks: Vec<Task> = (0..10).map(|i| {
        Task::new(
            format!("task-{}", i),
            format!("任务 {}", i),
            "bench".into(),
        ).with_skill("test")
    }).collect();
    
    c.bench_function("dag_execution_10_tasks", |b| {
        b.to_async(&rt).iter(|| {
            let dag = scheduler.build_dag(tasks.clone()).await.unwrap();
            scheduler.execute_dag(dag)
        })
    });
}

fn bench_parallel_agents(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("parallel_agents");
    
    for num_agents in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("agents", num_agents),
            num_agents,
            |b, &num| {
                let pool = rt.block_on(AgentPool::new_test_with_capacity(num));
                
                b.to_async(&rt).iter(|| {
                    let agents: Vec<_> = (0..num)
                        .map(|_| pool.acquire(AgentType::Coder))
                        .collect();
                    
                    futures::future::join_all(agents)
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_agent_turn, bench_dag_execution, bench_parallel_agents
);
criterion_main!(benches);
```

## 验收标准

- [ ] 所有 Agent 协作测试通过
- [ ] DAG 编排集成测试通过
- [ ] 记忆隔离测试通过
- [ ] P2P 跨设备测试通过
- [ ] 端到端工作流测试通过
- [ ] 性能基准测试达标（参考值见下）
- [ ] 测试覆盖率 > 80%
- [ ] 所有测试可重复执行（无 flaky tests）
- [ ] CI/CD 集成测试配置完成

### 性能基准参考值

| 指标 | 目标值 |
|------|--------|
| Agent turn 响应时间 | < 2s (单轮对话) |
| DAG 执行吞吐量 | > 10 tasks/s |
| 并行 Agent 扩展比 | > 0.7 (8 agents) |
| P2P Agent 发现时间 | < 5s |
| 记忆搜索延迟 (p99) | < 100ms |

## 依赖

- TASK_7_4 (DAG 编排)
- TASK_7_5 (记忆隔离)
- TASK_7_6 (P2P 跨设备)

## 阻塞

- 无（Phase 7 最后一项）

---

**关键测试场景**:
- ✅ Receptionist → Coder → Doc 完整协作
- ✅ DAG 四级决策机制验证
- ✅ Agent 记忆隔离与幻觉降低
- ✅ P2P 跨设备 Agent 调用
- ✅ 并行执行性能验证
