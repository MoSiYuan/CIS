# CIS v1.1.4 测试策略

> 创建日期: 2026-02-10
> 目标版本: v1.1.4
> 优先级: P0

---

## 执行摘要

本文档定义 CIS v1.1.4 版本的测试策略、测试金字塔设计和质量保证流程。

### 目标

| 指标 | 当前 (v1.1.3) | 目标 (v1.1.4) | 测量方法 |
|------|--------------|--------------|---------|
| 整体覆盖率 | ?% | **75%** | `cargo llvm-cov` |
| 关键模块覆盖率 | ?% | **80%** | 模块级统计 |
| P0 功能测试 | ? | **100%** | 手工验证 |
| CI 通过率 | ?% | **95%** | GitHub Actions |

---

## 一、测试金字塔

```
                  ┌─────────────────────┐
                  │     E2E 测试         │  10%
                  │   关键路径 (5-10个)  │
                  ├─────────────────────┤
                  │     集成测试         │  30%
                  │   模块间 (50-100个)  │
                  ├─────────────────────┤
                  │     单元测试         │  60%
                  │   函数级 (300-500个) │
                  └─────────────────────┘
```

### 测试分层原则

1. **单元测试** - 快速、隔离、可并行
2. **集成测试** - 真实依赖、覆盖关键路径
3. **E2E 测试** - 用户场景、少量核心流程

---

## 二、模块测试覆盖率目标

### P0 - 关键模块 (80%+)

| 模块 | 覆盖率目标 | 重点测试 |
|------|-----------|---------|
| `p2p/` | 80% | 连接、传输、DHT |
| `wasm/` | 85% | 沙箱、内存、生命周期 |
| `agent/federation/` | 80% | 任务分发、事件订阅 |
| `matrix/federation/` | 75% | 事件同步、签名验证 |

### P1 - 重要模块 (70%+)

| 模块 | 覆盖率目标 | 重点测试 |
|------|-----------|---------|
| `scheduler/` | 75% | DAG 执行、并发 |
| `skill/` | 70% | 路由、执行器 |
| `storage/` | 75% | CRUD、事务 |
| `network/` | 70% | WebSocket、QUIC |

### P2 - 辅助模块 (60%+)

| 模块 | 覆盖率目标 | 说明 |
|------|-----------|------|
| `cli/` | 60% | 命令行解析 |
| `gui/` | 50% | UI 交互 (手工为主) |
| `utils/` | 65% | 工具函数 |

---

## 三、关键测试场景

### 3.1 P0 - 必须覆盖

#### WASM Skill 生命周期

```rust
#[tokio::test]
async fn wasm_skill_lifecycle() {
    // 1. 加载 WASM 模块
    let skill = WasmSkill::load("test_skill.wasm").await.unwrap();

    // 2. 初始化
    skill.initialize().await.unwrap();

    // 3. 执行
    let result = skill.execute("test_method", b"input").await.unwrap();
    assert_eq!(result, b"expected output");

    // 4. 清理
    skill.cleanup().await.unwrap();

    // 5. 内存泄漏检测
    // (使用 valgrind 或类似工具)
}
```

#### P2P 连接完整性

```rust
#[tokio::test]
async fn p2p_connection_cycle() {
    // 1. 启动两个节点
    let node1 = P2PNetwork::start(config1).await.unwrap();
    let node2 = P2PNetwork::start(config2).await.unwrap();

    // 2. 建立连接
    node1.connect(&node2.id()).await.unwrap();
    assert!(node1.is_connected(&node2.id()));

    // 3. 数据传输
    node1.send(&node2.id(), b"hello").await.unwrap();
    let data = node2.receive().await.unwrap();
    assert_eq!(data, b"hello");

    // 4. 断线重连
    node2.stop().await;
    tokio::time::sleep(Duration::from_secs(5)).await;
    node2.start().await.unwrap();
    assert!(node1.is_connected(&node2.id()));

    // 5. 资源清理
    node1.stop().await;
    node2.stop().await;
}
```

#### Agent 联邦任务分发

