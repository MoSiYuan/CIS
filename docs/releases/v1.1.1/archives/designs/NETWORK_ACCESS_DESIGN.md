# CIS 网络准入控制设计

## 核心原则

```
1. 手动配置 DID - 用户自己决定信任谁
2. WebSocket 握手后立即 Challenge - 无延迟验证
3. 白名单准入 - 只有白名单 DID 能加入网络
4. DNS 式同步 - 黑白名单分布式传播
5. 自闭模式 - 完全隔离，只出不进
```

## 网络准入模式

```rust
pub enum NetworkMode {
    /// 开放模式 - 验证 DID 即可加入（不安全，测试用）
    Open,
    
    /// 白名单模式 - DID 在白名单才能加入（推荐）
    Whitelist,
    
    /// 自闭模式 - 禁止任何新节点加入，只与已验证节点通信
    Solitary,
    
    /// 观察模式 - 允许连接但不转发数据（类似 Shadow Ban）
    Quarantine,
}

pub struct AccessControl {
    pub mode: NetworkMode,
    pub whitelist: HashSet<String>,  // DID 白名单
    pub blacklist: HashSet<String>,  // DID 黑名单
    pub acl_version: u64,            // ACL 版本号，用于同步
    pub acl_signature: String,       // 管理员签名
}
```

## 节点加入流程

```
用户手动配置阶段:
                              
用户 A                          配置
─────► 发现节点 B (mDNS/手动输入) 
      获取 B 的 DID: did:cis:node-b:abc123
      ▼
      将 did:cis:node-b:abc123 加入白名单
      ▼
      保存到 ~/.cis/whitelist.toml
                              
网络连接阶段:
                              
A:6767 ◄─────────────────────── B:xxxxx
       WebSocket 连接请求
       
       ───────────────────────►
       立即发送 DID Challenge
       {
         "type": "did_challenge",
         "nonce": "random-128-bit",
         "challenger_did": "did:cis:node-a:xyz789",
         "timestamp": 1234567890
       }
       
       ◄───────────────────────
       DID Response
       {
         "type": "did_response", 
         "responder_did": "did:cis:node-b:abc123",
         "challenge_signature": "sig..."
       }
       
A 验证:
1. 解析 responder_did 获取公钥
2. 验证 challenge_signature
3. 检查 responder_did 是否在本地白名单
4. 全部通过 → Verified
5. 开始正常通信
```

## 数据结构

### 节点配置（本地）

```rust
// ~/.cis/network_acl.toml
#[derive(Serialize, Deserialize)]
pub struct NetworkAcl {
    /// 本节点 DID（由 identity/did.rs 生成）
    pub local_did: String,
    
    /// 准入模式
    pub mode: NetworkMode,
    
    /// 白名单 - 显式允许的 DID
    pub whitelist: Vec<AclEntry>,
    
    /// 黑名单 - 显式拒绝的 DID
    pub blacklist: Vec<AclEntry>,
    
    /// ACL 版本（用于同步）
    pub version: u64,
    
    /// 最后更新时间
    pub updated_at: i64,
    
    /// 管理签名（防止篡改）
    pub signature: Option<String>,
}

pub struct AclEntry {
    pub did: String,
    pub added_at: i64,
    pub added_by: String,      // 谁添加的（本节点 DID 或其他节点 DID）
    pub reason: Option<String>, // 添加原因
    pub expires_at: Option<i64>, // 过期时间（可选）
}

impl NetworkAcl {
    /// 检查 DID 是否允许连接
    pub fn is_allowed(&self, did: &str) -> AclResult {
        match self.mode {
            NetworkMode::Solitary => AclResult::Denied("Solitary mode".into()),
            
            NetworkMode::Whitelist => {
                if self.blacklist.iter().any(|e| e.did == did) {
                    AclResult::Denied("In blacklist".into())
                } else if self.whitelist.iter().any(|e| e.did == did) {
                    AclResult::Allowed
                } else {
                    AclResult::Denied("Not in whitelist".into())
                }
            }
            
            NetworkMode::Open => {
                if self.blacklist.iter().any(|e| e.did == did) {
                    AclResult::Denied("In blacklist".into())
                } else {
                    AclResult::Allowed
                }
            }
            
            NetworkMode::Quarantine => AclResult::Quarantine,
        }
    }
}
```

### 配置示例

