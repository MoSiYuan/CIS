# CIS GUI MVVM 架构设计文档

> **版本**: 1.0
> **创建日期**: 2026-02-12
> **作者**: Team H (CisApp GUI 重构团队)

---

## 1. 概述

### 1.1 当前问题

CisApp 类存在以下严重问题：

- **代码量过大**: 5000+ 行代码，违反单一职责原则
- **职责混乱**: 混合了 UI 渲染、业务逻辑、状态管理、命令处理
- **难以测试**: 业务逻辑与 UI 框架紧密耦合
- **维护困难**: 修改一个功能可能影响其他功能
- **代码重复**: 相似的逻辑在多处重复

### 1.2 MVVM 架构优势

采用 MVVM (Model-View-ViewModel) 架构可以带来以下优势：

- **职责分离**: View 只负责渲染，ViewModel 管理状态和逻辑
- **易于测试**: ViewModel 可以独立于 UI 框架进行单元测试
- **代码复用**: 通用逻辑抽取到基类或工具函数
- **维护性提升**: 每个文件职责清晰，修改影响范围小
- **团队协作**: 不同开发者可以并行开发不同的 ViewModel

---

## 2. 架构设计

### 2.1 整体架构

```
┌────────────────────────────────────────────────────────────────┐
│                         CisApp (View Container)              │
│  - 只负责 UI 组件的组装和渲染                                 │
│  - 委托所有业务逻辑给 ViewModel                               │
└────────────────────────────────────────────────────────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│ MainViewModel│    │NodeViewModel │    │TerminalView  │
│              │    │              │    │   Model       │
│- 应用状态    │    │- 节点列表    │    │- 命令历史    │
│- 服务管理    │    │- 节点操作    │    │- 命令处理    │
│- 通知机制    │    │- 状态同步    │    │- 输出格式化  │
└──────────────┘    └──────────────┘    └──────────────┘
         │                    │                    │
         ▼                    ▼                    ▼
┌────────────────────────────────────────────────────────────────┐
│                       Controllers                            │
│  - NodeController: 节点 CRUD 操作                            │
│  - TaskController: 任务和 DAG 操作                            │
│  - ServiceController: 服务调用封装                             │
└────────────────────────────────────────────────────────────────┘
         │
         ▼
┌────────────────────────────────────────────────────────────────┐
│                    cis-core Services                         │
│  - NodeService                                              │
│  - DagService                                               │
│  - MemoryService                                            │
└────────────────────────────────────────────────────────────────┘
```

### 2.2 模块结构

```
cis-gui/src/
├── app.rs                    # 主应用 (精简后，~300 行)
├── view_models/
│   ├── mod.rs               # 模块导出
│   ├── base.rs              # ViewModel 基类
│   ├── main.rs              # 主视图模型
│   ├── node.rs              # 节点视图模型
│   ├── terminal.rs          # 终端视图模型
│   └── decision.rs         # 决策视图模型
├── controllers/
│   ├── mod.rs               # 控制器模块
│   ├── node_controller.rs   # 节点控制器
│   ├── task_controller.rs   # 任务控制器
│   └── service_controller.rs # 服务控制器
└── (其他 UI 组件保持不变)
```

---

## 3. ViewModel 设计

### 3.1 基类设计

所有 ViewModel 共享的基础功能：

```rust
pub trait ViewModel: Send + Sync {
    /// 获取 ViewModel 名称
    fn name(&self) -> &str;

    /// 通知 UI 有状态变化
    fn notify(&self);

    /// 检查是否需要刷新 UI
    fn needs_refresh(&self) -> bool;
}
```

### 3.2 MainViewModel

**职责**: 管理应用级别的状态和服务

```rust
pub struct MainViewModel {
    // 服务实例
    node_service: Option<NodeService>,
    dag_service: Option<DagService>,

    // 异步运行时
    runtime: tokio::runtime::Runtime,

    // 子 ViewModels
    node_vm: Arc<NodeViewModel>,
    terminal_vm: Arc<TerminalViewModel>,
    decision_vm: Arc<DecisionViewModel>,

    // 应用状态
    is_initialized: bool,
    last_error: Option<String>,
}

impl MainViewModel {
    pub fn new() -> Self;
    pub fn initialize(&mut self) -> Result<()>;
    pub fn get_node_vm(&self) -> Arc<NodeViewModel>;
    pub fn get_terminal_vm(&self) -> Arc<TerminalViewModel>;
    pub fn get_decision_vm(&self) -> Arc<DecisionViewModel>;
}
```

