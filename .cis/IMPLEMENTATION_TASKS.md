# CIS 真实实现任务清单

> **原则**: 所有功能必须真实实现，禁止模拟/占位/简化代码
> 
> **状态**: 🔴 未完成 | 🟡 进行中 | 🟢 已完成

---

## 统计概览

| 类别 | 数量 | 状态 |
|-----|------|------|
| TODO/FIXME/XXX | 169 处 | 待清理 |
| 模拟/占位实现 | 40+ 处 | 待替换 |
| CLI 命令模拟 | 15+ 处 | 待实现 |

---

## 一、P2P 网络层 (最高优先级)

### 1.1 节点发现服务 🔴
**文件**: `cis-node/src/commands/p2p.rs:302-347`

**当前问题**:
```rust
// 模拟发现过程
for i in 0..timeout_secs {
    if i == 3 { pb.println("  📡 Found node: node-abc123 @ 192.168.1.100:7677"); } // 硬编码
    if i == 5 { pb.println("  📡 Found node: node-def456 @ 192.168.1.101:7677"); } // 硬编码
}
```

**真实实现方案**:
- 使用 `cis-core/src/p2p/discovery.rs` 中的 `DiscoveryService`
- 基于 `mdns-sd` 库实现真实的 mDNS 广播和发现
- 服务名: `_cis._tcp.local`
- 端口: 7676

**依赖库**: `mdns-sd` (已配置)

---

### 1.2 P2P 网络启动/停止 🔴
**文件**: `cis-node/src/commands/p2p.rs:585-672`

**当前问题**: 仅打印状态信息，无真实网络启动

**真实实现方案**:
- 使用 `cis-core/src/p2p/mod.rs` 中的 `P2PNetwork`
- 集成 QUIC 传输层 (`QuicTransport`)
- 集成 DHT 服务 (`DhtService`)
- 集成 Gossip 协议 (`GossipProtocol`)
- 需要维护全局 P2P 实例状态

---

### 1.3 节点连接/断开 🔴
**文件**: `cis-node/src/commands/p2p.rs:380-458`

**当前问题**:
```rust
// 模拟连接
println!("  🔄 Connecting to {}...", addr);
tokio::time::sleep(Duration::from_millis(500)).await;
println!("  ✅ Successfully connected to {}", node_id);
```

**真实实现方案**:
- 使用 `P2PNetwork::connect()` 建立 QUIC 连接
- 使用 `PeerManager` 管理连接状态
- 持久化连接信息到本地存储

---

### 1.4 DHT 操作 🔴
**文件**: `cis-node/src/commands/p2p.rs:760-840`

**当前问题**: 
```rust
// 模拟存储
println!("  💾 Storing key '{}' in DHT...", key);
```

**真实实现方案**:
- 使用 `DhtService::put()` / `get()`
- 实现 Kademlia 协议的真实查找
- 维护路由表

**依赖**: `cis-core/src/p2p/dht.rs` (部分实现)

---

## 二、Matrix 服务层

### 2.1 Matrix Server PID 管理 🔴
**文件**: `cis-node/src/commands/matrix.rs:139,155`

```rust
// TODO: Implement PID file tracking and graceful shutdown
// TODO: Check if server is running via PID file
```

**真实实现**:
- PID 文件: `~/.local/share/cis/matrix.pid`
- 启动时写入 PID
- 停止时读取 PID 并发送 SIGTERM
- 状态检查时检测进程是否存在

---

### 2.2 Matrix 端口配置 🔴
**当前问题**: 默认端口混乱

| 功能 | 当前配置 | 正确配置 |
|-----|---------|---------|
| Matrix Server | 7676 (默认) / 8448 (用户指定) | 7676 |
| 节点发现 | 6767 (UDP) | 6767 |
| P2P 传输 | 7677 | 7677 |

**修复**:
- 统一配置文件中端口定义
- 添加端口冲突检测

---

## 三、Agent 持久化层

### 3.1 Agent 进程检测 🔴
**文件**: `cis-core/src/agent/persistent/opencode.rs:569`

```rust
// TODO: 实现进程扫描或端口检测
```

**真实实现**:
- 使用 `sysinfo` 或 `ps` 检测进程
- 端口探测确认服务状态
- PID 文件管理

---

### 3.2 Agent Session 信息获取 🔴
**文件**: `cis-core/src/agent/persistent/claude.rs:577-579`

```rust
last_active_at: s.created_at, // TODO: 从 session 获取最后活动时间
total_tasks: 0, // TODO: 从持久化存储获取
work_dir: std::env::temp_dir().into(), // TODO: 从 session 获取
```

**真实实现**:
- 读取 session 日志文件
- 统计任务数量
- 获取工作目录配置

---

## 四、Agent Federation

### 4.1 Matrix 事件发送 🔴
**文件**: `cis-core/src/agent/federation/agent.rs:192`

```rust
// TODO: 实现实际的 Matrix 事件发送
```

**真实实现**:
- 使用 `MatrixClient` 发送事件
- 实现 FederationClient 完整逻辑

---

### 4.2 Federation 心跳 🔴
**文件**: `cis-core/src/agent/federation/agent.rs:271`

