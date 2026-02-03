# Matrix 联邦架构改进计划

**文档版本**: 1.0  
**创建日期**: 2026-02-03  
**优先级**: P0 > P1 > P2

---

## 当前状态概览

### 已实现组件
- ✅ FederationManager 基础结构
- ✅ WebSocket 服务器/客户端框架
- ✅ MatrixStore 存储层
- ✅ SyncQueue 同步队列
- ✅ DID 身份系统基础
- ✅ Noise 协议加密通道

### 关键缺失（基于代码审查）
- ❌ `FederationManager::connect_websocket()` 完整实现
- ❌ WebSocket DID 验证逻辑
- ❌ MatrixBridge Skill 调用集成
- ❌ 联邦广播机制
- ❌ WebSocket 请求/响应模式
- ❌ mDNS 服务发现

---

## P0 - 核心功能缺失（ blocker）

### Task P0-1: 实现 FederationManager::connect_websocket()

**文件**: `cis-core/src/matrix/federation_impl.rs`  
**优先级**: 🔴 最高  
**预估工时**: 8h

#### 当前状态
```rust
// Line 380-400 附近
async fn connect_websocket(&self, node_id: &str) -> Result<Arc<Tunnel>> {
    // TODO: 实现完整的 WebSocket 连接流程
}
```

#### 实现要求
1. **完整的连接流程**:
   ```rust
   pub async fn connect_websocket(&self, node_id: &str) -> Result<Arc<Tunnel>> {
       // 1. 获取节点信息
       let peer = self.discovery.find_peer(node_id).await?;
       
       // 2. 创建 WebSocket 连接
       let ws_client = WebSocketClient::new(&peer.endpoint).await?;
       
       // 3. Noise XX 握手
       let noise = NoiseProtocol::new(self.local_keypair);
       let handshake = noise.xx_handshake_initiator().await?;
       ws_client.send(handshake).await?;
       
       // 4. DID 验证
       self.verify_remote_did(&ws_client, node_id).await?;
       
       // 5. 创建 Tunnel
       let tunnel = Tunnel::new(ws_client, noise).await?;
       
       // 6. 保存连接状态
       self.connections.insert(node_id, tunnel.clone()).await;
       
       Ok(tunnel)
   }
   ```

2. **错误处理和重连**:
   - 指数退避重连策略
   - 连接状态持久化
   - 连接失败原因追踪

#### 验收标准
- [ ] 能成功建立到其他节点的 WebSocket 连接
- [ ] 连接经过 Noise XX 握手加密
- [ ] 支持自动重连（最大重试 5 次）
- [ ] 单元测试覆盖率 > 80%

---

### Task P0-2: 实现 WebSocket DID 验证

**文件**: `cis-core/src/matrix/websocket/server.rs:501`  
**优先级**: 🔴 最高  
**预估工时**: 6h

#### 当前状态
```rust
// TODO: Implement actual DID verification
async fn verify_did(&self, token: &str) -> Result<VerifiedIdentity> {
    // 当前是占位实现
    Ok(VerifiedIdentity::anonymous())
}
```

#### 实现要求
1. **DID 验证流程**:
   ```rust
   pub async fn verify_did(&self, token: &str) -> Result<VerifiedIdentity> {
       // 1. 解析 DID token
       let did_claims = DIDToken::parse(token)?;
       
       // 2. 验证签名
       let public_key = self.did_resolver.resolve(&did_claims.issuer).await?;
       did_claims.verify_signature(&public_key)?;
       
       // 3. 验证有效期
       if did_claims.is_expired() {
           return Err(CisError::auth("DID token expired"));
       }
       
       // 4. 验证 challenge（防止重放）
       self.verify_challenge(&did_claims.challenge).await?;
       
       // 5. 记录验证结果
       self.auth_log.record(&did_claims).await?;
       
       Ok(VerifiedIdentity::from(did_claims))
   }
   ```

2. **挑战-响应机制**:
   - 服务器生成随机 challenge
   - 客户端使用 DID 私钥签名
   - 服务器验证签名

#### 验收标准
- [ ] DID token 能被正确解析和验证
- [ ] 过期 token 被拒绝
- [ ] 重放攻击被阻止（challenge 机制）
- [ ] 与现有的 DIDManager 集成

---

### Task P0-3: 完善 MatrixBridge Skill 调用和联邦广播

**文件**: 
- `cis-core/src/matrix/bridge.rs:344` (Skill 调用)
- `cis-core/src/matrix/bridge.rs:478` (联邦广播)

**优先级**: 🔴 最高  
**预估工时**: 10h