```rust
#[tokio::test]
async fn federation_task_distribution() {
    // 1. 启动两个 Agent
    let agent1 = FederationAgent::start(config1).await.unwrap();
    let agent2 = FederationAgent::start(config2).await.unwrap();

    // 2. 建立联邦
    agent1.federate(&agent2.id()).await.unwrap();

    // 3. 分发任务
    let task = Task::new("test_task");
    agent1.dispatch(task.clone()).await.unwrap();

    // 4. 验证执行
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = agent2.get_task_status(&task.id).await {
                assert_eq!(status, TaskStatus::Completed);
                break;
            }
        }
    }).await.unwrap();

    // 5. 结果返回
    let result = agent1.get_task_result(&task.id).await.unwrap();
    assert!(result.is_success());
}
```

#### 调度器 DAG 执行

```rust
#[tokio::test]
async fn scheduler_dag_execution() {
    // 1. 创建 DAG
    let dag = DagSkill::new("test_dag");
    dag.add_task("task1", deps![]);
    dag.add_task("task2", deps!["task1"]);
    dag.add_task("task3", deps!["task1"]);
    dag.add_task("task4", deps!["task2", "task3"]);

    // 2. 执行
    let executor = SchedulerExecutor::new();
    executor.execute_dag(&dag).await.unwrap();

    // 3. 验证执行顺序
    let log = executor.get_execution_log();
    assert!(log.index_of("task1") < log.index_of("task2"));
    assert!(log.index_of("task1") < log.index_of("task3"));
    assert!(log.index_of("task2") < log.index_of("task4"));
    assert!(log.index_of("task3") < log.index_of("task4"));

    // 4. 验证并行执行
    let duration = log.get_duration();
    assert!(duration < Duration::from_secs(2)); // 串行需要 4s
}
```

### 3.2 P1 - 重要场景

#### 并发任务调度

```rust
#[tokio::test]
async fn concurrent_scheduling() {
    let executor = SchedulerExecutor::new();
    executor.set_max_concurrent_tasks(3);

    // 启动 10 个任务
    let tasks: Vec<_> = (0..10)
        .map(|i| executor.submit(Task::new(format!("task{}", i))))
        .collect();

    // 等待全部完成
    for task in tasks {
        task.await.unwrap();
    }

    // 验证最多 3 个并发
    let max_concurrent = executor.get_max_concurrent_count();
    assert!(max_concurrent <= 3);
}
```

#### 网络分区恢复

```rust
#[tokio::test]
async fn network_partition_recovery() {
    let node1 = P2PNetwork::start(config1).await.unwrap();
    let node2 = P2PNetwork::start(config2).await.unwrap();

    // 建立连接
    node1.connect(&node2.id()).await.unwrap();

    // 模拟网络分区
    node1.simulate_partition(true).await;

    // 验证连接断开
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!node1.is_connected(&node2.id()));

    // 恢复网络
    node1.simulate_partition(false).await;

    // 验证自动重连
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(node1.is_connected(&node2.id()));
}
```

### 3.3 P2 - 边界场景

#### 内存泄漏检测

```bash
# 使用 heaptrack
cargo build --release
heaptrack ./cis-core test wasm-memory-leak

# 或使用 valgrind
valgrind --leak-check=full ./cis-core test wasm-memory-leak
```

#### 性能回归测试

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn wasm_execution_benchmark(c: &mut Criterion) {
    c.bench_function("wasm_execute", |b| {
        b.iter(|| {
            let skill = WasmSkill::load("test.wasm").unwrap();
            skill.execute(black_box("test"), black_box(b"input")).unwrap()
        })
    });
}

criterion_group!(benches, wasm_execution_benchmark);
criterion_main!(benches);
```

---

## 四、测试工具栈

### 4.1 依赖配置

```toml
[dev-dependencies]
# 异步测试
tokio-test = "0.4"

# Mock 框架
mockall = "0.12"

# 属性测试
proptest = "1.0"
quickcheck = "1.0"

# 性能测试
criterion = "0.5"

# 覆盖率
cargo-llvm-cov = "0.5"

# 快速失败
cargo-harness = "0.1"

# 快照测试
insta = "1.34"
```

### 4.2 CI/CD 集成

```yaml
# .github/workflows/test.yml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, nightly]

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}
          components: llvm-tools-preview

      - name: Install cargo-llvm-cov
        run: cargo install cargo-llvm-cov

      - name: Unit tests
        run: cargo test --lib

      - name: Integration tests
        run: cargo test --test '*'

      - name: Coverage
        run: cargo llvm-cov --html --lcov

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: ./lcov.info

      - name: Lint
        run: |
          cargo fmt --check
          cargo clippy -- -D warnings

      - name: Security audit
        run: |
          cargo install cargo-audit
          cargo audit
