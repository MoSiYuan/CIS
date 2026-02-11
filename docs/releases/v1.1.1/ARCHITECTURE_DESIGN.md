# CIS 安全机制架构设计：核心 vs Skill

## 设计原则

**安全机制必须分两层：**
1. **核心层（强制）**: TrustState 检查、通信拦截 - 不可绕过
2. **Skill 层（可扩展）**: DID 验证流程、安全策略 - 可自定义

```
┌─────────────────────────────────────────────────────────────────┐
│                         应用层                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │  cis-node   │  │  cis-gui    │  │  第三方客户端            │  │
│  │   (CLI)     │  │   (GUI)     │  │                         │  │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘  │
│         │                │                      │                │
│         └────────────────┼──────────────────────┘                │
│                          ▼                                       │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Security Skill (可插拔)                      │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │   │
│  │  │ DID 验证流程  │  │ 安全策略配置  │  │ 审计日志     │   │   │
│  │  │ - Challenge  │  │ - 自动验证规则│  │ - 事件记录   │   │   │
│  │  │ - Response   │  │ - 拦截策略    │  │ - 告警通知   │   │   │
│  │  │ - 签名验证   │  │ - 超时设置    │  │ - 分析报告   │   │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘   │   │
│  │                                                          │   │
│  │  接口: verify_peer(), get_security_policy(), log_event() │   │
│  └──────────────────────────┬───────────────────────────────┘   │
│                             │                                    │
│         ┌───────────────────┼───────────────────┐               │
│         │                   │                   │               │
│         ▼                   ▼                   ▼               │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────────────┐   │
│  │  CLI Hook   │   │  GUI Hook   │   │   Matrix Core       │   │
│  │  - 命令拦截 │   │  - 事件处理 │   │  ┌───────────────┐  │   │
│  │  - 状态显示 │   │  - 界面渲染 │   │  │ TrustState    │  │   │
│  │  - 交互流程 │   │  - 对话框   │   │  │ 检查          │  │   │
│  └─────────────┘   └─────────────┘   │  ├───────────────┤  │   │
│                                       │  │ 通信拦截      │  │   │
│                                       │  │ (不可绕过)    │  │   │
│                                       │  └───────────────┘  │   │
│                                       └─────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 核心层（强制）设计

### 最小核心修改

```rust
// cis-core/src/matrix/federation/types.rs

/// 信任状态 - 核心层只保留状态，策略交给 Skill
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrustState {
    Unknown,
    Pending { discovered_at: i64 },
    Verified { verified_at: i64, did: String },
    Blocked { blocked_at: i64 },  // 核心层不记录具体原因，只记录状态
}

/// PeerInfo 扩展 - 最小化修改
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    // ... 原有字段 ...
    pub trust_state: TrustState,
    pub expected_did: Option<String>,  // 用于验证
}

impl PeerInfo {
    /// 核心层：是否允许通信（强制检查）
    pub fn can_communicate(&self) -> bool {
        matches!(self.trust_state, TrustState::Verified { .. })
    }
}
```

### 通信拦截（核心层）

```rust
// cis-core/src/matrix/federation/tunnel.rs

impl Tunnel {
    /// 发送事件前强制检查 - 不可绕过
    pub async fn send_event(&self, event: &CisMatrixEvent) -> Result<(), TunnelError> {
        // 查询目标节点的 trust_state
        let peer = self.get_peer(&event.target).await?;
        
        if !peer.can_communicate() {
            // 记录拦截日志
            tracing::warn!(
                "Event blocked: target={} state={:?}",
                event.target, peer.trust_state
            );
            
            return Err(TunnelError::TrustCheckFailed {
                target: event.target.clone(),
                state: peer.trust_state.clone(),
                // 提供解决指引
                help: format!(
                    "Node not verified. Run 'cis node verify {}' or use GUI to verify",
                    event.target
                ),
            });
        }
        
        // 继续发送...
        self.do_send_event(event).await
    }
}
```

---

## Security Skill 设计

### Skill 结构

```rust
// skills/cis-security/src/lib.rs

use cis_core::skill::{Skill, SkillContext, Event};
use cis_core::matrix::federation::types::{PeerInfo, TrustState};

/// Security Skill - 提供 DID 验证和安全策略
pub struct SecuritySkill {
    config: SecurityConfig,
    verifier: DidVerifier,
    audit_logger: AuditLogger,
}