### 3.3 NodeViewModel

**职责**: 管理节点列表、状态和操作

```rust
pub struct NodeViewModel {
    // 节点数据
    nodes: Arc<RwLock<Vec<ManagedNode>>>,
    demo_nodes: Vec<ManagedNode>,

    // 服务
    node_service: Option<NodeService>,
    runtime: tokio::runtime::Runtime,

    // 刷新状态
    last_refresh: Instant,
    refresh_interval: Duration,
    is_refreshing: Arc<AtomicBool>,

    // 通知通道
    refresh_tx: tokio::sync::mpsc::Sender<NodeRefreshResult>,
}

impl NodeViewModel {
    pub fn new(node_service: Option<NodeService>) -> Self;
    pub fn get_nodes(&self) -> Vec<ManagedNode>;
    pub fn refresh_nodes(&self);
    pub fn ping_node(&self, node_id: &str) -> Result<bool>;
    pub fn block_node(&self, node_id: &str) -> Result<()>;
    pub fn unblock_node(&self, node_id: &str) -> Result<()>;
    pub fn verify_node(&self, node_id: &str, did: &str) -> Result<()>;
    pub fn inspect_node(&self, node_id: &str) -> Result<NodeInfo>;
}
```

### 3.4 TerminalViewModel

**职责**: 管理终端历史、命令处理和输出

```rust
pub struct TerminalViewModel {
    // 命令历史
    history: Arc<RwLock<Vec<String>>>,

    // 命令处理依赖
    node_vm: Arc<NodeViewModel>,
    decision_vm: Arc<DecisionViewModel>,
    runtime: tokio::runtime::Runtime,

    // 服务
    node_service: Option<NodeService>,
    dag_service: Option<DagService>,
}

impl TerminalViewModel {
    pub fn new(
        node_vm: Arc<NodeViewModel>,
        decision_vm: Arc<DecisionViewModel>,
        node_service: Option<NodeService>,
        dag_service: Option<DagService>,
    ) -> Self;

    pub fn get_history(&self) -> Vec<String>;
    pub fn execute_command(&mut self, cmd: &str);
    pub fn clear(&mut self);

    // 命令处理方法
    fn show_help(&mut self);
    fn cmd_node_list(&mut self);
    fn cmd_node_inspect(&mut self, node_id: &str);
    fn cmd_dag_list(&mut self);
    fn cmd_dag_run(&mut self, dag_id: &str);
}
```

### 3.5 DecisionViewModel

**职责**: 管理四级决策面板

```rust
pub struct DecisionViewModel {
    // 决策面板
    panel: DecisionPanel,
    pending_decision: Option<PendingDecision>,
}

impl DecisionViewModel {
    pub fn new() -> Self;
    pub fn set_pending_decision(&mut self, decision: PendingDecision);
    pub fn get_pending_decision(&self) -> Option<&PendingDecision>;
    pub fn handle_action(&mut self, action: DecisionAction) -> String;
}
```

---

## 4. Controller 设计

### 4.1 NodeController

```rust
pub struct NodeController {
    node_service: NodeService,
    runtime: tokio::runtime::Runtime,
}

impl NodeController {
    pub fn new(node_service: NodeService) -> Self;

    // CRUD 操作
    pub async fn list_nodes(&self, options: ListOptions) -> Result<NodeListResult>;
    pub async fn inspect_node(&self, node_id: &str) -> Result<NodeInfo>;
    pub async fn ping_node(&self, node_id: &str) -> Result<bool>;
    pub async fn bind_node(&self, options: BindOptions) -> Result<NodeInfo>;
    pub async fn block_node(&self, node_id: &str) -> Result<()>;
    pub async fn unblock_node(&self, node_id: &str) -> Result<()>;
    pub async fn verify_node(&self, node_id: &str, did: &str) -> Result<()>;
    pub async fn get_node_stats(&self, node_id: &str) -> Result<NodeStats>;
}
```

