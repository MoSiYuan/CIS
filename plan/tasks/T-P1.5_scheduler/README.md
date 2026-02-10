# T-P1.5: Scheduler 模拟等待

**优先级**: 🟡 P1  
**预估时间**: 3h  
**依赖**: -  
**分配**: Agent-F

---

## 问题描述

Scheduler 使用模拟等待时间，而非真实等待用户输入。

**问题文件**: `cis-core/src/scheduler/skill_executor.rs`

**行号**: 327, 355

**当前代码**:
```rust
// 模拟等待时间（实际应用中这里会等待用户输入）
tokio::time::sleep(Duration::from_secs(2)).await;
```

---

## 修复方案

使用异步通道等待真实输入:

```rust
use tokio::sync::mpsc;

pub struct SkillExecutor {
    input_rx: mpsc::Receiver<String>,
}

impl SkillExecutor {
    pub async fn wait_for_user_input(&mut self) -> Result<String> {
        // 真实等待用户输入
        match self.input_rx.recv().await {
            Some(input) => Ok(input),
            None => Err(anyhow!("Input channel closed")),
        }
    }
}
```

---

## 验收标准

- [ ] 实现真实的用户输入等待
- [ ] 支持超时机制
- [ ] 支持取消操作