#### 当前状态
```rust
// TODO: 实际调用 Skill 的处理逻辑
async fn execute_skill(&self, task: SkillTask) -> SkillResult {
    // 当前是占位实现
    SkillResult::success()
}

// TODO: 实现实际的联邦广播逻辑
async fn broadcast_to_federation(&self, event: CisMatrixEvent) {
    // 当前为空实现
}
```

#### 实现要求
1. **Skill 调用集成**:
   ```rust
   pub async fn execute_skill(&self, task: SkillTask) -> SkillResult {
       let start = Instant::now();
       
       // 1. 加载 Skill
       let skill = self.skill_manager.load(&task.skill).await?;
       
       // 2. 准备上下文
       let ctx = SkillContext::new()
           .with_params(task.params)
           .with_room_id(task.room_id);
       
       // 3. 执行 Skill
       let result = match skill.execute(&task.action, ctx).await {
           Ok(output) => {
               // 4. 保存结果到记忆
               self.save_skill_result(&task, &output).await?;
               
               SkillResult {
                   success: true,
                   data: Some(output),
                   error: None,
                   elapsed_ms: start.elapsed().as_millis() as u64,
               }
           }
           Err(e) => SkillResult {
               success: false,
               data: None,
               error: Some(e.to_string()),
               elapsed_ms: start.elapsed().as_millis() as u64,
           }
       };
       
       // 5. 联邦广播（如果房间启用了联邦）
       if self.is_federated_room(&task.room_id).await? {
           self.broadcast_skill_result(&task, &result).await?;
       }
       
       Ok(result)
   }
   ```

2. **联邦广播机制**:
   ```rust
   pub async fn broadcast_to_federation(&self, event: CisMatrixEvent) {
       // 1. 获取联邦中的节点
       let peers = self.federation_manager.get_ready_peers().await;
       
       // 2. 序列化事件
       let payload = serde_json::to_vec(&event).unwrap();
       
       // 3. 并行广播
       let futures = peers.iter().map(|peer| {
           let payload = payload.clone();
           async move {
               if let Some(tunnel) = peer.tunnel().await {
                   if let Err(e) = tunnel.send(payload).await {
                       warn!("Failed to broadcast to {}: {}", peer.node_id(), e);
                   }
               }
           }
       });
       
       futures::future::join_all(futures).await;
   }
   ```

#### 验收标准
- [ ] Matrix 消息能触发 Skill 调用
- [ ] Skill 执行结果保存到记忆
- [ ] 联邦广播能到达所有在线节点
- [ ] 支持 room 级别的联邦开关控制
- [ ] 端到端测试通过

---

## P1 - 优化增强

### Task P1-1: WebSocket 同步请求/响应处理

**文件**: `cis-core/src/matrix/sync/consumer.rs:227`  
**优先级**: 🟡 高  
**预估工时**: 6h

#### 实现要求
1. **请求-响应模式**:
   ```rust
   pub struct SyncRequest {
       pub request_id: String,
       pub since: Option<String>,
       pub timeout_ms: u64,
   }
   
   pub struct SyncResponse {
       pub request_id: String,
       pub events: Vec<CisMatrixEvent>,
       pub next_batch: String,
   }
   
   impl SyncConsumer {
       pub async fn request_sync(&self, peer: &PeerInfo, req: SyncRequest) 
           -> Result<SyncResponse> {
           let (tx, rx) = oneshot::channel();
           
           // 注册等待响应
           self.pending_requests.insert(req.request_id.clone(), tx).await;
           
           // 发送请求
           peer.send(req).await?;
           
           // 等待响应（带超时）
           match timeout(Duration::from_millis(req.timeout_ms), rx).await {
               Ok(Ok(response)) => Ok(response),
               Ok(Err(_)) => Err(CisError::sync("Response channel closed")),
               Err(_) => Err(CisError::sync("Sync request timeout")),
           }
       }
   }
   ```

#### 验收标准
- [ ] 支持同步请求超时
- [ ] 支持批量事件返回
- [ ] 支持断点续传 (since token)

---

### Task P1-2: 联邦存储集成

**文件**: `cis-core/src/matrix/federation/server.rs:479`  
**优先级**: 🟡 高  
**预估工时**: 8h

#### 实现要求
1. **联邦事件存储**:
   ```rust
   impl MatrixStore {
       // 保存联邦事件
       pub async fn save_federated_event(&self, event: &CisMatrixEvent) -> Result<()> {
           let db = self.federation_db();
           
           db.execute(
               "INSERT INTO federation_events 
                (event_id, sender, room_id, event_type, content, origin_server_ts, signatures)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(event_id) DO UPDATE SET
                signatures = excluded.signatures",
               params![
                   event.event_id,
                   event.sender,
                   event.room_id,
                   event.event_type,
                   event.content,
                   event.origin_server_ts,
                   event.signatures
               ],
           )?;
           
           Ok(())
       }
       
       // 查询联邦事件
       pub async fn query_federated_events(
           &self,
           room_id: &str,
           since: Option<String>,
           limit: usize,
       ) -> Result<Vec<CisMatrixEvent>> {
           // 实现查询逻辑
       }
   }
   ```

