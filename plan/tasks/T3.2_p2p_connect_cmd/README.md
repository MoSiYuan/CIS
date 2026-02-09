# T3.2: p2p connect/disconnect 命令

**任务编号**: T3.2  
**任务名称**: p2p connect/disconnect Commands  
**优先级**: P1  
**预估时间**: 3h  
**依赖**: T2.1 (P2P Network)  
**分配状态**: 待分配

---

## 任务概述

实现真实的节点连接和断开命令。

---

## 输入

### 依赖任务输出
- **T2.1**: `P2PNetwork`

### 待修改文件
- `cis-node/src/commands/p2p.rs:380-458`

---

## 输出要求

```rust
async fn connect_node(address: &str, node_id: Option<&str>) -> Result<()> {
    let network = P2PNetwork::global().await?;
    network.connect(address).await?;
    println!("✅ Connected to {}", address);
    Ok(())
}

async fn disconnect_node(node_id: &str) -> Result<()> {
    let network = P2PNetwork::global().await?;
    network.disconnect(node_id).await?;
    println!("✅ Disconnected from {}", node_id);
    Ok(())
}
```

---

## 验收标准

- [ ] 连接真实建立 QUIC 连接
- [ ] 断开后资源释放
- [ ] 连接失败返回具体错误

---

## 阻塞关系

**依赖**:
- T2.1: P2PNetwork
