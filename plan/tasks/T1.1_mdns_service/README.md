# T1.1: mDNS 服务封装

**任务编号**: T1.1  
**任务名称**: mDNS Service Wrapper  
**优先级**: P0 (最高)  
**预估时间**: 4 小时  
**依赖**: 无  
**分配状态**: 待分配

---

## 任务概述

封装 `mdns-sd` crate，为 CIS 提供简洁的 mDNS 局域网服务发现和广播接口。这是 P2P 网络发现的基础组件。

---

## 输入

### 现有代码
- **参考文件**: `cis-core/src/p2p/discovery.rs` (已有基础结构)
- **依赖 crate**: `mdns-sd = "0.10"` (已在 `cis-core/Cargo.toml` 配置)

### 现有结构分析
```rust
// discovery.rs 已有:
pub struct DiscoveryService {
    node_id: String,
    discovered_peers: Arc<Mutex<HashSet<PeerDiscoveryInfo>>>,
}
```

---

## 输出要求

### 必须实现的接口

```rust
// 文件: cis-core/src/p2p/mdns_service.rs (新建)

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use anyhow::Result;

/// mDNS 服务实例
pub struct MdnsService {
    daemon: mdns_sd::ServiceDaemon,
    service_type: String,
    node_id: String,
}

/// 发现的节点信息
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredNode {
    pub node_id: String,
    pub address: SocketAddr,
    pub did: String,
    pub metadata: HashMap<String, String>,
}

impl MdnsService {
    /// 创建并启动 mDNS 广播服务
    /// 
    /// # Arguments
    /// * `node_id` - 本节点唯一标识
    /// * `port` - 服务端口 (通常是 7676)
    /// * `did` - 去中心化身份标识
    /// * `metadata` - 额外的元数据 (capabilities 等)
    pub fn new(
        node_id: &str,
        port: u16,
        did: &str,
        metadata: HashMap<String, String>,
    ) -> Result<Self> {
        // 实现...
    }
    
    /// 发现同网段的 CIS 节点
    /// 
    /// # Arguments
    /// * `timeout` - 发现超时时间
    /// 
    /// # Returns
    /// 发现的节点列表（不包含本节点）
    pub fn discover(&self, timeout: Duration) -> Result<Vec<DiscoveredNode>> {
        // 实现...
    }
    
    /// 持续监听新节点加入
    /// 
    /// # Returns
    /// 接收新发现节点的 channel
    pub fn watch(&self) -> Result<tokio::sync::mpsc::Receiver<DiscoveredNode>> {
        // 实现...
    }
    
    /// 停止 mDNS 服务
    pub fn shutdown(self) -> Result<()> {
        // 实现...
    }
}

impl Drop for MdnsService {
    /// 确保资源释放
    fn drop(&mut self) {
        // 实现...
    }
}
```

---

## 技术规格

### mDNS 服务类型
```
_service type: _cis._tcp.local
_service name: {node_id}
_port: 7676 (可配置)
_txt records: 
  - node_id={node_id}
  - did={did}
  - version=1.1.3
  - caps=memory_sync,skill_invoke
```

### 错误处理
- 网络不可用时返回 `Err(anyhow!("Network unavailable"))`
- 权限不足时返回 `Err(anyhow!("Permission denied"))`
- 超时返回空列表（非错误）

---

## 实现步骤

1. **创建文件** `cis-core/src/p2p/mdns_service.rs`
2. **实现 MdnsService::new**
   - 创建 ServiceDaemon
   - 构建 ServiceInfo
   - 注册服务
3. **实现 MdnsService::discover**
   - 调用 browse()
   - 收集 ServiceResolved 事件
   - 过滤本节点
   - 超时后返回
4. **实现 MdnsService::watch**
   - 创建 channel
   - 在后台任务中持续监听
5. **实现 Drop trait**
   - 确保 daemon 正确关闭
6. **添加到 mod.rs**
   - `pub mod mdns_service;`
   - `pub use mdns_service::{MdnsService, DiscoveredNode};`

---

## 单元测试要求

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mdns_service_creation() {
        // 测试创建服务
    }
    
    #[tokio::test]
    async fn test_mdns_discover_self() {
        // 测试不会发现自己
    }
    
    #[tokio::test]
    async fn test_mdns_discover_timeout() {
        // 测试超时返回空列表
    }
    
    #[tokio::test]
    async fn test_mdns_shutdown() {
        // 测试资源释放
    }
}
```

**覆盖率要求**: > 80%

---

## 验收标准

- [ ] `MdnsService::new` 成功创建并广播服务
- [ ] 两台同一局域网机器可以互相发现
- [ ] 发现超时后返回空列表（不 panic）
- [ ] 服务停止后资源正确释放（无内存泄漏）
- [ ] 单测通过率 100%，覆盖率 > 80%
- [ ] 代码通过 `cargo clippy` 检查

---

## 集成验证

```bash
# 终端 1 (本机)
cargo test --package cis-core test_mdns -- --nocapture

# 终端 2 (同局域网另一台机器，可选)
# 运行 discovery 测试，验证是否能发现终端 1
```

---

## 参考文档

- [mdns-sd crate docs](https://docs.rs/mdns-sd)
- [mDNS RFC 6762](https://tools.ietf.org/html/rfc6762)
- 现有代码: `cis-core/src/p2p/discovery.rs`

---

## 输出文件清单

```
cis-core/src/p2p/
├── mdns_service.rs      # 主要实现 (必须)
├── mod.rs               # 添加导出 (修改)
└── tests/
    └── mdns_service_test.rs  # 单元测试 (必须)
```

---

## 阻塞关系

**阻塞以下任务**:
- T2.1: P2P Network 状态管理
- T3.1: p2p discover 命令

**被阻塞于**: 无
