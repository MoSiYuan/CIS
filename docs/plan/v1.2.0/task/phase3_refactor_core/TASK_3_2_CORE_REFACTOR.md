# TASK 3.2: cis-core 核心重构

> **Phase**: 3 - cis-core 重构
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 6-7

---

## 任务概述

重构 `cis-core` 成为轻量级的协调层，专注于运行时管理和上下文协调。

## 工作内容

### 1. 新 cis-core 结构

```
cis-core/src/
├── lib.rs              # 重导出 + 核心初始化
├── runtime.rs          # Runtime 管理
│   ├── mod.rs
│   ├── builder.rs      # RuntimeBuilder
│   └── runtime.rs      # Runtime 实现
├── orchestration.rs    # 编排逻辑
│   ├── mod.rs
│   ├── dag.rs          # DAG 编排（高级）
│   └── pipeline.rs     # Pipeline 编排
├── context.rs          # 执行上下文
│   ├── mod.rs
│   ├── global.rs       # 全局上下文
│   └── task.rs         # 任务上下文
└── config.rs           # 配置管理
```

### 2. 实现 RuntimeBuilder

```rust
// runtime/builder.rs
pub struct RuntimeBuilder {
    storage: Option<Box<dyn Storage>>,
    memory: Option<Box<dyn Memory>>,
    scheduler: Option<Box<dyn Scheduler>>,
    network: Option<Box<dyn Network>>,
    vector_store: Option<Box<dyn VectorStore>>,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            storage: None,
            memory: None,
            scheduler: None,
            network: None,
            vector_store: None,
        }
    }
    
    pub fn with_storage(mut self, storage: impl Storage + 'static) -> Self {
        self.storage = Some(Box::new(storage));
        self
    }
    
    pub fn with_memory(mut self, memory: impl Memory + 'static) -> Self {
        self.memory = Some(Box::new(memory));
        self
    }
    
    // ... 其他 with_xxx 方法
    
    pub fn build(self) -> Result<Runtime, BuildError> {
        Ok(Runtime {
            storage: self.storage.ok_or(BuildError::MissingStorage)?,
            memory: self.memory.ok_or(BuildError::MissingMemory)?,
            scheduler: self.scheduler.ok_or(BuildError::MissingScheduler)?,
            network: self.network,
            vector_store: self.vector_store,
        })
    }
}
```

### 3. 实现 Runtime

```rust
// runtime/runtime.rs
pub struct Runtime {
    storage: Box<dyn Storage>,
    memory: Box<dyn Memory>,
    scheduler: Box<dyn Scheduler>,
    network: Option<Box<dyn Network>>,
    vector_store: Option<Box<dyn VectorStore>>,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }
    
    pub async fn execute(&self, task: Task) -> Result<TaskResult, ExecutionError> {
        let handle = self.scheduler.schedule(task).await?;
        handle.await.map_err(|e| ExecutionError::from(e))
    }
    
    pub fn storage(&self) -> &dyn Storage {
        &*self.storage
    }
    
    pub fn memory(&self) -> &dyn Memory {
        &*self.memory
    }
    
    // ... 其他访问器
}
```

### 4. 实现上下文管理

```rust
// context/global.rs
pub struct GlobalContext {
    runtime: Arc<Runtime>,
    metadata: HashMap<String, String>,
}

impl GlobalContext {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            runtime,
            metadata: HashMap::new(),
        }
    }
    
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}

// context/task.rs
pub struct TaskContext {
    global: Arc<GlobalContext>,
    task_id: TaskId,
    inputs: HashMap<String, Value>,
}

impl TaskContext {
    pub fn input(&self, name: &str) -> Option<&Value> {
        self.inputs.get(name)
    }
    
    pub fn storage(&self) -> &dyn Storage {
        self.global.runtime().storage()
    }
}
```

## 验收标准

- [ ] RuntimeBuilder 模式实现完整
- [ ] Runtime 可正确初始化所有组件
- [ ] 上下文管理可用
- [ ] 编排功能正常工作
- [ ] 向后兼容层可用
- [ ] 所有测试通过

## 依赖

- Task 3.1 (依赖更新)

## 阻塞

- Task 4.x (ZeroClaw 集成)
- Task 5.x (测试)

---
