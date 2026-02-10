# T-P1.6: Matrix Cloud 配额模拟

**优先级**: 🟡 P1  
**预估时间**: 3h  
**依赖**: Cloud API  
**分配**: Agent-A

---

## 问题描述

Cloud 配额使用是模拟的。

**问题文件**: `cis-core/src/matrix/cloud/client.rs:779`

**当前代码**:
```rust
// 模拟配额使用（实际使用需要异步环境）
let quota_used = 0.5; // 模拟 50% 使用
```

---

## 修复方案

实现真实的配额查询:

```rust
pub async fn get_quota_usage(&self) -> Result<QuotaInfo> {
    let response = self.client
        .get("/_matrix/client/v3/admin/whois")
        .send()
        .await?;
    
    let quota: QuotaInfo = response.json().await?;
    Ok(quota)
}
```

---

## 验收标准

- [ ] 实现真实的配额查询 API
- [ ] 缓存配额信息
