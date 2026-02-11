# T-P1.7: Federation 实现占位

**优先级**: 🟡 P1  
**预估时间**: 2h  
**依赖**: FederationClient  
**分配**: Agent-B

---

## 问题描述

Federation 返回 placeholder 响应。

**问题文件**: `cis-core/src/matrix/federation_impl.rs:681`

**当前代码**:
```rust
// For now, return a placeholder response
return Ok(json!({"status": "ok"}));
```

---

## 修复方案

使用已实现的 FederationClient:

```rust
use cis_core::agent::federation_client::FederationClient;

pub async fn send_federation_request(&self, event: FederationEvent) -> Result<()> {
    let client = FederationClient::new(&self.node_id);
    client.send_event(event).await
}
```

---

## 验收标准

- [ ] 使用 FederationClient 发送真实事件
- [ ] 处理 Federation 响应