```rust
// TODO: 通过 FederationClient 发送心跳
```

---

### 4.3 Room 事件订阅 🔴
**文件**: `cis-core/src/agent/federation/agent.rs:293`

```rust
// TODO: 订阅 Matrix Room 事件
```

---

### 4.4 远程任务处理 🔴
**文件**: `cis-core/src/agent/federation/agent.rs:320`

```rust
// TODO: 处理远程任务请求
```

---

## 五、网络层

### 5.1 UDP 直连 🔴
**文件**: `cis-core/src/matrix/websocket/client.rs:323`

```rust
// TODO: 建立 UDP 直连（当前版本回退到 WebSocket）
```

---

### 5.2 CORS 配置 🔴
**文件**: 
- `cis-core/src/matrix/server.rs:70`
- `cis-core/src/matrix/federation/server.rs:195`

```rust
.allow_origin(Any)  // TODO: Configure specific origins for production
```

---

### 5.3 节点服务模拟 🔴
**文件**: `cis-core/src/service/node_service.rs:485,495,562`

```rust
// 模拟 ping 操作
// 模拟 RTT
// 返回基于节点 RTT 和状态的模拟统计
```

---

## 六、AI/嵌入层

### 6.1 模拟 Embedding 🔴
**文件**: 
- `cis-core/src/memory/service.rs:929`
- `cis-core/src/task/vector.rs:415-421`
- `cis-core/src/vector/storage.rs:1876-1882`

```rust
/// 模拟 embedding service（用于测试）
/// 简单的确定性模拟：根据文本哈希生成向量
```

**真实实现**:
- 已配置 `fastembed` 库
- 使用 `NomicEmbedTextV15` 模型
- 需要正确初始化和调用

---

### 6.2 Claude 嵌入 🔴
**文件**: `cis-core/src/ai/embedding.rs:380`

```rust
/// 注意：这是一个模拟实现，实际应该调用 Claude CLI 的嵌入功能
```

---

## 七、WASM 层

### 7.1 WASM Host 函数 Stub 🔴
**文件**: `cis-core/src/wasm/host.rs`

大量 stub 实现:
- `host_memory_get`
- `host_memory_set`
- `host_memory_delete`
- `host_ai_chat`
- `host_log`
- `host_http_post`

**真实实现**: 需要完整实现 WASM 宿主函数

---

### 7.2 Mock AI Provider 🔴
**文件**: `cis-core/src/wasm/host.rs:1251-1328`

```rust
Arc::new(Mutex::new(mock_ai::MockAiProvider::new()));
```

---

## 八、调度器/执行器

### 8.1 Skill 执行等待 🔴
**文件**: `cis-core/src/scheduler/skill_executor.rs:327,355`

```rust
// 模拟等待时间（实际应用中这里会等待用户输入）
```

---

### 8.2 GLM 事件 🔴
**文件**: `cis-core/src/glm/mod.rs:242,709-711`

```rust
Ok("mock_event_id".to_string())
let run_id = format!("dag-run-mock-{}-{}", dag.dag_id, uuid::Uuid::new_uuid());
```

---

## 九、CLI 命令层

### 9.1 IM 命令模拟 🔴
**文件**: `cis-node/src/commands/im.rs:502`

```rust
println!("⚠️  IM Skill 未加载，以上为模拟数据");
```

---

### 9.2 Worker 命令 Placeholder 🔴
**文件**: `cis-node/src/commands/worker.rs`

- `show_worker_logs` (placeholder)
- `show_worker_stats` (placeholder)
- `start_worker` (placeholder)
- Task queue depth (placeholder)

---

### 9.3 Session PTY 🔴
**文件**: `cis-node/src/commands/session.rs:366`

```rust
// TODO: Start interactive PTY session here
```

---

## 十、网络 ACL

### 10.1 ACL 广播 Stub 🔴
**文件**: `cis-node/src/commands/network.rs:1287`

```rust
/// Helper: Broadcast ACL update to P2P network (stub when p2p disabled)
```

---

## 实施计划

### Phase 1: 核心网络层 (Week 1)
1. 实现真实 P2P 发现 (mDNS)
2. 实现真实 P2P 连接管理
3. 修复 Matrix PID 管理
4. 统一端口配置

### Phase 2: Agent 层 (Week 2)
1. 实现 Agent 进程检测
2. 完成 Federation 事件发送
3. 实现心跳和订阅

### Phase 3: AI/嵌入层 (Week 3)
1. 替换模拟 Embedding
2. 修复 Claude Provider
3. 完成向量存储

### Phase 4: WASM/执行层 (Week 4)
1. 实现 WASM Host 函数
2. 替换 GLM mock
3. 完善 Skill 执行

### Phase 5: 清理 (Week 5)
1. 移除所有模拟代码
2. 添加集成测试
3. 端到端验证

---

## 验收标准

- [ ] `cis p2p discover` 发现真实节点
- [ ] `cis p2p connect` 建立真实连接
- [ ] `cis matrix start/stop` 真实启动/停止进程
- [ ] `cis agent execute` 调用真实 AI provider
- [ ] `cis dag run` 完整执行 DAG
- [ ] 多节点组网测试通过
