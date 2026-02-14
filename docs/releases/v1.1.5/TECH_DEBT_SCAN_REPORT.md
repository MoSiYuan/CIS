# CIS Core 技术债务扫描报告

**扫描日期**: 2026-02-10
**扫描范围**: `cis-core/src` 全部 .rs 文件 (255 个文件)
**扫描内容**: TODO/FIXME/XXX/HACK, 简化实现, Mock/Placeholder, 硬编码值, 未完成特性

---

## 📊 执行摘要

| 优先级 | 数量 | 状态 |
|--------|------|------|
| 🔴 高优先级 | 12 | 影响核心功能 |
| 🟡 中优先级 | 28 | 影响完整性 |
| 🟢 低优先级 | 45+ | 可延后 |

---

## 🔴 高优先级 (核心功能影响)

### 1. WASM 执行未完全实现
**文件**: `service/skill_executor_impl.rs:192-196`, `scheduler/skill_executor.rs:258`
```rust
// service/skill_executor_impl.rs
self.add_log(execution_id, "WASM execution not fully implemented").await;
Err(CisError::skill("WASM execution not yet fully implemented. Please use native skill type for now."))
```
**问题**: WASM Skill 执行核心功能缺失，仅返回错误
**影响**: 无法执行 WASM 技能

### 2. Matrix Bridge Native Skill 执行模拟
**文件**: `matrix/bridge.rs:700-711`
```rust
// 实际实现需要通过 SkillRegistry 获取 Skill 实例并调用
// 这里简化为返回执行信息
Ok(serde_json::json!({
    "skill": skill_name,
    "event": event,
    "status": "executed",
    "note": "Native skill execution simulated - actual implementation needs skill instance registry"
}))
```
**问题**: Native Skill 实际并未执行，只是返回模拟数据
**影响**: Matrix 指令无法真正执行 Skill

### 3. Remote Skill 执行未实现
**文件**: `matrix/bridge.rs:676`, `service/skill_executor_impl.rs:217`, `scheduler/skill_executor.rs:171`
```rust
Err(CisError::skill("Remote skills not yet supported"))
Err(CisError::skill("Remote skill execution not yet implemented"))
```
**问题**: 远程 Skill 调用功能完全缺失
**影响**: 分布式场景无法使用

### 4. DAG Skill 执行未实现
**文件**: `matrix/bridge.rs:679`, `scheduler/skill_executor.rs:175`
```rust
Err(CisError::skill("DAG skills not yet supported"))
Err(CisError::skill("DAG skill execution not yet implemented - use execute_dag_skill"))
```
**问题**: DAG 类型 Skill 无法通过标准接口执行
**影响**: DAG 工作流集成受限

### 5. Kademlia DHT 路由表未实现
**文件**: `p2p/kademlia/mod.rs:102-143`
```rust
pub struct KademliaDht {
    local_id: NodeId,
    config: KademliaConfig,
    storage: Arc<RwLock<HashMap<String, Vec<u8>>>>,  // 仅本地存储
}
// start() 方法:
// TODO: 启动路由表维护任务
```
**问题**: 完整的 Kademlia 路由表未实现，只有本地存储
**影响**: DHT 功能不完整，仅支持本地操作

### 6. DHT 操作简化实现
**文件**: `p2p/dht_ops.rs:66-155`
```rust
// 向最近的节点存储（简化实现，实际应使用 Kademlia 路由表）
// 简化实现：广播查询请求
// 简化返回，实际应该等待响应
// 简化实现：使用字符串长度的差值（实际应该使用节点 ID 的字节 XOR）
```
**问题**: 所有 DHT 操作都是简化/模拟实现
**影响**: 分布式存储不可靠

### 7. Federation 心跳 TODO
**文件**: `agent/federation/agent.rs:273`
```rust
// TODO: 通过 FederationClient 发送心跳
```
**问题**: 联邦 Agent 心跳机制未实现
**影响**: 节点健康检测缺失

### 8. Federation 事件订阅未实现
**文件**: `agent/federation/agent.rs:295`, `agent/federation/agent.rs:322`
```rust
// TODO: 订阅 Matrix Room 事件
// TODO: 处理远程任务请求
```
**问题**: 联邦事件订阅和处理缺失
**影响**: 联邦协作功能不完整

### 9. Agent 直接 Skill 调用未实现
**文件**: `agent/bridge.rs:222-226`
```rust
// 当前实现：直接返回错误，提示使用 HTTP API
Err(CisError::skill(format!(
    "Direct skill call not implemented. Use CIS HTTP API: POST /api/v1/skills/{}/{}",
    skill, method
)))
```
**问题**: Agent 无法直接调用 Skill
**影响**: Agent 集成能力受限