```toml
# ~/.cis/network_acl.toml
local_did = "did:cis:my-node:abc123"
mode = "whitelist"  # solitary / whitelist / open / quarantine
version = 42
updated_at = 1704067200

[[whitelist]]
did = "did:cis:workstation:def456"
added_at = 1704067200
added_by = "did:cis:my-node:abc123"
reason = "Work laptop"

[[whitelist]]
did = "did:cis:server:ghi789"
added_at = 1703980800
added_by = "did:cis:my-node:abc123" 
reason = "Home server"

[[blacklist]]
did = "did:cis:attacker:suspicious"
added_at = 1703894400
added_by = "did:cis:my-node:abc123"
reason = "DID mismatch during verification"
expires_at = 1706486400  # 30天后过期
```

## 黑白名单 DNS 式同步

### 同步协议

```rust
/// ACL 更新事件（通过 Matrix Federation 传播）
pub struct AclUpdateEvent {
    /// 事件类型
    pub action: AclAction,
    
    /// 目标 DID
    pub target_did: String,
    
    /// ACL 版本号（单调递增）
    pub version: u64,
    
    /// 更新时间
    pub timestamp: i64,
    
    /// 更新者 DID
    pub updated_by: String,
    
    /// 更新者签名（证明身份）
    pub signature: String,
}

pub enum AclAction {
    AddToWhitelist,
    RemoveFromWhitelist,
    AddToBlacklist,
    RemoveFromBlacklist,
    FullSyncRequest,   // 请求完整 ACL
    FullSyncResponse,  // 返回完整 ACL
}

/// ACL 同步器
pub struct AclSync {
    local_acl: Arc<RwLock<NetworkAcl>>,
    pending_updates: Vec<AclUpdateEvent>,
}

impl AclSync {
    /// 广播 ACL 更新给所有已验证节点
    pub async fn broadcast_update(&self, update: AclUpdateEvent) {
        for peer in self.get_verified_peers().await {
            if let Err(e) = self.send_acl_update(&peer, &update).await {
                warn!("Failed to send ACL update to {}: {}", peer.did, e);
            }
        }
    }
    
    /// 接收 ACL 更新
    pub async fn on_acl_update(&self, update: AclUpdateEvent) -> Result<()> {
        // 1. 验证签名
        self.verify_signature(&update)?;
        
        // 2. 检查更新者是否在白名单（只有信任的节点才能更新我的 ACL）
        if !self.is_trusted_updater(&update.updated_by) {
            return Err(Error::UntrustedUpdater);
        }
        
        // 3. 检查版本号（必须是新的）
        let local_version = self.local_acl.read().await.version;
        if update.version <= local_version {
            return Ok(()); // 旧版本，忽略
        }
        
        // 4. 应用更新
        self.apply_update(update).await?;
        
        // 5. 继续广播（类似 DNS 传播）
        self.broadcast_update(update).await;
        
        Ok(())
    }
}
```

### 同步流程

```
场景 1: 添加节点到白名单
                              
用户 A                          用户 B
  │                              │
  ▼                              ▼
┌─────────┐                   ┌─────────┐
│ 添加 DID │                   │ 已验证  │
│ 到白名单 │                   │ 节点    │
└────┬────┘                   └────┬────┘
     │                             │
     │ 1. 更新本地 ACL             │
     │    version += 1             │
     │                             │
     │ 2. 广播 AclUpdateEvent      │
     │────────────────────────────►│
     │    {                        │
     │      action: AddToWhitelist,│
     │      target_did: "did:C",   │
     │      version: 43,           │
     │      updated_by: "did:A",   │
     │      signature: "sig..."    │
     │    }                        │
     │                             │
     │                             │ 3. 验证签名
     │                             │    验证版本
     │                             │    更新本地 ACL
     │                             │
     │                             │ 4. 继续广播给其他节点
     │                             │─────────────────────► C
     │                             │
                              
场景 2: 新节点加入，请求完整 ACL
                              
新节点 D                       已验证节点 A
  │                              │
  │ 1. 建立 WebSocket            │
  │─────────────────────────────►│
  │                              │
  │ 2. DID Challenge/Response   │
  │◄────────────────────────────►│
  │                              │
  │ 3. 验证通过，发送            │
  │    FullSyncRequest           │
  │─────────────────────────────►│
  │                              │
  │ 4. 返回 FullSyncResponse     │
  │◄─────────────────────────────│
  │    {                         │
  │      acl: NetworkAcl,        │
  │      signature: "sig..."     │
  │    }                         │
  │                              │
  │ 5. D 验证签名后应用 ACL      │
  │                              │
```

## 自闭模式 (Solitary)