```

---

## 五、Mock 设计规范

### 5.1 网络服务 Mock

```rust
#[cfg(test)]
mockall::mock! {
    pub NetworkService {}

    #[async_trait]
    impl NetworkService for NetworkService {
        async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()>;
        async fn broadcast(&self, data: &[u8]) -> Result<()>;
        async fn connected_peers(&self) -> Vec<PeerInfo>;
    }
}

#[tokio::test]
async fn test_with_mock_network() {
    let mut mock_network = MockNetworkService::new();

    // 预设行为
    mock_network
        .expect_send_to()
        .with(mockall::predicate::eq("node1"), always())
        .returning(|_, _| Ok(()));

    // 使用 Mock
    let service = NodeService::new(Arc::new(mock_network));
    service.send_message("node1", b"hello").await.unwrap();
}
```

### 5.2 存储 Mock

```rust
#[cfg(test)]
mockall::mock! {
    pub StorageService {}

    #[async_trait]
    impl StorageService for StorageService {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
        async fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    }
}

#[tokio::test]
async fn test_with_mock_storage() {
    let mut mock_storage = MockStorageService::new();

    mock_storage
        .expect_get()
        .with(mockall::predicate::eq("test_key"))
        .returning(|_| Ok(Some(b"test_value".to_vec())));

    let service = CacheService::new(Arc::new(mock_storage));
    assert_eq!(service.get("test_key").await.unwrap(), Some(b"test_value".to_vec()));
}
```

---

## 六、测试执行计划

### 6.1 阶段一：基础设施 (Week 1)

- [ ] 搭建 CI/CD 流程
- [ ] 配置测试工具链
- [ ] 建立 Mock 框架
- [ ] 编写测试用例模板

### 6.2 阶段二：单元测试 (Week 2-4)

- [ ] P0 模块单元测试 (p2p, wasm, federation)
- [ ] P1 模块单元测试 (scheduler, skill, storage)
- [ ] 覆盖率达到 60%

### 6.3 阶段三：集成测试 (Week 5-7)

- [ ] P0 场景集成测试
- [ ] P1 场景集成测试
- [ ] 覆盖率达到 70%

### 6.4 阶段四：E2E 测试 (Week 8-9)

- [ ] 关键流程端到端测试
- [ ] 性能基准测试
- [ ] 压力测试

### 6.5 阶段五：回归测试 (Week 10-11)

- [ ] 全量回归测试
- [ ] 覆盖率达到 75%
- [ ] Bug 修复验证

---

## 七、质量门禁

### 7.1 提交前检查

```bash
#!/bin/bash
# pre-commit hook

# 格式检查
cargo fmt --check

# Lint
cargo clippy -- -D warnings

# 单元测试
cargo test --lib

# 覆盖率检查 (不低于 70%)
cargo llvm-cov --lcov
if [ $(lcov --summary lcov.info | grep lines | awk '{print $2}' | sed 's/%//') -lt 70 ]; then
    echo "Coverage below 70%"
    exit 1
fi
```

### 7.2 PR 合并条件

- [ ] 所有 CI 检查通过
- [ ] 覆盖率不低于 70%
- [ ] 至少 1 人 Code Review
- [ ] P0 代码必须有对应测试
- [ ] 安全审计通过

### 7.3 发布条件

- [ ] 覆盖率达到 75%
- [ ] P0/P1 功能测试 100% 通过
- [ ] 性能基准达标
- [ ] 安全审计无高危漏洞
- [ ] E2E 测试全部通过

---

## 八、测试维护

### 8.1 定期任务

- [ ] 每周检查覆盖率趋势
- [ ] 每月更新测试用例
- [ ] 每季度性能基线校准

### 8.2 测试债务管理

| 模块 | 测试债务 | 优先级 | 计划修复时间 |
|------|---------|--------|------------|
| `gui/` | 覆盖率 50% | P2 | v1.2.0 |
| `cli/` | 缺少集成测试 | P1 | v1.1.5 |

---

*文档创建日期: 2026-02-10*
*下次更新日期: 开发过程中持续更新*
