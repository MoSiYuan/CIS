# T4.2: Federation 事件发送

**任务编号**: T4.2  
**任务名称**: Federation Event Sending  
**优先级**: P2  
**预估时间**: 5h  
**依赖**: T2.2 (Matrix Server Manager)  
**分配状态**: 待分配

---

## 任务概述

实现 Agent Federation 的真实 Matrix 事件发送。

---

## 输入

### 待修改文件
- `cis-core/src/agent/federation/agent.rs:192,271,293,320`

### 当前问题
```rust
// TODO: 实现实际的 Matrix 事件发送
// TODO: 通过 FederationClient 发送心跳
// TODO: 订阅 Matrix Room 事件
```

---

## 输出要求

```rust
impl FederationClient {
    pub async fn send_heartbeat(&self) -> Result<()>;
    pub async fn send_task_request(&self, task: &TaskRequest) -> Result<String>;
    pub async fn subscribe_events(&self, callback: impl Fn(FederationEvent)) -> Result<()>;
}
```

---

## 验收标准

- [ ] 心跳事件真实发送到 Matrix Room
- [ ] 其他节点能收到并处理
- [ ] 断线后自动重连

---

## 阻塞关系

**依赖**:
- T2.2: Matrix Server Manager