impl SecuritySkill {
    /// 验证节点（核心功能）
    pub async fn verify_peer(
        &self,
        peer: &mut PeerInfo,
        options: VerifyOptions,
    ) -> Result<VerificationResult, SecurityError> {
        // 1. 发送 DID Challenge
        let challenge = self.create_challenge(&peer.server_name);
        
        // 2. 等待 Response（带超时）
        let response = self.wait_for_response(&challenge, options.timeout).await?;
        
        // 3. 验证签名
        self.verifier.verify_signature(&challenge, &response)?;
        
        // 4. 检查 DID 匹配
        if let Some(expected) = &peer.expected_did {
            if &response.did != expected {
                // DID 不匹配 - 可能的攻击
                self.audit_logger.log(SecurityEvent::DidMismatch {
                    peer: peer.server_name.clone(),
                    expected: expected.clone(),
                    actual: response.did.clone(),
                }).await;
                
                return Err(SecurityError::DidMismatch);
            }
        }
        
        // 5. 更新状态
        peer.trust_state = TrustState::Verified {
            verified_at: now(),
            did: response.did,
        };
        
        // 6. 记录审计日志
        self.audit_logger.log(SecurityEvent::VerificationSucceeded {
            peer: peer.server_name.clone(),
            did: response.did.clone(),
        }).await;
        
        Ok(VerificationResult::Success { did: response.did })
    }
    
    /// 拦截节点
    pub async fn block_peer(&self, peer: &mut PeerInfo, reason: BlockReason) {
        peer.trust_state = TrustState::Blocked {
            blocked_at: now(),
        };
        
        self.audit_logger.log(SecurityEvent::PeerBlocked {
            peer: peer.server_name.clone(),
            reason,
        }).await;
    }
    
    /// 获取安全策略
    pub fn get_policy(&self) -> SecurityPolicy {
        self.config.policy.clone()
    }
}

#[async_trait]
impl Skill for SecuritySkill {
    fn name(&self) -> &str { "cis_security" }
    
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::Custom { name, data } => match name.as_str() {
                "security:verify_peer" => {
                    let peer_id = data.get("peer_id").as_str().unwrap();
                    let result = self.verify_peer(peer_id).await;
                    // 发送结果事件
                    ctx.emit_event("security:verify_result", result)?;
                }
                "security:block_peer" => { ... }
                "security:get_policy" => { ... }
                _ => {}
            }
            _ => {}
        }
        Ok(())
    }
}
```

### Skill 配置

```toml
# skills/cis-security/skill.toml
[skill]
name = "cis_security"
version = "0.1.0"
description = "DID verification and security policy enforcement"
auto_load = true  # 安全 Skill 默认自动加载

[config]
# 验证超时（秒）
verify_timeout = 30

# 自动验证策略
[config.auto_verify]
enabled = false  # 默认关闭，需手动验证
# 可配置为：mdns_discovered = "pending", manual_add = "required"

# 审计日志
[config.audit]
enabled = true
storage = "local"  # local, sqlite, remote
retention_days = 90
```

---

## CLI 集成方案

### CLI 命令设计

```bash
# cis-node/src/commands/security.rs

/// 安全相关命令 - 调用 Security Skill
pub mod security {
    /// 验证节点
    pub async fn verify(peer_id: &str, opts: VerifyOptions) -> Result<()> {
        // 调用 Security Skill
        let skill = get_skill("cis_security")?;
        
        let result = skill.call("verify_peer", json!({
            "peer_id": peer_id,
            "expected_did": opts.expected_did,
            "timeout": opts.timeout,
        })).await?;
        
        // 显示结果
        match result {
            Ok(verified) => {
                println!("✓ Node {} verified", peer_id);
                println!("  DID: {}", verified.did);
                println!("  Verified at: {}", verified.verified_at);
            }
            Err(e) => {
                println!("✗ Verification failed: {}", e);
                if let Some(help) = e.help {
                    println!("  Help: {}", help);
                }
            }
        }
        
        Ok(())
    }
    
    /// 查看安全状态
    pub async fn status(peer_id: Option<&str>) -> Result<()> {
        let store = get_federation_store()?;
        
        let peers = match peer_id {
            Some(id) => vec![store.get_peer(id)?],
            None => store.list_peers()?,
        };
        
        println!("{:<20} {:<12} {:<20}", "NODE", "STATE", "DETAILS");
        println!("{}", "-".repeat(60));
        
        for peer in peers {
            let (state_icon, details) = match &peer.trust_state {
                TrustState::Unknown => ("?", "Unknown".to_string()),
                TrustState::Pending { .. } => ("◐", "Verification pending".to_string()),
                TrustState::Verified { did, .. } => ("✓", format!("Verified: {}", did)),
                TrustState::Blocked { .. } => ("✗", "Blocked".to_string()),
            };
            
            println!("{:<20} {:<12} {:<20}", 
                peer.server_name, 
                state_icon,
                details
            );
        }
        
        Ok(())
    }
    
    /// 拦截节点
    pub async fn block(peer_id: &str, reason: &str) -> Result<()> {
        let skill = get_skill("cis_security")?;
        skill.call("block_peer", json!({
            "peer_id": peer_id,
            "reason": reason,
        })).await?;
        
        println!("✓ Node {} blocked", peer_id);
        Ok(())
    }
    