2. **签名验证**:
   - 验证事件签名
   - 验证发送者 DID
   - 处理签名冲突

#### 验收标准
- [ ] 联邦事件持久化到独立数据库
- [ ] 支持按 room 和时间范围查询
- [ ] 签名验证通过

---

### Task P1-3: 事件类型映射优化

**文件**: `cis-core/src/matrix/nucleus.rs:1110`  
**优先级**: 🟡 中  
**预估工时**: 4h

#### 实现要求
```rust
pub fn map_event_type(content: &MessageContent) -> CISMessageType {
    match content {
        // Skill 调用
        MessageContent::Text { body, .. } if body.starts_with("!skill ") => {
            CISMessageType::SkillCommand
        }
        
        // 任务管理
        MessageContent::Text { body, .. } if body.starts_with("!task ") => {
            CISMessageType::TaskCommand
        }
        
        // 记忆查询
        MessageContent::Text { body, .. } if body.starts_with("!memory ") => {
            CISMessageType::MemoryQuery
        }
        
        // 文件分享
        MessageContent::File { .. } => CISMessageType::FileShare,
        
        // 默认：普通消息
        _ => CISMessageType::PlainText,
    }
}
```

#### 验收标准
- [ ] 支持 !skill / !task / !memory 命令识别
- [ ] 支持文件类型自动分类
- [ ] 支持自定义消息类型扩展

---

### Task P1-4: Room 状态自动同步

**优先级**: 🟡 中  
**预估工时**: 6h

#### 实现要求
```rust
impl FederationManager {
    /// 启动 room 状态同步任务
    pub async fn start_room_sync_task(&self) {
        let manager = self.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // 1. 获取所有联邦房间
                let rooms = manager.get_federated_rooms().await;
                
                // 2. 对每个房间进行增量同步
                for room in rooms {
                    if let Err(e) = manager.sync_room_state(&room).await {
                        error!("Failed to sync room {}: {}", room.id(), e);
                    }
                }
            }
        });
    }
    
    /// 同步单个房间状态
    async fn sync_room_state(&self, room: &RoomInfo) -> Result<()> {
        let since = room.last_sync_token().await?;
        
        // 向所有联邦节点请求增量更新
        for peer in self.get_ready_peers().await {
            let request = SyncRequest {
                room_id: room.id().to_string(),
                since: since.clone(),
                limit: 100,
            };
            
            match peer.request_sync(request).await {
                Ok(response) => {
                    self.apply_sync_response(room, response).await?;
                }
                Err(e) => {
                    warn!("Sync failed with {}: {}", peer.node_id(), e);
                }
            }
        }
        
        Ok(())
    }
}
```

---

## P2 - 功能增强

### Task P2-1: mDNS 服务发现

**文件**: `cis-core/src/matrix/federation/discovery.rs:187`  
**优先级**: 🟢 低  
**预估工时**: 6h

#### 当前状态
```rust
// TODO: Implement actual mDNS discovery
pub async fn discover_local_peers(&self) -> Vec<PeerInfo> {
    vec![] // 空实现
}
```

#### 实现要求
```rust
pub async fn discover_local_peers(&self) -> Vec<PeerInfo> {
    let mdns = ServiceDaemon::new()?;
    let service_type = "_cis-matrix._tcp.local.";
    
    let mut peers = vec![];
    let receiver = mdns.browse(service_type)?;
    
    while let Ok(event) = receiver.recv_timeout(Duration::from_secs(5)) {
        if let ServiceEvent::ServiceResolved(info) = event {
            if let Some(peer) = self.parse_mdns_info(&info) {
                peers.push(peer);
            }
        }
    }
    
    peers
}
```

---

### Task P2-2: UDP Hole Punching

**文件**: `cis-core/src/matrix/websocket/client.rs:200`  
**优先级**: 🟢 低  
**预估工时**: 10h

#### 当前状态
```rust
// TODO: Implement UDP hole punching
pub async fn try_hole_punching(&self, peer: &PeerInfo) -> Result<Connection> {
    // 空实现
}
```

#### 实现要求
1. **STUN 发现公网地址**（已部分实现）
2. **协调打孔时机**
3. **备用 TURN 中继**（P2 后续）

---

### Task P2-3: Cloud Anchor 云端服务

**优先级**: 🟢 低  
**预估工时**: 12h

