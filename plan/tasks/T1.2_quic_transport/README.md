# T1.2: QUIC 传输层实现

**任务编号**: T1.2  
**任务名称**: QUIC Transport Layer  
**优先级**: P0 (最高)  
**预估时间**: 6 小时  
**依赖**: 无  
**分配状态**: 待分配

---

## 任务概述

实现基于 QUIC 协议的 P2P 传输层，提供可靠的、支持多路复用的节点间通信能力。

---

## 输入

### 现有代码
- **参考文件**: `cis-core/src/p2p/transport.rs` (已有基础结构)
- **依赖 crate**: `quinn = "0.11"` (已配置)

### 现有结构
```rust
pub struct QuicTransport {
    endpoint: Endpoint,
}
```

---

## 输出要求

### 必须实现的接口

```rust
// 文件: cis-core/src/p2p/quic_transport.rs (新建或重构 transport.rs)

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use quinn::{Endpoint, Connection, RecvStream, SendStream};

/// QUIC 传输层
pub struct QuicTransport {
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
    node_id: String,
}

/// 对等节点连接信息
#[derive(Debug, Clone)]
pub struct PeerConnection {
    pub node_id: String,
    pub address: SocketAddr,
    pub quinn_conn: Connection,
    pub established_at: std::time::Instant,
}

/// 连接信息（不包含内部 quinn 对象）
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub node_id: String,
    pub address: SocketAddr,
    pub established_at: std::time::Instant,
    pub rtt_ms: Option<u64>,
}

impl QuicTransport {
    /// 创建 QUIC 端点并绑定到本地地址
    /// 
    /// # Arguments
    /// * `bind_addr` - 绑定地址 (如 "0.0.0.0:7677")
    /// * `node_id` - 本节点标识
    pub async fn bind(bind_addr: &str, node_id: &str) -> Result<Self> {
        // 实现...
    }
    
    /// 连接到远程节点
    /// 
    /// # Arguments
    /// * `node_id` - 目标节点标识
    /// * `addr` - 目标地址
    /// 
    /// # Returns
    /// 连接是否新建（已存在则返回 false）
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<bool> {
        // 实现...
    }
    
    /// 断开与指定节点的连接
    pub async fn disconnect(&self, node_id: &str) -> Result<()> {
        // 实现...
    }
    
    /// 获取所有连接信息
    pub async fn list_connections(&self) -> Vec<ConnectionInfo> {
        // 实现...
    }
    
    /// 向指定节点发送数据
    /// 
    /// # Arguments
    /// * `node_id` - 目标节点
    /// * `data` - 原始数据
    pub async fn send(&self, node_id: &str, data: &[u8]) -> Result<()> {
        // 实现...
    }
    
    /// 接收消息（阻塞）
    /// 
    /// # Returns
    /// (sender_node_id, data)
    pub async fn recv(&self) -> Result<(String, Vec<u8>)> {
        // 实现...
    }
    
    /// 获取本地监听地址
    pub fn local_addr(&self) -> Result<SocketAddr> {
        // 实现...
    }
    
    /// 关闭传输层
    pub async fn shutdown(self) -> Result<()> {
        // 实现...
    }
}

impl Drop for QuicTransport {
    fn drop(&mut self) {
        // 清理连接...
    }
}
```

---

## 技术规格

### QUIC 配置
```rust
// Server config
let mut server_config = ServerConfig::with_single_cert(cert, key)?;
server_config.transport_config(Arc::new({
    let mut config = TransportConfig::default();
    config.max_concurrent_uni_streams(100.try_into().unwrap());
    config.max_concurrent_bidi_streams(100.try_into().unwrap());
    Arc::new(config)
}));

// Client config
let client_config = ClientConfig::with_native_roots();
```

### 协议约定
- **ALPN**: `cis/1.0`
- **最大并发流**: 100 (双向)
- **Keep-alive**: 30 秒
- **Idle timeout**: 60 秒

---

## 实现步骤

1. **证书生成**
   - 使用 `rcgen` 生成自签名证书
   - 或支持外部证书配置

2. **实现 bind**
   - 创建 Endpoint
   - 配置 QUIC 参数
   - 启动后台接收任务

3. **实现 connect**
   - 检查是否已连接
   - 创建 QUIC 连接
   - 存储到 connections map

4. **实现 send/recv**
   - 打开 bi-directional stream
   - 发送/接收数据
   - 处理流错误

5. **实现 disconnect**
   - 关闭 QUIC 连接
   - 从 map 移除

6. **添加测试**
   - 本地回环测试
   - 并发连接测试

---

## 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_bind_localhost() {
        let transport = QuicTransport::bind("127.0.0.1:0", "node1").await.unwrap();
        assert!(transport.local_addr().unwrap().port() > 0);
    }
    
    #[tokio::test]
    async fn test_connect_and_send() {
        // 创建两个 transport
        // 互相连接
        // 发送消息验证
    }
    
    #[tokio::test]
    async fn test_concurrent_connections() {
        // 测试 100+ 并发连接
    }
    
    #[tokio::test]
    async fn test_disconnect_cleanup() {
        // 验证断开后资源释放
    }
}
```

---

## 验收标准

- [ ] 本地回环测试通过 (127.0.0.1:0)
- [ ] 支持并发 100+ 连接
- [ ] 连接断开后能正确清理资源
- [ ] 提供连接状态查询接口
- [ ] 单测覆盖率 > 80%
- [ ] 通过 `cargo clippy` 检查

---

## 参考文档

- [quinn crate docs](https://docs.rs/quinn)
- [QUIC RFC 9000](https://tools.ietf.org/html/rfc9000)
- 现有代码: `cis-core/src/p2p/transport.rs`

---

## 输出文件

```
cis-core/src/p2p/
├── quic_transport.rs    # 主要实现
├── mod.rs               # 添加导出
└── tests/
    └── quic_transport_test.rs
```

---

## 阻塞关系

**阻塞**:
- T2.1: P2P Network 状态管理
- T3.2: p2p connect/disconnect 命令