### 10. P2P 公共内存同步未完成
**文件**: `p2p/network.rs:223`
```rust
Err(CisError::p2p("P2P public memory sync not fully implemented".to_string()))
```
**问题**: P2P 公共记忆同步功能缺失
**影响**: 分布式记忆功能不可用

### 11. Windows PID 管理不支持
**文件**: `system/pid_manager.rs:158`
```rust
anyhow::bail!("Windows not yet supported")
```
**问题**: Windows 平台进程管理未实现
**影响**: Windows 支持受限

---

## 🟡 中优先级 (完整性影响)

### 12. P2P 主题订阅简化
**文件**: `p2p/network.rs:514-520`
```rust
/// 订阅主题（简化实现）
pub async fn subscribe(&self, topic: &str) -> Result<()> {
    Err(CisError::p2p("Topic subscription not fully implemented".to_string()))
}
```

### 13. Matrix Federation 发现简化
**文件**: `matrix/federation/federation_discovery.rs:242-247`
```rust
/// 注意：当前为简化实现，直接返回空列表
debug!("SRV lookup for {} (simplified)", name);
// 简化实现：直接返回空列表
Ok(Vec::new())
```

### 14. Matrix Federation 签名占位符
**文件**: `matrix/federation/federation_discovery.rs:572`
```rust
signature: "valid_signature_placeholder".to_string(),
```

### 15. Federation Server 签名简化
**文件**: `matrix/federation/server.rs:341-345`
```rust
// In a real implementation, we would sign this response
// For now, we return an empty signature (simplified scheme B)
signatures.insert(state.config.server_name.clone(), HashMap::new());
```

### 16. 网络发现简化
**文件**: `network/simple_discovery.rs:1`
```rust
//! # 简化的节点发现服务
```

### 17. Matrix Store 简化认证
**文件**: `matrix/store.rs:9,180`
```rust
//! - `matrix_users`: Local user accounts (simplified auth)
// Local users table (simplified auth for Phase 0)
```

### 18. Matrix Sync 简化实现
**文件**: `matrix/routes/sync.rs:7,175,224`
```rust
//! This is a simplified sync that:
/// Phase 1: Simplified - returns joined rooms with messages.
// Build state events (simplified - just room member events)
```

### 19. Matrix 锚点简化版
**文件**: `matrix/anchor.rs:3,19`
```rust
//! 简化版：支持手动配置和可选的云端发现
/// 手动配置的 peers（简化版主要用这个）
```

### 20. WebSocket Noise 协议占位符
**文件**: `matrix/websocket/server.rs:8`
```rust
//! - Noise protocol handshake (placeholder)
```

### 21. WebSocket 客户端未完全实现
**文件**: `matrix/websocket/client.rs:804-806`
```rust
/// Not implemented
#[error("Not implemented: {0}")]
NotImplemented(String),
```

### 22. NAT 打洞简化实现
**文件**: `p2p/nat.rs:731-733`
```rust
// 简化实现：向 relay 发送请求，然后打洞
info!("Relayed hole punch not fully implemented, falling back to direct");
```

### 23. P2P 同步简化
**文件**: `p2p/sync.rs:331`
```rust
// 简化实现：返回空列表
Ok(Vec::new())
```

### 24. Agent 持久化数据 TODO
**文件**: `agent/persistent/claude.rs:577-579`, `agent/persistent/opencode.rs:569`
```rust
last_active_at: s.created_at, // TODO: 从 session 获取最后活动时间
total_tasks: 0, // TODO: 从持久化存储获取
work_dir: std::env::temp_dir().into(), // TODO: 从 session 获取
// TODO: 实现进程扫描或端口检测
```

### 25. 持久化 Agent 进程检测 TODO
**文件**: `agent/process_detector.rs:124-125`
```rust
last_active_at: proc.start_time, // TODO: 从 session 文件获取
total_tasks: 0, // TODO: 从持久化存储获取
```

### 26. 调度器多 Agent 执行器简化
**文件**: `scheduler/multi_agent_executor.rs:188`
```rust
/// 创建新的执行器（简化版，使用默认调度器）
```

### 27. DAG 执行器简化
**文件**: `scheduler/dag_executor.rs:60`
```rust
// 执行节点（简化版：顺序执行）
```

