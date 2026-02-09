# T3.1: p2p discover 命令真实实现

**任务编号**: T3.1  
**任务名称**: Real p2p discover Command  
**优先级**: P1  
**预估时间**: 3 小时  
**依赖**: T2.1 (P2P Network)  
**分配状态**: 待分配

---

## 任务概述

替换 `cis p2p discover` 命令中的模拟实现，使用真实的 P2P 网络发现功能。

---

## 输入

### 依赖任务输出
- **T2.1**: `P2PNetwork` 全局实例管理

### 待修改文件
- **文件**: `cis-node/src/commands/p2p.rs` (298-347 行)

### 当前模拟代码
```rust
// 当前实现（待删除）
async fn discover_nodes(timeout_secs: u64, verbose: bool) -> Result<()> {
    // 模拟发现过程
    for i in 0..timeout_secs {
        if i == 3 {
            pb.println("  📡 Found node: node-abc123 @ 192.168.1.100:7677");  // 硬编码！
        }
        if i == 5 {
            pb.println("  📡 Found node: node-def456 @ 192.168.1.101:7677");  // 硬编码！
        }
    }
    println!("\nDiscovered 2 nodes:");  // 假数据！
}
```

---

## 输出要求

### 替换后的实现

```rust
// 文件: cis-node/src/commands/p2p.rs

/// 发现节点（真实实现）
async fn discover_nodes(timeout_secs: u64, verbose: bool) -> Result<()> {
    use cis_core::p2p::network::P2PNetwork;
    
    // 获取全局 P2P 网络实例
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow::anyhow!(
            "P2P network not started. Run 'cis p2p start' first."
        ))?;
    
    println!("🔍 Discovering nodes...");
    println!("   Timeout: {} seconds\n", timeout_secs);
    
    // 显示进度
    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message("Searching...");
    
    // 等待发现
    let start = std::time::Instant::now();
    loop {
        // 检查超时
        if start.elapsed().as_secs() >= timeout_secs {
            break;
        }
        
        // 更新进度
        let elapsed = start.elapsed().as_secs();
        pb.set_message(format!("Searching... ({}s)", elapsed));
        
        // 检查是否有新节点（非阻塞）
        let peers = network.discovered_peers().await;
        if !peers.is_empty() {
            break;
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    pb.finish_and_clear();
    
    // 获取最终结果
    let peers = network.discovered_peers().await;
    
    if peers.is_empty() {
        println!("❌ No nodes discovered");
        println!("\nPossible reasons:");
        println!("  • No CIS nodes on the same network");
        println!("  • Firewall blocking mDNS (port 6767)");
        println!("  • P2P network not fully started");
        return Ok(());
    }
    
    println!("✅ Discovered {} node(s):\n", peers.len());
    
    for (i, peer) in peers.iter().enumerate() {
        println!("  [{}] {}", i + 1, peer.node_id);
        println!("      Address: {}", peer.address);
        println!("      DID: {}", peer.did);
        
        if verbose {
            println!("      Connected: {}", if peer.connected { "yes" } else { "no" });
            println!("      Last seen: {:?} ago", 
                std::time::SystemTime::now().duration_since(peer.last_seen).unwrap_or_default()
            );
        }
        println!();
    }
    
    println!("Use 'cis p2p connect <address>' to connect to a node.");
    
    Ok(())
}
```

---

## 输出格式要求

### 无节点发现
```
🔍 Discovering nodes...
   Timeout: 10 seconds

❌ No nodes discovered

Possible reasons:
  • No CIS nodes on the same network
  • Firewall blocking mDNS (port 6767)
  • P2P network not fully started
```

### 发现节点（简洁）
```
🔍 Discovering nodes...
   Timeout: 10 seconds

✅ Discovered 2 node(s):

  [1] workstation-node
      Address: 192.168.1.100:7677
      DID: did:cis:workstation123

  [2] laptop-node
      Address: 192.168.1.101:7677
      DID: did:cis:laptop456

Use 'cis p2p connect <address>' to connect to a node.
```

### 发现节点（verbose）
```
🔍 Discovering nodes...
   Timeout: 10 seconds

✅ Discovered 2 node(s):

  [1] workstation-node
      Address: 192.168.1.100:7677
      DID: did:cis:workstation123
      Connected: yes
      Last seen: 2s ago

  [2] laptop-node
      Address: 192.168.1.101:7677
      DID: did:cis:laptop456
      Connected: no
      Last seen: 5s ago
```

---

## 关键约束

### ❌ 禁止事项
- **禁止硬编码节点**: 不能有 node-abc123, node-def456
- **禁止模拟延迟**: 不能用 sleep 假装在搜索
- **禁止假数据**: 所有显示的数据必须来自 P2PNetwork

### ✅ 必须实现
- 检查 P2P 网络是否已启动
- 从 `P2PNetwork::discovered_peers()` 获取真实数据
- 实时显示发现进度
- 正确处理超时

---

## 测试验证

### 手动测试步骤

```bash
# 1. 确保 P2P 未启动
cis p2p stop 2>/dev/null || true

# 2. 尝试发现（应该提示未启动）
cis p2p discover
# 期望: "P2P network not started"

# 3. 启动 P2P
cis p2p start

# 4. 发现节点
cis p2p discover
# 期望: 显示真实发现的节点（或空列表）

# 5. 详细模式
cis p2p discover --verbose
# 期望: 显示额外信息
```

### 集成测试（两台机器）

```bash
# 机器 A
cis p2p start
cis p2p discover --verbose
# 应该发现机器 B

# 机器 B
cis p2p start
cis p2p discover --verbose
# 应该发现机器 A
```

---

## 验收标准

- [ ] 无硬编码的 node-abc123/node-def456
- [ ] 真实发现同网段运行的 CIS 节点
- [ ] 网络未启动时给出明确错误提示
- [ ] 超时后正确返回（不 panic）
- [ ] verbose 模式显示额外信息
- [ ] 单测通过（mock P2PNetwork）

---

## 输出

```
cis-node/src/commands/p2p.rs
```

---

## 阻塞关系

**依赖**:
- T2.1: P2PNetwork 全局实例

**阻塞**: 无
