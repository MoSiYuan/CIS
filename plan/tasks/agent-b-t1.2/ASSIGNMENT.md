# Agent-B 任务分配

**Agent 标识**: Agent-B  
**任务**: T1.2 + T4.1  
**技能要求**: QUIC、P2P 协议、网络传输  
**优先级**: P0 (最高)  
**预估总时间**: 12 小时

---

## 任务清单

### 任务 1: T1.2 - QUIC 传输层实现
**文件**: `plan/tasks/T1.2_quic_transport/README.md`  
**时间**: 6h  
**状态**: 🔴 立即开始

**核心目标**:
- 实现基于 QUIC 的 P2P 传输层
- 支持连接管理和消息传输
- 使用 `quinn` crate

**关键接口**:
```rust
impl QuicTransport {
    pub async fn bind(bind_addr: &str, node_id: &str) -> Result<Self>;
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<bool>;
    pub async fn disconnect(&self, node_id: &str) -> Result<()>;
    pub async fn send(&self, node_id: &str, data: &[u8]) -> Result<()>;
    pub async fn list_connections(&self) -> Vec<ConnectionInfo>;
}
```

**输出文件**:
- `cis-core/src/p2p/quic_transport.rs`
- `cis-core/src/p2p/tests/quic_transport_test.rs`

---

### 任务 2: T4.1 - DHT 真实操作
**文件**: `plan/tasks/T4.1_dht_operations/README.md`  
**时间**: 6h  
**状态**: 🔴 等待 T2.1 完成后开始

**核心目标**:
- 实现 DHT put/get/find_node
- 基于 Kademlia 协议
- 支持跨节点数据存储和检索

**关键接口**:
```rust
impl P2PNetwork {
    pub async fn dht_put(&self, key: &str, value: &str) -> Result<()>;
    pub async fn dht_get(&self, key: &str) -> Result<Option<String>>;
    pub async fn dht_find_node(&self, node_id: &str) -> Result<Vec<NodeInfo>>;
}
```

---

## 执行顺序

```
┌─────────────────────────────────────────────────────┐
│  1. T1.2 (6h)                                        │
│     - 实现 QuicTransport                            │
│     - 配置 QUIC 参数                                │
│     - 编写单元测试                                  │
│     - 提交 PR                                        │
│                                                      │
│     ↓ 等待 T2.1 完成                                 │
│                                                      │
│  2. T4.1 (6h)                                        │
│     - 基于现有 dht.rs 实现真实操作                  │
│     - 集成到 P2PNetwork                             │
│     - 提交 PR                                        │
└─────────────────────────────────────────────────────┘
```

---

## 协作接口

**你提供的接口**:
```rust
pub use cis_core::p2p::quic_transport::{QuicTransport, ConnectionInfo};
```

**你依赖的接口**:
```rust
// T2.1 (Agent-D) 提供 P2PNetwork
// T4.1 时需要 DHT 基础结构（已有 dht.rs）
```

---

## 验收标准

### T1.2 验收
- [ ] 本地回环测试通过 (127.0.0.1:0)
- [ ] 支持并发 100+ 连接
- [ ] 连接断开后正确清理资源
- [ ] 单测覆盖率 > 80%

### T4.1 验收
- [ ] put 后 get 能获取相同值
- [ ] 跨节点数据可检索
- [ ] 路由表维护正确
- [ ] 节点离线后数据仍可用

---

## 关键挑战

### T1.2 挑战
- QUIC 证书生成和管理
- 并发连接的资源管理
- 连接状态维护

### T4.1 挑战
- Kademlia 路由表维护
- 节点加入/离开的动态处理
- 数据冗余和一致性

---

## 开始工作

1. 阅读: `plan/tasks/T1.2_quic_transport/README.md`
2. 创建分支: `git checkout -b agent-b/t1.2-quic`
3. 开始实现 QUIC 传输层

---

**祝你好运！**