### 28. mDNS 发现任务 TODO
**文件**: `p2p/network.rs:527`
```rust
// TODO: 启动 mDNS 发现任务
```

### 29. P2P 优先级和超时逻辑 TODO
**文件**: `p2p/network.rs:611,625`
```rust
// TODO: 在 future 版本中实现优先级、超时和重试逻辑
// TODO: 在 future 版本中实现优先级和超时逻辑
```

### 30. WebSocket UDP 传输 TODO
**文件**: `matrix/websocket/client.rs:347`
```rust
// TODO: 在 future 版本中直接使用 UDP 传输层
```

### 31. 网络 ACL 简化处理
**文件**: `network/acl_tests.rs:560`
```rust
// Second add should be ignored
```

### 32. Matrix Bridge 事件获取简化
**文件**: `matrix/bridge.rs:593`
```rust
// 注意：由于存储层没有提供 get_event 方法，我们构造一个简化的事件
```

### 33. 决策倒计时简化
**文件**: `decision/countdown.rs:180`
```rust
// 简化的实现，实际应监听键盘输入
```

### 34. 内存服务未使用函数
**文件**: `memory/service.rs:636-715` (多处)
```rust
#[allow(dead_code)]
async fn get_private(&self, key: &str) -> Result<Option<MemoryItem>>;
#[allow(dead_code)]
async fn delete_private(&self, key: &str) -> Result<bool>;
#[allow(dead_code)]
async fn get_public(&self, key: &str) -> Result<Option<MemoryItem>>;
#[allow(dead_code)]
async fn delete_public(&self, key: &str) -> Result<bool>;
```

### 35. 任务向量搜索简化
**文件**: `task/vector.rs:191,194,319`
```rust
// 获取任务基本信息（这里简化处理，实际应该从 TaskStorage 获取）
title: task_id.clone(), // 简化：使用 ID 作为标题占位
title: id, // 简化：使用 ID 作为标题占位
```

### 36. 联邦客户端简化处理
**文件**: `agent/federation_client.rs:84,138,180`
```rust
// 这里简化处理
```

---

## 🟢 低优先级 (可延后)

### 37. 大量 Mock 实现 (仅测试使用)
**文件**: `test/mocks/*.rs`, 多处测试文件
- `test/mocks/network_service.rs`: Mock 网络服务
- `test/mocks/storage_service.rs`: Mock 存储服务
- `test/mocks/event_bus.rs`: Mock 事件总线
- `test/mocks/ai_provider.rs`: Mock AI Provider
- `test/mocks/embedding_service.rs`: Mock 嵌入服务
- `test/mocks/skill_executor.rs`: Mock Skill 执行器

### 38. Event Bus 简化实现
**文件**: `event_bus/memory.rs:12-14`
```rust
//! ## 简化实现说明（SHAME_TAG）
//! 根据 D03 设计文档的要求，以下功能在当前实现中被简化：
```

### 39. Conversation 上下文简化
**文件**: `conversation/context.rs:400,538`
```rust
// 注意：这里简化处理，实际可能需要从 conversation_db 获取完整信息
// 简化实现：提取用户问题的关键词组合
```

### 40. Matrix Nucleus 占位符
**文件**: `matrix/nucleus.rs:610`
```rust
let _nucleus_arc = Arc::new(RwLock::new(())); // Placeholder for self reference
```

### 41. 硬编码超时值
**文件**: 多处
```rust
// 30 秒超时
// 5 秒超时
// 默认 300 秒任务超时
```

### 42. 临时目录硬编码
**文件**: `config/loader.rs:530`, `storage/unified_paths.rs`, `storage/paths.rs`
```rust
temp_dir = "/tmp/cis"
```

### 43. 大量 #[allow(dead_code)]
**文件**: 20+ 个文件
- `network/websocket.rs`: node_id, config, ping_counter
- `matrix/routes/*.rs`: 多个字段
- `agent/federation/*.rs`: 多个字段
- `glm/mod.rs`: user_id

### 44. 测试忽略属性
**文件**: `ai/embedding_fastembed.rs:121`, `agent/persistent/*.rs`
```rust
#[ignore = "Downloads model on first run (~130MB)"]
#[ignore = "Requires claude to be installed"]
#[ignore = "Requires opencode to be installed"]
#[ignore = "Requires running OpenCode server"]
```

### 45. Container Mock 降级已移除标记
**文件**: `container.rs:102,167`
```rust
// SHAME_TAG NEW-3 REMOVED: Mock degradation eliminated in v1.1.5
```

---