#### 实现要求
```rust
pub struct CloudAnchor {
    endpoint: String,
    auth_token: String,
}

impl CloudAnchor {
    /// 注册本节点
    pub async fn register(&self, node_info: &NodeInfo) -> Result<()> {
        let client = reqwest::Client::new();
        client.post(&format!("{}/register", self.endpoint))
            .bearer_auth(&self.auth_token)
            .json(node_info)
            .send()
            .await?;
        Ok(())
    }
    
    /// 查询在线节点
    pub async fn query_peers(&self) -> Result<Vec<NodeInfo>> {
        let client = reqwest::Client::new();
        let response = client.get(&format!("{}/peers", self.endpoint))
            .bearer_auth(&self.auth_token)
            .send()
            .await?;
        
        Ok(response.json().await?)
    }
    
    /// 心跳保活
    pub async fn heartbeat(&self) -> Result<()> {
        // 定期发送心跳
    }
}
```

---

## 实施路线图

### Phase 1: P0 核心功能（Week 1-2）

| Week | Task | 负责人 | 状态 |
|------|------|--------|------|
| W1D1-2 | P0-1: connect_websocket | TBD | ⏳ |
| W1D3-4 | P0-2: DID 验证 | TBD | ⏳ |
| W1D5-W2D2 | P0-3: Bridge Skill 调用 | TBD | ⏳ |
| W2D3-5 | 集成测试 & Bugfix | TBD | ⏳ |

### Phase 2: P1 优化增强（Week 3-4）

| Week | Task | 优先级 | 状态 |
|------|------|--------|------|
| W3 | P1-1: 请求/响应模式 | 🟡 高 | ⏳ |
| W3 | P1-2: 联邦存储集成 | 🟡 高 | ⏳ |
| W4 | P1-3: 事件映射优化 | 🟡 中 | ⏳ |
| W4 | P1-4: Room 状态同步 | 🟡 中 | ⏳ |

### Phase 3: P2 功能增强（Week 5+）

| Task | 优先级 | 依赖 | 状态 |
|------|--------|------|------|
| P2-1: mDNS | 🟢 低 | 无 | ⏳ |
| P2-2: UDP 打孔 | 🟢 低 | P0-1 | ⏳ |
| P2-3: Cloud Anchor | 🟢 低 | P0-1 | ⏳ |

---

## 关键依赖

### 外部依赖
- `mdns-sd`: mDNS 服务发现（已引入）
- `igd`: UPnP/NAT 穿透（已引入）
- `stun`: STUN 客户端（已引入）

### 内部依赖
- DIDManager: 身份验证
- SkillManager: Skill 调用
- MatrixStore: 联邦存储

---

## 测试策略

### 单元测试
- 每个 Task 需要配套单元测试
- 覆盖率目标: > 80%

### 集成测试
```rust
#[tokio::test]
async fn test_federation_end_to_end() {
    // 1. 启动两个节点
    let node_a = spawn_test_node("node_a").await;
    let node_b = spawn_test_node("node_b").await;
    
    // 2. 建立联邦连接
    node_a.connect(&node_b.did()).await.unwrap();
    
    // 3. 发送测试事件
    let event = create_test_event();
    node_a.broadcast(event.clone()).await.unwrap();
    
    // 4. 验证接收
    let received = node_b.recv_event_timeout(5s).await.unwrap();
    assert_eq!(received.event_id, event.event_id);
}
```

### 性能测试
- 联邦广播延迟 < 100ms（局域网）
- 同步吞吐量 > 1000 events/sec
- 并发连接数 > 50

---

## 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| DID 验证复杂度高 | 中 | 高 | 分阶段实现，先用简单方案 |
| WebSocket 稳定性 | 中 | 高 | 增加重连和降级机制 |
| 存储性能瓶颈 | 低 | 中 | 使用 WAL 模式，批量写入 |

---

## 附录

### 代码位置速查

| 组件 | 文件路径 |
|------|----------|
| FederationManager | `src/matrix/federation_impl.rs` |
| MatrixBridge | `src/matrix/bridge.rs` |
| WebSocket Server | `src/matrix/websocket/server.rs` |
| WebSocket Client | `src/matrix/websocket/client.rs` |
| DID 验证 | `src/identity/did.rs` |
| Sync Consumer | `src/matrix/sync/consumer.rs` |

### 相关文档
- [MATRIX_IMPLEMENTATION.md](./MATRIX_IMPLEMENTATION.md)
- [NETWORKING.md](./NETWORKING.md)
- [P2P_STATUS_ANALYSIS.md](./P2P_STATUS_ANALYSIS.md)

---

**计划批准后将拆分为具体的 GitHub Issues 进行跟踪。**
