# Agent-A 任务分配

**Agent 标识**: Agent-A  
**任务**: T1.1 + T3.1  
**技能要求**: 网络编程、mDNS、局域网发现  
**优先级**: P0 (最高)  
**预估总时间**: 7 小时

---

## 任务清单

### 任务 1: T1.1 - mDNS 服务封装
**文件**: `plan/tasks/T1.1_mdns_service/README.md`  
**时间**: 4h  
**状态**: 🔴 立即开始

**核心目标**:
- 封装 `mdns-sd` crate
- 提供 `MdnsService` 和 `DiscoveredNode` 接口
- 实现局域网节点发现

**关键接口**:
```rust
impl MdnsService {
    pub fn new(node_id: &str, port: u16, did: &str, metadata: HashMap<String, String>) -> Result<Self>;
    pub fn discover(&self, timeout: Duration) -> Result<Vec<DiscoveredNode>>;
    pub fn shutdown(self) -> Result<()>;
}
```

**输出文件**:
- `cis-core/src/p2p/mdns_service.rs`
- `cis-core/src/p2p/tests/mdns_service_test.rs`

---

### 任务 2: T3.1 - p2p discover 命令真实实现
**文件**: `plan/tasks/T3.1_p2p_discover_cmd/README.md`  
**时间**: 3h  
**状态**: 🔴 等待 T2.1 完成后开始（或先准备）

**核心目标**:
- 替换 `cis-node/src/commands/p2p.rs` 中的模拟发现代码
- 删除硬编码的 node-abc123/node-def456
- 使用真实的 `P2PNetwork::discovered_peers()`

**关键修改**:
```rust
// 替换 discover_nodes 函数
async fn discover_nodes(timeout_secs: u64, verbose: bool) -> Result<()> {
    let network = P2PNetwork::global().await
        .ok_or_else(|| anyhow!("P2P network not started"))?;
    let peers = network.discovered_peers().await;
    // 显示真实发现的节点
}
```

---

## 执行顺序

```
┌─────────────────────────────────────────────────────┐
│  1. T1.1 (4h)                                        │
│     - 实现 MdnsService                              │
│     - 编写单元测试                                  │
│     - 提交 PR                                        │
│                                                      │
│     ↓ 提交后并行                                     │
│                                                      │
│  2. T3.1 (3h)                                        │
│     - 准备代码（使用 mock P2PNetwork）              │
│     - 等待 T2.1 合并后切换到真实实现                │
│     - 提交 PR                                        │
└─────────────────────────────────────────────────────┘
```

---

## 协作接口

**你提供的接口** (供其他 Agent 使用):
```rust
// T1.1 完成后，其他 Agent 可以通过：
pub use cis_core::p2p::mdns_service::{MdnsService, DiscoveredNode};
```

**你依赖的接口** (需要其他 Agent 提供):
```rust
// T2.1 (Agent-D) 提供：
pub use cis_core::p2p::network::P2PNetwork;
```

---

## 验收标准

### T1.1 验收
- [ ] `MdnsService::new` 成功创建服务
- [ ] 两台同一局域网机器可以互相发现
- [ ] 发现超时后返回空列表（不 panic）
- [ ] 服务停止后资源正确释放
- [ ] 单测覆盖率 > 80%

### T3.1 验收
- [ ] 无硬编码节点
- [ ] 真实发现同网段节点
- [ ] 网络未启动时给出明确错误
- [ ] 超时后正确返回

---

## 联系方式

**依赖你的 Agent**:
- Agent-D (T2.1) - 使用你的 MdnsService

**你依赖的 Agent**:
- Agent-D (T2.1) - 提供 P2PNetwork 接口

---

## 测试验证

```bash
# T1.1 单元测试
cargo test --package cis-core mdns -- --nocapture

# T3.1 集成测试（两台机器）
# 机器 A
cis p2p start
cis p2p discover

# 机器 B
cis p2p start
cis p2p discover  # 应该发现机器 A
```

---

## 开始工作

1. 阅读完整任务文档: `plan/tasks/T1.1_mdns_service/README.md`
2. 创建分支: `git checkout -b agent-a/t1.1-mdns`
3. 开始实现
4. 完成后提交 PR

**祝你好运！**