## 🎯 重点关注分析

### bridge.rs WASM 集成状态: ⚠️ 部分完成

**文件**: `matrix/bridge.rs:717-783`

**已完成**:
- WASM Runtime 基础设施 (`wasm/runtime.rs`)
- WASM 模块验证和加载
- 内存管理和执行超时控制
- Host 函数绑定

**未完成**:
- `execute_native_skill()` (行 684-715): 仅返回模拟数据
- `execute_wasm_skill()` (行 717-783): 条件编译，依赖 `feature = "wasm"`
- Skill 实例注册表访问

**结论**: WASM 集成基础架构存在，但 Bridge 到实际 Skill 执行的路径未完全打通。

---

### Kademlia Transport 状态: ⚠️ 框架存在，功能不完整

**文件**: `p2p/kademlia/`

**已完成**:
- 传输接口定义 (`transport.rs:16-36`)
- P2PNetworkTransport 基本实现 (`transport.rs:38-142`)
- MockTransport 用于测试 (`transport.rs:144-222`)
- Kademlia 消息格式定义 (`message.rs`)
- 节点 ID 和距离计算 (`node_id.rs`, `distance.rs`)

**未完成**:
- 完整的路由表实现 (`routing_table.rs` 重复导入，实际未使用)
- Kademlia 查询算法完整实现 (`query.rs` 存在但未集成)
- DHT 的 `start()` 仅打印日志，未启动路由维护 (`mod.rs:113`)
- 缺少实际的节点发现和加入网络逻辑

**结论**: Kademlia 代码结构完整，但核心路由和查询功能未真正启用。

---

### 联邦协议状态: ✅ 基本实现完成，部分简化

**文件**: `matrix/federation/`

**已完成**:
- Federation Server (`server.rs`): 完整 HTTP 服务
- Federation Client (`client.rs`): HTTP 客户端，支持 mTLS
- 事件接收和发送 (`/_cis/v1/event/receive`)
- 服务器密钥端点 (`/_matrix/key/v2/server`)
- 事件签名验证 (`verify_event_signature`)
- 事件去重和持久化
- Peer 发现和管理 (`discovery.rs`)

**简化/占位**:
- 服务器响应签名返回空 (`server.rs:341-345`)
- SRV 发现返回空列表 (`federation_discovery.rs:242-247`)
- 握手挑战签名使用占位符 (`federation_discovery.rs:572`)

**结论**: 联邦协议核心功能可用，但签名和发现机制有简化处理。

---

## 📋 建议行动计划

### 短期 (1-2 周)
1. **完成 Matrix Bridge Native Skill 执行**: 实现 SkillRegistry 访问和实际执行
2. **修复 WASM Skill 执行路径**: 打通 Bridge -> WasmRuntime -> Skill 的完整链路
3. **完成 Federation 心跳和事件订阅**: 实现 `agent/federation/agent.rs` 中的 TODO

### 中期 (1 个月)
4. **完成 Kademlia DHT 路由表**: 实现完整的路由维护和查询机制
5. **完成 DHT 操作实现**: 替换 `dht_ops.rs` 中的简化实现
6. **实现 Remote Skill 调用**: 完成分布式 Skill 执行能力

### 长期 (2-3 个月)
7. **移除所有 Mock 降级**: 确保生产代码不依赖 Mock
8. **完成 Matrix Federation 签名**: 实现完整的密钥签名流程
9. **Windows 支持**: 完成 PID 管理和进程检测

---

## 📁 相关文件清单

### 高优先级文件
- `service/skill_executor_impl.rs`
- `scheduler/skill_executor.rs`
- `matrix/bridge.rs`
- `p2p/kademlia/mod.rs`
- `p2p/dht_ops.rs`
- `agent/federation/agent.rs`
- `agent/bridge.rs`
- `p2p/network.rs`
- `system/pid_manager.rs`

### 中优先级文件
- `matrix/federation/federation_discovery.rs`
- `matrix/federation/server.rs`
- `matrix/routes/sync.rs`
- `matrix/anchor.rs`
- `matrix/websocket/server.rs`
- `matrix/websocket/client.rs`
- `p2p/nat.rs`
- `p2p/sync.rs`
- `agent/persistent/*.rs`
- `scheduler/multi_agent_executor.rs`
- `scheduler/dag_executor.rs`

### Mock/测试相关
- `test/mocks/*.rs`
- `container.rs`

---

*报告生成时间: 2026-02-10*
*扫描工具: 手动代码审查 + grep 扫描*
