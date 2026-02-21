# TASK 4.1: ZeroClaw 适配层实现

> **Phase**: 4 - ZeroClaw 兼容
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: 23f642b
> **负责人**: TBD
> **周期**: Week 8

---

## 任务概述

实现 CIS v1.2.0 与 ZeroClaw 的适配层，使 CIS 可作为 ZeroClaw 的 backend 运行。

## 工作内容

### 1. 分析 ZeroClaw 接口

基于前期分析，ZeroClaw 需要：
- `Memory` trait: `remember`, `recall`, `forget`
- `DelegateTool`: 任务委托接口
- `Session` 管理: 隔离不同对话

### 2. 实现 Memory 适配器

```rust
// crates/cis-memory/src/zeroclaw/adapter.rs
#[cfg(feature = "zeroclaw-compat")]
pub struct ZeroclawMemoryAdapter<M: Memory> {
    inner: M,
    session_manager: Arc<SessionManager>,
}

#[cfg(feature = "zeroclaw-compat")]
impl<M: Memory> ZeroclawMemoryAdapter<M> {
    pub fn new(inner: M) -> Self {
        Self {
            inner,
            session_manager: Arc::new(SessionManager::new()),
        }
    }
    
    /// 适配 ZeroClaw 的记忆接口
    pub async fn save_memory(
        &self,
        session_id: &str,
        category: MemoryCategory,
        content: &str,
    ) -> Result<MemoryId, MemoryError> {
        let entry = MemoryEntry::builder()
            .session_id(session_id)
            .category(category)
            .content(content)
            .build();
        
        self.inner.remember(entry.clone()).await?;
        Ok(entry.id)
    }
    
    /// 适配 ZeroClaw 的回忆接口
    pub async fn load_memories(
        &self,
        session_id: &str,
        category: Option<MemoryCategory>,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let filter = MemoryFilter::new()
            .session_id(session_id)
            .category(category)
            .query(query);
        
        self.inner.recall_with_filter(filter, limit).await
    }
}
```

### 3. 实现 Delegate Tool 适配

```rust
// crates/cis-core/src/zeroclaw/delegate.rs
#[cfg(feature = "zeroclaw")]
pub struct ZeroclawDelegateAdapter {
    runtime: Arc<Runtime>,
}

#[cfg(feature = "zeroclaw")]
impl ZeroclawDelegateAdapter {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self { runtime }
    }
    
    /// 将 ZeroClaw 的 delegate 请求转换为 CIS 任务
    pub async fn delegate_task(
        &self,
        agent_type: &str,
        task_description: &str,
        context: DelegateContext,
    ) -> Result<DelegateResult, DelegateError> {
        // 创建 CIS 任务
        let task = Task::builder()
            .task_type(TaskType::from(agent_type))
            .description(task_description)
            .context(context.into())
            .build()?;
        
        // 通过 CIS 调度器执行
        let result = self.runtime.execute(task).await?;
        
        // 转换结果为 ZeroClaw 格式
        Ok(DelegateResult::from(result))
    }
}
```

### 4. 配置 feature flag

```toml
# crates/cis-core/Cargo.toml
[features]
default = []
zeroclaw = [
    "dep:zeroclaw",
    "cis-memory/zeroclaw-compat",
]
```

## 验收标准

- [ ] Memory 适配器通过 ZeroClaw 接口测试
- [ ] Delegate 适配器工作正常
- [ ] Session 隔离正确实现
- [ ] Feature flag 控制编译
- [ ] 与 ZeroClaw 集成测试通过

## 依赖

- Task 3.2 (cis-core 重构完成)

## 阻塞

- Task 4.2 (端到端验证)

---
