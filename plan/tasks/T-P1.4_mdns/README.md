# T-P1.4: Matrix mDNS 发现

**优先级**: 🟡 P1  
**预估时间**: 2h  
**依赖**: MdnsService  
**分配**: Agent-E

---

## 问题描述

Matrix 局域网发现是 placeholder。

**问题文件**: `cis-core/src/matrix/federation/mod.rs:49`

**当前代码**:
```rust
//! - ✅ Optional mDNS discovery (placeholder)
```

---

## 修复方案

集成已实现的 MdnsService:

```rust
use cis_core::p2p::MdnsService;

pub struct MatrixDiscovery {
    mdns: MdnsService,
}

impl MatrixDiscovery {
    pub async fn discover_local_homeservers(&self) -> Vec<DiscoveredNode> {
        let service_type = "_matrix._tcp.local";
        self.mdns.discover_with_type(service_type).await
    }
}
```

---

## 验收标准

- [ ] 使用 MdnsService 发现 Matrix 节点
- [ ] 支持 _matrix._tcp.local 服务类型