    /// 列出审计日志
    pub async fn audit_logs(filter: LogFilter) -> Result<()> {
        let skill = get_skill("cis_security")?;
        let logs = skill.call("get_audit_logs", json!({
            "filter": filter,
        })).await?;
        
        for log in logs {
            println!("[{}] {} - {:?}", log.timestamp, log.event_type, log.details);
        }
        
        Ok(())
    }
}
```

### CLI 交互流程

```bash
# 示例：CLI 验证流程

$ cis node list
NODE                 STATE    ADDRESS
desk-local           ✓        192.168.1.100:6767
laptop-remote        ◐        192.168.1.105:6767  # 待验证

$ cis security verify laptop-remote
🔍 Verifying node "laptop-remote" at 192.168.1.105:6767...
📡 Sending DID Challenge...
⏳ Waiting for response (timeout: 30s)...
✓ Signature verified
✓ DID matched: did:cis:laptop:abc123
✓ Node verified successfully!

$ cis node list
NODE                 STATE    ADDRESS
desk-local           ✓        192.168.1.100:6767
laptop-remote        ✓        192.168.1.105:6767  # 已验证

# 现在可以通信了
$ cis message send laptop-remote "hello"
✓ Message sent
```

---

## 调整后的开发计划

### 优先级调整

```
Phase 0: 核心层最小修改 (1天) ────┐
Phase 1: Security Skill (2天) ────┼─── CLI 可用 ───┐
Phase 2: CLI 集成 (1天) ──────────┘                ├─── GUI 集成
Phase 3: GUI 界面 (3天) ───────────────────────────┘
Phase 4: 远程 Agent (3天)
Phase 5: 测试 (2天)
```

### 新任务拆分

#### Phase 0: 核心层最小修改 (1天)

**Task 0.1: TrustState 核心数据结构 (0.5天)**
- 文件: `cis-core/src/matrix/federation/types.rs`
- 内容: 添加 `TrustState` enum, `PeerInfo` 扩展
- 关键: **最小修改**，只保留状态，策略交给 Skill

**Task 0.2: 核心通信拦截 (0.5天)**
- 文件: `cis-core/src/matrix/federation/tunnel.rs`
- 内容: `send_event()` 前添加 `can_communicate()` 检查
- 关键: **不可绕过**，返回明确错误信息

#### Phase 1: Security Skill (2天)

**Task 1.1: Security Skill 框架 (0.5天)**
- 文件: `skills/cis-security/src/lib.rs` (新建)
- 内容: Skill 结构，基本接口

**Task 1.2: DID 验证实现 (1天)**
- 文件: `skills/cis-security/src/verifier.rs`
- 内容: Challenge-Response, 签名验证

**Task 1.3: 审计日志 (0.5天)**
- 文件: `skills/cis-security/src/audit.rs`
- 内容: 事件记录，查询接口

#### Phase 2: CLI 集成 (1天)

**Task 2.1: CLI 安全命令 (0.5天)**
- 文件: `cis-node/src/commands/security.rs` (新建)
- 内容: `cis security verify/status/block`

**Task 2.2: 节点命令集成 (0.5天)**
- 文件: `cis-node/src/commands/node.rs`
- 内容: `cis node list` 显示 trust_state

---

## 架构优势

| 方面 | 核心+Skill 架构 | 纯核心架构 |
|------|----------------|-----------|
| **模块化** | ✅ Skill 可独立升级 | ❌ 需要更新核心 |
| **可定制** | ✅ 用户可替换 Security Skill | ❌ 固定实现 |
| **强制安全** | ✅ 核心层拦截不可绕过 | ✅ 同样可做到 |
| **CLI 优先** | ✅ Skill 提供 CLI 接口 | ❌ CLI 需单独实现 |
| **生态扩展** | ✅ 第三方可开发安全策略 | ❌ 只有官方实现 |

---

## 接口契约

### 核心层 <-> Skill 层

```rust
// 核心层提供（只读）
pub trait SecurityContext {
    fn get_peer(&self, id: &str) -> Option<PeerInfo>;
    fn list_peers(&self) -> Vec<PeerInfo>;
    fn update_trust_state(&self, id: &str, state: TrustState);
}

// Skill 层提供
pub trait SecuritySkill {
    async fn verify_peer(&self, id: &str, opts: VerifyOptions) -> Result<VerificationResult>;
    async fn block_peer(&self, id: &str, reason: &str);
    async fn get_audit_logs(&self, filter: LogFilter) -> Vec<AuditLog>;
}
```

### CLI <-> Skill

```bash
# CLI 调用 Skill 的接口
cis security <command> -> SecuritySkill::call(command, args)
```
