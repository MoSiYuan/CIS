# T2.1: P2P Network 状态管理

**任务编号**: T2.1  
**任务名称**: P2P Network State Management  
**优先级**: P1  
**预估时间**: 5 小时  
**依赖**: T1.1 (mDNS), T1.2 (QUIC)  
**分配状态**: 待分配

---

## 任务概述

实现全局 P2P 网络状态管理，作为单例提供启动、停止、状态查询和节点管理功能。

---

## 输入

### 依赖任务输出
- **T1.1**: `MdnsService` 和 `DiscoveredNode`
- **T1.2**: `QuicTransport` 和 `ConnectionInfo`

### 现有代码
- 文件: `cis-core/src/p2p/mod.rs` (已有 P2PNetwork 结构)

---

## 输出要求

### 必须实现的接口

```rust
// 文件: cis-core/src/p2p/network.rs (重构或新建)

use std::sync::Arc;
use tokio::sync::{OnceCell, RwLock};
use anyhow::Result;

use crate::p2p::{
    mdns_service::{MdnsService, DiscoveredNode},
    quic_transport::{QuicTransport, ConnectionInfo},
};

/// 全局 P2P 网络实例
static P2P_INSTANCE: OnceCell<RwLock<Option<Arc<P2PNetwork>>>> = OnceCell::const_new();

/// P2P 网络管理器
pub struct P2PNetwork {
    node_id: String,
    did: String,
    mdns: MdnsService,
    transport: QuicTransport,
    config: P2PConfig,
    started_at: std::time::Instant,
}

/// P2P 配置
#[derive(Debug, Clone)]
pub struct P2PConfig {
    pub listen_addr: String,      // 如 "0.0.0.0:7677"
    pub enable_mdns: bool,        // 启用 mDNS 发现
    pub enable_dht: bool,         // 启用 DHT
    pub bootstrap_nodes: Vec<String>,
}

/// 网络状态
#[derive(Debug, Clone)]
pub struct NetworkStatus {
    pub running: bool,
    pub node_id: String,
    pub listen_addr: String,
    pub uptime_secs: u64,
    pub connected_peers: usize,
    pub discovered_peers: usize,
}

/// 对等节点信息
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: String,
    pub did: String,
    pub address: String,
    pub connected: bool,
    pub last_seen: std::time::SystemTime,
}

impl P2PNetwork {
    /// 获取全局实例
    /// 
    /// # Returns
    /// None - 网络未启动
    /// Some(Arc<P2PNetwork>) - 网络运行中
    pub async fn global() -> Option<Arc<Self>> {
        let guard = P2P_INSTANCE.get_or_init(|| RwLock::new(None)).read().await;
        guard.as_ref().map(Arc::clone)
    }
    
    /// 初始化并启动 P2P 网络
    /// 
    /// # Arguments
    /// * `config` - P2P 配置
    /// 
    /// # Returns
    /// 已存在的实例（如果已启动）
    pub async fn start(config: P2PConfig) -> Result<Arc<Self>> {
        // 检查是否已启动
        // 创建 MdnsService
        // 创建 QuicTransport
        // 启动后台任务（监听连接、发现节点）
        // 存储到全局实例
    }
    
    /// 停止 P2P 网络
    /// 
    /// 清理所有资源，断开所有连接
    pub async fn stop() -> Result<()> {
        // 获取全局实例
        // 关闭 transport
        // 关闭 mdns
        // 清除全局实例
    }
    
    /// 获取状态
    pub async fn status(&self) -> NetworkStatus {
        // 实现...
    }
    
    /// 连接到指定节点
    /// 
    /// # Arguments
    /// * `addr` - 节点地址 (如 "192.168.1.100:7677")
    pub async fn connect(&self, addr: &str) -> Result<()> {
        // 解析地址
        // 调用 transport.connect()
    }
    
    /// 断开与节点的连接
    pub async fn disconnect(&self, node_id: &str) -> Result<()> {
        // 调用 transport.disconnect()
    }
    
    /// 获取已发现的节点列表
    pub async fn discovered_peers(&self) -> Vec<PeerInfo> {
        // 合并 mdns 发现的节点和已连接的节点
    }
    
    /// 获取已连接的节点列表
    pub async fn connected_peers(&self) -> Vec<PeerInfo> {
        // 从 transport 获取连接列表
    }
    
    /// 向节点发送消息
    pub async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()> {
        // 调用 transport.send()
    }
    
    /// 广播消息到所有连接节点
    pub async fn broadcast(&self, data: &[u8]) -> Result<usize> {
        // 发送到所有已连接节点
        // 返回发送成功的节点数
    }
}

impl Drop for P2PNetwork {
    fn drop(&mut self) {
        // 尝试优雅关闭
    }
}
```

---

## 实现步骤

1. **整合依赖组件**
   - 使用 T1.1 的 `MdnsService`
   - 使用 T1.2 的 `QuicTransport`

2. **实现全局单例**
   - 使用 `OnceCell<RwLock<Option<Arc<P2PNetwork>>>>`
   - 确保线程安全

3. **实现 start**
   - 检查是否已启动
   - 创建 MdnsService（如果启用）
   - 创建 QuicTransport
   - 启动后台监听任务
   - 存储实例

4. **实现 stop**
   - 获取写锁
   - 调用各组件 shutdown
   - 置为 None

5. **实现状态查询**
   - 从各组件收集状态
   - 计算运行时间

6. **实现节点管理**
   - connect/disconnect
   - discovered/connected peers

7. **添加测试**
   - 单测使用 mock transport
   - 集成测试需要真实网络

---

## 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_start_stop() {
        let config = P2PConfig {
            listen_addr: "127.0.0.1:0".to_string(),
            enable_mdns: false,  // 测试中禁用 mDNS
            enable_dht: false,
            bootstrap_nodes: vec![],
        };
        
        // 启动
        let network = P2PNetwork::start(config).await.unwrap();
        assert!(P2PNetwork::global().await.is_some());
        
        // 停止
        P2PNetwork::stop().await.unwrap();
        assert!(P2PNetwork::global().await.is_none());
    }
    
    #[tokio::test]
    async fn test_singleton() {
        // 多次 start 返回相同实例
    }
    
    #[tokio::test]
    async fn test_status() {
        // 验证状态信息准确
    }
}
```

---

## 验收标准

- [ ] 多次调用 start 返回相同实例（单例模式）
- [ ] stop 后资源完全释放
- [ ] 状态信息实时准确
- [ ] 支持并发访问（线程安全）
- [ ] 单测覆盖率 > 80%

---

## 输出文件

```
cis-core/src/p2p/
├── network.rs           # 主要实现（重构 mod.rs 或新建）
├── mod.rs               # 更新导出
└── tests/
    └── network_test.rs
```

---

## 阻塞关系

**依赖**:
- T1.1: MdnsService
- T1.2: QuicTransport

**阻塞**:
- T3.1: p2p discover 命令
- T3.2: p2p connect/disconnect 命令
- T4.1: DHT 操作