### 4.2 TaskController

```rust
pub struct TaskController {
    dag_service: DagService,
    runtime: tokio::runtime::Runtime,
}

impl TaskController {
    pub fn new(dag_service: DagService) -> Self;

    pub async fn list_dags(&self, options: ListOptions) -> Result<DagListResult>;
    pub async fn run_dag(&self, dag_id: &str, params: HashMap<String, Value>) -> Result<DagRun>;
    pub async fn get_run_status(&self, run_id: &str) -> Result<DagRunInfo>;
    pub async fn cancel_run(&self, run_id: &str) -> Result<()>;
    pub async fn confirm_run(&self, run_id: &str) -> Result<()>;
    pub async fn list_runs(&self, dag_id: &str, limit: usize) -> Result<Vec<DagRun>>;
}
```

---

## 5. 数据流设计

### 5.1 状态通知机制

使用观察者模式实现状态变化通知：

```rust
pub trait Observer<T> {
    fn on_update(&self, data: T);
}

pub struct Observable<T> {
    observers: Vec<Box<dyn Observer<T>>>,
    value: T,
}

impl<T: Clone> Observable<T> {
    pub fn new(value: T) -> Self {
        Self {
            observers: Vec::new(),
            value,
        }
    }

    pub fn subscribe(&mut self, observer: Box<dyn Observer<T>>) {
        self.observers.push(observer);
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
        for observer in &self.observers {
            observer.on_update(self.value.clone());
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }
}
```

### 5.2 命令执行流程

```
用户输入命令
    ↓
TerminalViewModel.execute_command()
    ↓
解析命令
    ↓
调用对应的 Controller 方法
    ↓
Controller 调用 cis-core Service
    ↓
返回结果
    ↓
TerminalViewModel 格式化输出
    ↓
更新 history，触发 UI 刷新
```

---

## 6. 重构后的 CisApp

### 6.1 精简的 CisApp 结构

```rust
pub struct CisApp {
    // ViewModels
    main_vm: Arc<MainViewModel>,
    node_vm: Arc<NodeViewModel>,
    terminal_vm: Arc<TerminalViewModel>,
    decision_vm: Arc<DecisionViewModel>,

    // UI 组件 (保持不变)
    node_tabs: NodeTabs,
    node_manager: NodeManager,
    glm_panel: GlmPanel,

    // 对话框状态 (纯 UI 状态)
    show_verification_dialog: bool,
    verification_input: String,
    show_dag_detail: bool,
}
```

### 6.2 精简的方法

```rust
impl CisApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // 初始化所有 ViewModels
        let main_vm = Arc::new(MainViewModel::new());
        main_vm.initialize().expect("Failed to initialize");

        let node_vm = main_vm.get_node_vm();
        let terminal_vm = main_vm.get_terminal_vm();
        let decision_vm = main_vm.get_decision_vm();

        // UI 组件初始化保持不变...

        Self {
            main_vm,
            node_vm,
            terminal_vm,
            decision_vm,
            // ...
        }
    }

    // update 方法只负责 UI 渲染
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // 1. 检查 ViewModel 状态变化
        if self.node_vm.needs_refresh() {
            // 触发 UI 刷新
        }

        // 2. 渲染 UI 组件
        self.render_node_tabs(ctx);
        self.render_terminal(ctx);
        self.render_panels(ctx);

        // 3. 处理 UI 事件，委托给 ViewModel
        self.handle_ui_events();
    }

    // 所有业务逻辑方法委托给 ViewModel
    fn execute_command(&mut self, cmd: &str) {
        self.terminal_vm.execute_command(cmd);
    }
}
```

---

## 7. 测试策略

