# TASK 2.5: 提取 cis-p2p

> **Phase**: 2 - 模块提取
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: 1aa447a
> **负责人**: TBD
> **周期**: Week 5

---

## 任务概述

将 P2P 网络模块从 cis-core 提取为独立的 `cis-p2p` crate，支持跨设备 Agent 调用。

## 工作内容

### 1. 分析现有 P2P 实现

审查 `cis-core/src/p2p/`：
- `Network` trait
- `libp2p` 集成
- 协议定义
- 节点发现

### 2. 创建 crate 结构

```
crates/cis-p2p/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── network.rs      # Network trait
│   ├── libp2p_impl.rs  # libp2p 实现
│   ├── protocol.rs     # 协议定义
│   ├── discovery.rs    # 节点发现
│   ├── messaging.rs    # 消息传递
│   └── rpc.rs          # 远程调用
└── tests/
    └── p2p_tests.rs
```

### 3. 实现网络抽象

```rust
// network.rs
#[async_trait]
pub trait Network: Send + Sync + Lifecycle {
    async fn dial(&self, peer_id: &PeerId) -> Result<Connection, NetworkError>;
    async fn listen(&self, addr: &Multiaddr) -> Result<(), NetworkError>;
    async fn broadcast(&self, message: Message) -> Result<(), NetworkError>;
    fn local_peer_id(&self) -> &PeerId;
    fn connected_peers(&self) -> Vec<PeerId>;
}

pub struct Connection {
    peer_id: PeerId,
    stream: Box<dyn AsyncReadWrite>,
}
```

### 4. 实现远程 Agent 调用

```rust
// rpc.rs
pub struct RemoteAgentClient<N: Network> {
    network: N,
    timeout: Duration,
}

impl<N: Network> RemoteAgentClient<N> {
    pub async fn call_agent(
        &self,
        peer_id: &PeerId,
        agent_type: AgentType,
        request: AgentRequest,
    ) -> Result<AgentResponse, RpcError> {
        // 建立连接
        let conn = self.network.dial(peer_id).await?;
        
        // 发送 RPC 请求
        let request_bytes = serialize(&RpcMessage::AgentCall {
            agent_type,
            request,
        })?;
        conn.send(&request_bytes).await?;
        
        // 等待响应
        let response_bytes = conn.recv_timeout(self.timeout).await?;
        let response: RpcMessage = deserialize(&response_bytes)?;
        
        match response {
            RpcMessage::AgentResponse(resp) => Ok(resp),
            RpcMessage::Error(e) => Err(RpcError::Remote(e)),
            _ => Err(RpcError::UnexpectedResponse),
        }
    }
}
```

### 5. 配置 feature flags

```toml
[features]
default = ["libp2p", "mdns"]
libp2p = ["dep:libp2p"]
mdns = ["libp2p?/mdns"]
quic = ["libp2p?/quic"]
```

## 验收标准

- [ ] libp2p 集成正常工作
- [ ] 节点发现功能正常
- [ ] 远程调用协议实现
- [ ] 支持跨设备消息传递
- [ ] 集成测试通过

## 依赖

- Task 1.3 (cis-traits)

## 阻塞

- Task 3.2 (重构 cis-core)
- Task 7.x (多 Agent 跨设备调用)

---
