# T-P1.5: Scheduler 模拟等待

**优先级**: 🟡 P1  
**预估时间**: 3h  
**依赖**: -  
**分配**: Agent-F  
**状态**: ✅ 已完成

---

## 问题描述

Scheduler 使用模拟等待时间，而非真实等待用户输入。

**问题文件**: `cis-core/src/scheduler/skill_executor.rs`

**行号**: 398, 442

**当前代码**:
```rust
// 模拟等待时间（实际应用中这里会等待用户输入）
tokio::time::sleep(Duration::from_secs(3)).await;
```

---

## 修复方案

使用异步通道等待真实输入:

### 新增类型

```rust
/// 用户输入类型
#[derive(Debug, Clone)]
pub enum UserInput {
    /// 确认任务继续执行
    Confirm { task_id: String },
    /// 取消任务
    Cancel { task_id: String, reason: String },
    /// 仲裁投票
    ArbitrationVote { 
        task_id: String, 
        stakeholder: String, 
        approve: bool,
        comment: Option<String>,
    },
    /// 跳过任务
    Skip { task_id: String },
}
```

### 修改 SkillDagExecutor

```rust
pub struct SkillDagExecutor {
    // ... 原有字段
    /// 用户输入接收器
    input_rx: mpsc::Receiver<UserInput>,
    /// 用户输入发送器
    input_tx: mpsc::Sender<UserInput>,
}
```

### 新的 wait_confirmation 实现

- 使用 `wait_for_input()` 等待真实用户输入
- 支持 Confirm、Cancel、Skip 操作
- 超时后默认继续（5分钟默认超时）

### 新的 wait_arbitration 实现

- 循环收集利益相关者投票
- 简单多数决（>50%）决定结果
- 超时后根据已收集投票决定
- 默认超时 10 分钟

---

## 验收标准

- [x] 实现真实的用户输入等待
- [x] 支持超时机制
- [x] 支持取消操作

---

## 变更详情

### 文件: `cis-core/src/scheduler/skill_executor.rs`

**新增**:
- `UserInput` 枚举类型（支持 Confirm, Cancel, ArbitrationVote, Skip）
- `SkillDagExecutor.input_rx` 字段
- `SkillDagExecutor.input_tx` 字段
- `SkillDagExecutor::input_sender()` 方法
- `SkillDagExecutor::wait_for_input()` 辅助方法

**修改**:
- `new()` - 初始化输入通道
- `with_decision_engine()` - 初始化输入通道
- `wait_confirmation()` - 使用真实输入等待
- `wait_arbitration()` - 使用真实投票收集

---

## 验证结果

```bash
cargo check -p cis-core
```

✅ 编译成功，无错误

---

## 使用示例

```rust
// 创建执行器
let mut executor = SkillDagExecutor::new(scheduler, skill_manager);
let input_tx = executor.input_sender();

// 在另一个任务/线程中发送用户输入
tokio::spawn(async move {
    input_tx.send(UserInput::Confirm { 
        task_id: "task-1".to_string() 
    }).await.ok();
});

// 执行器会等待真实输入
executor.execute_dag_skill(&dag_def, inputs).await?;
```