### 7.1 ViewModel 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_view_model_refresh() {
        let vm = NodeViewModel::new(None);
        vm.refresh_nodes();

        // 等待异步操作完成
        std::thread::sleep(Duration::from_secs(1));

        let nodes = vm.get_nodes();
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_terminal_command_parsing() {
        let node_vm = Arc::new(NodeViewModel::new(None));
        let decision_vm = Arc::new(DecisionViewModel::new());
        let mut terminal_vm = TerminalViewModel::new(
            node_vm,
            decision_vm,
            None,
            None,
        );

        terminal_vm.execute_command("help");
        let history = terminal_vm.get_history();

        assert!(history.iter().any(|line| line.contains("Available commands")));
    }
}
```

### 7.2 Controller 集成测试

```rust
#[tokio::test]
async fn test_node_controller() {
    let service = NodeService::new().unwrap();
    let controller = NodeController::new(service);

    let result = controller.list_nodes(ListOptions::default()).await;
    assert!(result.is_ok());
}
```

---

## 8. 迁移步骤

### 阶段 1: 创建基础结构 (1 天)

- [ ] 创建 `view_models/` 目录
- [ ] 创建 `controllers/` 目录
- [ ] 实现 `ViewModel` 基类 trait
- [ ] 实现基础状态通知机制

### 阶段 2: 实现 ViewModels (3 天)

- [ ] 实现 `MainViewModel` (P1-7.2)
- [ ] 实现 `NodeViewModel` (P1-7.3)
- [ ] 实现 `TerminalViewModel` (P1-7.4)
- [ ] 实现 `DecisionViewModel` (P1-7.4)
- [ ] 为每个 ViewModel 编写单元测试

### 阶段 3: 实现 Controllers (1 天)

- [ ] 实现 `NodeController`
- [ ] 实现 `TaskController`
- [ ] 实现 `ServiceController`
- [ ] 编写 Controller 集成测试

### 阶段 4: 重构 CisApp (2 天)

- [ ] 移除业务逻辑到 ViewModels
- [ ] 精简 `app.rs` 到 ~300 行
- [ ] 保持 UI 组件接口不变
- [ ] 添加集成测试

### 阶段 5: 验证和优化 (1 天)

- [ ] 功能回归测试
- [ ] 性能测试
- [ ] 代码审查
- [ ] 文档更新

---

## 9. 预期成果

### 9.1 代码行数对比

| 文件 | 重构前 | 重构后 | 变化 |
|------|--------|--------|------|
| app.rs | 1471 | ~300 | -79% |
| main.rs | (未创建) | ~150 | +150 |
| node.rs | (未创建) | ~400 | +400 |
| terminal.rs | (未创建) | ~500 | +500 |
| decision.rs | (未创建) | ~200 | +200 |
| base.rs | (未创建) | ~100 | +100 |
| node_controller.rs | (未创建) | ~300 | +300 |
| task_controller.rs | (未创建) | ~250 | +250 |
| **总计** | 1471 | ~2200 | +49% |

### 9.2 质量指标

| 指标 | 重构前 | 重构后 | 改进 |
|------|--------|--------|------|
| 单文件行数 | 1471 | <500 | -66% |
| 职责清晰度 | ⭐⭐ | ⭐⭐⭐⭐⭐ | +150% |
| 可测试性 | 30% | 80% | +167% |
| 维护难度 | 高 | 低 | -60% |
| 代码重复率 | 15% | <5% | -67% |

---

## 10. 风险与缓解

### 10.1 风险

1. **破坏现有功能**: 重构可能引入 bug
2. **性能下降**: 额外的抽象层可能影响性能
3. **学习曲线**: 团队需要熟悉新架构
4. **时间投入**: 比预期耗时更长

### 10.2 缓解措施

1. **保持 UI 接口不变**: 所有 UI 组件保持原有接口
2. **渐进式迁移**: 先实现新架构，再逐步迁移
3. **充分测试**: 每个阶段都有测试覆盖
4. **代码审查**: 每个阶段都要经过审查
5. **性能基准**: 建立性能基准，监控变化

---

## 11. 总结

采用 MVVM 架构重构 CisApp 将带来：

✅ **更好的代码组织**: 每个文件职责清晰
✅ **更高的可测试性**: ViewModel 可以独立测试
✅ **更强的可维护性**: 修改影响范围小
✅ **更好的团队协作**: 并行开发不同模块
✅ **更少的代码重复**: 通用逻辑抽取到基类

虽然总代码量有所增加，但代码质量和可维护性将大幅提升。

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**维护者**: Team H
