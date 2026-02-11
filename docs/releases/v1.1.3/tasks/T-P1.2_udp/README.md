# T-P1.2: Matrix UDP 直连实现

**优先级**: 🟡 P1  
**预估时间**: 6h  
**依赖**: P2PNetwork UDP  
**分配**: Agent-D

---

## 问题描述

当前 WebSocket 连接缺少 UDP 直连优化，同局域网应使用 UDP。

**问题文件**: `cis-core/src/matrix/websocket/client.rs:323`

**当前代码**:
```rust
// TODO: 建立 UDP 直连（当前版本回退到 WebSocket）
```

---

## 修复方案

使用已实现的 P2PNetwork UDP 能力:

```rust
pub async fn connect_udp(&mut self, addr: SocketAddr) -> Result<()> {
    // 检查是否为同局域网
    if is_same_lan(addr) {
        // 使用 P2PNetwork 的 UDP 连接
        let p2p = P2PNetwork::global().await?;
        let conn = p2p.connect_udp(addr).await?;
        self.transport = Transport::Udp(conn);
    } else {
        // 回退到 WebSocket
        self.connect_ws(addr).await?;
    }
}
```

---

## 验收标准

- [ ] 同局域网使用 UDP 直连
- [ ] 支持 TURN 中继跨网络
- [ ] 自动回退到 WebSocket