```rust
impl NetworkAcl {
    /// 进入自闭模式
    pub fn enter_solitary_mode(&mut self) {
        self.mode = NetworkMode::Solitary;
        
        // 断开所有非白名单连接
        for peer in self.get_all_peers() {
            if !self.whitelist.contains(&peer.did) {
                self.disconnect(&peer.did);
            }
        }
        
        // 不再接受新连接
        info!("Entered solitary mode. Only whitelisted peers can connect.");
    }
}

// 连接处理
async fn on_connect(&self, stream: TcpStream) {
    let peer_addr = stream.peer_addr();
    
    // 检查模式
    if self.acl.mode == NetworkMode::Solitary {
        // 自闭模式下，必须先知道对方 DID 才能决定是否接受
        // 所以先完成 WebSocket 握手和 DID Challenge
        // 然后检查 DID 是否在白名单
    }
}
```

## CLI 命令

```bash
# 查看当前网络准入配置
cis network status
# Mode: whitelist
# Local DID: did:cis:my-node:abc123
# Whitelist: 5 entries
# Blacklist: 1 entry
# ACL Version: 42

# 添加 DID 到白名单
cis network allow <did> [--reason "Work laptop"]
# 自动广播给其他节点

# 添加 DID 到黑名单
cis network deny <did> [--reason "Suspicious activity"] [--expires 30d]

# 移除白名单/黑名单
cis network unallow <did>
cis network undeny <did>

# 切换模式
cis network mode whitelist  # 白名单模式
cis network mode solitary   # 自闭模式
cis network mode open       # 开放模式（不安全）

# 查看黑白名单
cis network list whitelist
cis network list blacklist

# 手动同步 ACL（从指定节点拉取）
cis network sync --from <peer-id>

# 查看 ACL 传播日志
cis network acl-log
# [2024-01-15 14:32:10] did:A added did:B to whitelist (v43)
# [2024-01-15 14:32:15] Received ACL update from did:A (v43)
# [2024-01-15 14:32:20] Broadcasted ACL update to 3 peers
```

## 安全配置向导

```bash
$ cis init
...

Network Access Configuration:
1. Solitary mode - Only explicit whitelisted peers can connect (most secure)
2. Whitelist mode - New peers need whitelist approval (recommended)
3. Open mode - Accept any verified peer (not recommended)

Select mode [1-3]: 2

Enter your peer's DID to whitelist (or skip for now):
> did:cis:work-laptop:def456
Reason: My work laptop

Whitelist entry added.
Add more? [y/N]: n

Your network is configured in whitelist mode.
Only the following peers can join:
  - did:cis:work-laptop:def456 (My work laptop)

To add more peers later, use: cis network allow <did>
```

## 冲突解决

```rust
/// 多个节点同时更新 ACL 时的冲突解决
pub fn resolve_acl_conflict(updates: Vec<AclUpdateEvent>) -> Vec<AclUpdateEvent> {
    // 策略：时间戳优先，时间戳相同则 DID 字典序小的优先
    let mut latest: HashMap<String, AclUpdateEvent> = HashMap::new();
    
    for update in updates {
        let key = format!("{}:{:?}", update.target_did, update.action);
        
        if let Some(existing) = latest.get(&key) {
            if update.timestamp > existing.timestamp {
                latest.insert(key, update);
            } else if update.timestamp == existing.timestamp {
                // 时间戳相同，DID 字典序小的优先
                if update.updated_by < existing.updated_by {
                    latest.insert(key, update);
                }
            }
        } else {
            latest.insert(key, update);
        }
    }
    
    latest.into_values().collect()
}
```

## 安全考虑

### 1. ACL 篡改防护

```rust
// 每个 ACL 版本都必须由管理员签名
pub fn sign_acl(acl: &NetworkAcl, signing_key: &SigningKey) -> String {
    let acl_bytes = serialize_acl_without_signature(acl);
    let signature = signing_key.sign(&acl_bytes);
    hex::encode(signature.to_bytes())
}

pub fn verify_acl_signature(acl: &NetworkAcl) -> Result<()> {
    let signature = Signature::from_bytes(&hex::decode(&acl.signature)?)?;
    let acl_bytes = serialize_acl_without_signature(acl);
    let verifying_key = resolve_did(&acl.local_did)?;
    verifying_key.verify(&acl_bytes, &signature)?;
    Ok(())
}
```

### 2. 更新者身份验证

```rust
// 只有白名单中的节点才能更新我的 ACL
pub fn is_trusted_updater(&self, updater_did: &str) -> bool {
    // 自己肯定可以
    if updater_did == self.local_did {
        return true;
    }
    
    // 白名单中的节点也可以（可选配置）
    self.whitelist.contains(updater_did)
}
```

### 3. 防止版本回滚攻击

```rust
// 绝不接受旧版本
if update.version <= local_version {
    return Err(Error::VersionTooOld);
}

// 版本号必须是连续的
if update.version != local_version + 1 {
    // 可能漏了更新，请求完整同步
    self.request_full_sync(&update.updated_by).await?;
}
```
