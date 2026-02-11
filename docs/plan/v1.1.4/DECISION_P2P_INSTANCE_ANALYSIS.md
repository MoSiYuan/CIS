# P2P_INSTANCE 全局单例问题详细分析

> 决策 2 的详细技术解释
> 日期: 2026-02-10

---

## 问题现状

### 代码位置
```rust
// cis-core/src/p2p/network.rs:261
impl P2PNetwork {
    /// ⚠️ 已标记废弃，但仍在广泛使用
    #[deprecated(since = "1.1.4", note = "Use dependency injection instead")]
    pub fn global() -> Result<Arc<P2PNetwork>> {
        P2P_INSTANCE.get()
            .and_then(|lock| lock.read().ok())
            .and_then(|guard| guard.clone())
            .ok_or_else(|| CisError::p2p("P2P not initialized"))
    }
}

// 全局静态实例
static P2P_INSTANCE: OnceCell<RwLock<Option<Arc<P2PNetwork>>>> = OnceCell::new();
```

### 当前使用情况

```bash
$ grep -r "P2PNetwork::global()" cis-core/src --include="*.rs" | wc -l
23 处调用

$ grep -r "P2P_INSTANCE" cis-core/src --include="*.rs" | wc -l
15 处引用
```

### 依赖 P2P_INSTANCE 的核心模块

1. **Matrix Bridge** (`matrix/bridge.rs`)
   - 处理 Room 事件时发送 P2P 消息
   - 当前直接调用 `P2PNetwork::global()`

2. **Agent Federation** (`agent/federation/`)
   - 发现节点时使用全局实例广播
   - 任务分发时获取连接

3. **CLI 命令** (`cis-node/src/commands/`)
   - `node status` 显示 P2P 状态
   - `peer list` 获取连接列表

4. **Skill 执行器** (`scheduler/skill_executor.rs`)
   - 远程 Skill 调用时发送网络请求

---

## 为什么难以移除

### 1. 深度耦合的调用链

```
CLI Command (cis-node)
    ↓ 调用
NodeService::status()
    ↓ 调用
P2PNetwork::global().connected_peers()
    ↓ 访问
P2P_INSTANCE (全局状态)
```

**问题**: 从 CLI 到 P2P 有 3-4 层调用，每层都需要修改注入依赖

### 2. 生命周期管理复杂

```rust
// 当前模式：隐式初始化
pub fn init() {
    let network = P2PNetwork::new(config);
    P2P_INSTANCE.set(RwLock::new(Some(network)));
}

// 依赖注入模式需要显式传递
pub struct NodeService {
    network: Arc<dyn NetworkService>, // 每个服务都需要这个字段
}
```

**问题**: 
- 现有 15+ 个服务需要添加 `network` 字段
- 每个服务的构造函数需要修改
- 所有调用点需要更新

### 3. 测试代码大量依赖

```rust
// 现有测试模式
#[test]
fn test_something() {
    let _guard = setup_global_p2p(); // 设置全局实例
    // 测试代码直接使用 P2PNetwork::global()
}
```

**问题**: 50+ 个测试需要重写，使用 `TestContainerBuilder`

### 4. 异步初始化时序

```rust
// 当前：可以在任何地方、任何时间获取
async fn any_function() {
    let p2p = P2PNetwork::global()?; // 可能失败（未初始化）
}

// 依赖注入：必须在构造时提供
async fn new_service(container: &ServiceContainer) -> Self {
    Self {
        network: container.network(), // 保证存在
    }
}
```

**问题**: 需要重构异步初始化流程，确保依赖按序创建

---

## 移除方案对比

### 方案 A: 一次性全部移除 (激进)

**实施步骤**:
1. 修改 `ServiceContainer::production()` 创建 P2PNetwork
2. 修改 15+ 个服务，添加 `network` 字段
3. 修改 23 处调用，使用注入的依赖
4. 重写 50+ 个测试
5. 验证所有功能正常

**工作量**: 约 2-3 周
**风险**: 高（可能引入回归 bug）
**优点**: 彻底解决问题

### 方案 B: 分阶段迁移 (推荐)

**阶段 1 (v1.1.5)**: 核心服务迁移
- 迁移 `Agent Federation`（最依赖 P2P）
- 迁移 `Matrix Bridge`
- 保持其他模块使用全局实例（添加废弃警告）

**阶段 2 (v1.1.6)**: 次要服务迁移
- 迁移 `Skill Executor`
- 迁移 `CLI Commands`

**阶段 3 (v1.2.0)**: 完全移除
- 删除全局实例
- 清理所有废弃代码

**工作量**: 每阶段 3-5 天
**风险**: 低（逐步验证）
**优点**: 可控、可回滚

### 方案 C: 保留但隔离 (妥协)

**设计**:
```rust
// 保留全局实例，但仅用于 CLI
pub mod cli {
    pub fn quick_p2p_call() {
        // 使用 P2P_INSTANCE
    }
}

// 核心逻辑使用依赖注入
pub mod core {
    pub struct FederationManager {
        network: Arc<dyn NetworkService>, // 注入
    }
}
```

**工作量**: 1 周（重构目录结构）
**风险**: 中（维护两套模式）
**优点**: 快速交付，逐步过渡

---

## 推荐方案 (基于您的决策 2)

### 采用方案 B: 分阶段迁移

**理由**:
1. 保证 v1.1.5 按时交付核心功能
2. 逐步验证，降低回归风险
3. 与 D02-4 (v1.2.0 完全移除) 的承诺一致

### v1.1.5 阶段实施计划

**Week 1**: Agent Federation 迁移
```rust
// 修改前
pub struct FederationManager;
impl FederationManager {
    pub async fn discover() {
        let p2p = P2PNetwork::global().unwrap(); // ❌ 全局
    }
}

// 修改后
pub struct FederationManager {
    network: Arc<dyn NetworkService>, // ✅ 注入
}
impl FederationManager {
    pub fn new(network: Arc<dyn NetworkService>) -> Self {
        Self { network }
    }
}
```

**Week 2**: Matrix Bridge 迁移 + 验证

**Week 3**: 回归测试，修复问题

**关键代码变更**:
```rust
// cis-core/src/container.rs
pub async fn production(config: Config) -> Result<Self> {
    // 创建 P2PNetwork
    let network = Arc::new(P2PNetwork::new(&config.p2p).await?);
    
    // 创建联邦管理器（注入依赖）
    let federation = FederationManager::new(network.clone());
    
    // ... 其他服务
}
```

---

## 过渡期间的兼容性保证

```rust
// p2p/network.rs
impl P2PNetwork {
    #[deprecated(since = "1.1.4", note = "Use ServiceContainer instead")]
    pub fn global() -> Result<Arc<P2PNetwork>> {
        // 添加运行时警告
        tracing::warn!("P2PNetwork::global() is deprecated, use dependency injection");
        
        P2P_INSTANCE.get()
            .and_then(|lock| lock.read().ok())
            .and_then(|guard| guard.clone())
            .ok_or_else(|| CisError::p2p("P2P not initialized"))
    }
}
```

---

## 验收标准

**v1.1.5 阶段**:
- [ ] `Agent Federation` 不再使用 `P2PNetwork::global()`
- [ ] `Matrix Bridge` 不再使用 `P2PNetwork::global()`
- [ ] 其他模块调用时显示废弃警告
- [ ] 所有测试通过

**v1.2.0 阶段**:
- [ ] 完全删除 `P2P_INSTANCE`
- [ ] 删除 `P2PNetwork::global()` 方法
- [ ] 清理所有废弃代码

---

*分析完成时间: 2026-02-10*
