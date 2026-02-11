# CIS v1.1.3 剩余任务清单

**更新时间**: 2026-02-10  
**已完成**: T-P0.1 ✅  
**剩余任务**: 8 个 (P0: 1, P1: 7)

---

## 🔴 P0 - 核心功能模拟 (必须修复)

| 任务 | 模块 | 问题文件 | 预估 | 分配 | 状态 |
|------|------|----------|------|------|------|
| T-P0.1 | AI/Embedding | `embedding.rs:380` | 4h | Agent-F | ✅ **已完成** |
| T-P0.2 | OpenCode | `opencode.rs:100` | 2h | Agent-A | ⏳ 待开始 |

### T-P0.2 详情
**问题**: 通过 prompt 注入模拟多轮对话  
**修复**: 使用 `opencode continue -c <session_id>` 实现真实 session  
```rust
// 当前: 模拟
let prompt = format!("{previous_context}\nUser: {new_message}\nAssistant:");

// 目标: 真实 session
Command::new("opencode")
    .arg("continue")
    .arg("-c")
    .arg(&self.session_id)
```

---

## 🟡 P1 - 重要功能不完整

| 任务 | 模块 | 问题文件 | 预估 | 分配 | 状态 |
|------|------|----------|------|------|------|
| T-P1.1 | Matrix CORS | `server.rs:70`, `federation/server.rs:195` | 2h | Agent-C | ⏳ 待开始 |
| T-P1.2 | Matrix UDP | `websocket/client.rs:323` | 6h | Agent-D | ⏳ 待开始 |
| T-P1.3 | Matrix Challenge | `websocket/client.rs:583` | 4h | Agent-B | ⏳ 待开始 |
| T-P1.4 | Matrix mDNS | `federation/mod.rs:49` | 2h | Agent-E | ⏳ 待开始 |
| T-P1.5 | Scheduler | `skill_executor.rs:327,355` | 3h | Agent-F | ⏳ 待开始 |
| T-P1.6 | Matrix Cloud | `cloud/client.rs:779` | 3h | Agent-A | ⏳ 待开始 |
| T-P1.7 | Federation | `federation_impl.rs:681` | 2h | Agent-B | ⏳ 待开始 |

### 任务详情

#### T-P1.1: Matrix CORS 配置
**问题**: `.allow_origin(Any)` 生产环境不安全  
**修复**: 从 MatrixConfig 读取 allowed_origins 配置

#### T-P1.2: Matrix UDP 直连
**问题**: `// TODO: 建立 UDP 直连`  
**修复**: 使用 `P2PNetwork::global().connect_udp(addr)`

#### T-P1.3: Matrix Challenge Response
**问题**: `let response = b"placeholder";`  
**修复**: 实现 Noise protocol 握手 (`Noise_XX_25519_ChaChaPoly_BLAKE2s`)

#### T-P1.4: Matrix mDNS 发现
**问题**: `// ✅ Optional mDNS discovery (placeholder)`  
**修复**: 集成 `MdnsService::discover_with_type("_matrix._tcp.local")`

#### T-P1.5: Scheduler 模拟等待
**问题**: `tokio::time::sleep(Duration::from_secs(2))`  
**修复**: 使用 `mpsc::Receiver` 等待真实用户输入

#### T-P1.6: Matrix Cloud 配额模拟
**问题**: `let quota_used = 0.5; // 模拟 50% 使用`  
**修复**: 调用 `/_matrix/client/v3/admin/whois` API

#### T-P1.7: Federation 占位
**问题**: `return Ok(json!({"status": "ok"}));`  
**修复**: 使用 `FederationClient::send_event(event)`

---

## 可并行任务组

### 立即开始 (无依赖)
- [ ] T-P0.2 (Agent-A)
- [ ] T-P1.1 (Agent-C) 
- [ ] T-P1.4 (Agent-E)
- [ ] T-P1.5 (Agent-F)
- [ ] T-P1.6 (Agent-A)
- [ ] T-P1.7 (Agent-B)

### 需要依赖
- [ ] T-P1.2 (Agent-D) - 依赖 P2PNetwork UDP
- [ ] T-P1.3 (Agent-B) - 依赖 Noise protocol

---

## Agent 工作负载

| Agent | 任务数 | 任务 |
|-------|--------|------|
| Agent-A | 2 | T-P0.2, T-P1.6 |
| Agent-B | 2 | T-P1.3, T-P1.7 |
| Agent-C | 1 | T-P1.1 |
| Agent-D | 1 | T-P1.2 |
| Agent-E | 1 | T-P1.4 |
| Agent-F | 1 | T-P1.5 (T-P0.1 ✅ 已完成) |
