# CIS 项目完善路线图

> 基于 [kimi_agent.md](issue/kimi_agent.md) 评估报告制定
> 当前完成度: **75%** | 目标: **v1.1.0 生产就绪**

---

## 📊 现状分析

### 完成度概览

```
核心引擎:   ██████████ 90%  ✅ DAG、存储、安全
网络层:     ██████░░░░ 60%  ⚠️  P2P 部分实现
界面层:     █████░░░░░ 50%  ⚠️  GUI 演示阶段
生态集成:   █████░░░░░ 50%  ⚠️  Skill 框架待完善
测试稳定:   ████░░░░░░ 40%  🔴 内存问题待修复
```

### 关键问题清单

| 优先级 | 问题 | 影响 | 文件位置 |
|--------|------|------|----------|
| 🔴 P0 | SIGBUS 内存错误 | 测试失败 | `memory::service`, `matrix::websocket` |
| 🔴 P0 | GUI 演示数据 | 无法生产使用 | `cis-gui/src/app.rs` |
| 🟡 P1 | WASM Skill todo!() | 生态无法扩展 | `skill/manager.rs` |
| 🟡 P1 | IM 集成占位符 | 协作功能缺失 | `skills/im/src/matrix_adapter.rs` |
| 🟢 P2 | P2P 网络不完整 | 组网受限 | `p2p/discovery.rs` |

---

## 🎯 里程碑规划

### Milestone 1: 稳定性修复 (2-3周)
**目标**: 修复关键测试失败，达到可测试状态

#### Week 1-2: 内存问题修复
- [ ] **SIGBUS 问题诊断**
  - [ ] 复现 `memory::service::tests::test_memory_service_delete` 失败
  - [ ] 复现 `matrix::websocket::server` 相关测试失败
  - [ ] 复现 `storage::db::tests::test_core_db` 失败
  - [ ] 使用 AddressSanitizer 定位内存错误

- [ ] **Matrix WebSocket Server 修复**
  ```rust
  // 文件: cis-core/src/matrix/websocket/server.rs
  // 问题: test_sync_request_handling, test_sync_response_handling 失败
  // 任务: 修复同步响应处理逻辑
  ```

- [ ] **Memory Service 修复**
  ```rust
  // 文件: cis-core/src/memory/service.rs
  // 问题: test_memory_service_delete 内存访问错误
  // 任务: 修复删除操作的内存安全问题
  ```

#### Week 2-3: 数据库测试修复
- [ ] **Storage DB 修复**
  ```rust
  // 文件: cis-core/src/storage/db.rs
  // 问题: test_core_db 失败
  // 任务: 修复数据库连接管理
  ```

- [ ] **测试覆盖率提升**
  - [ ] 核心模块测试覆盖率从 40% → 60%
  - [ ] 添加 `matrix::store_social` 更多边界测试
  - [ ] 添加 `federation` 集成测试

**交付标准**:
```bash
cargo test --lib  # 全部通过
# 无 SIGBUS/SIGSEGV 错误
# 核心模块覆盖率 >= 60%
```

---

### Milestone 2: GUI 生产化 (3-4周)
**目标**: GUI 连接真实数据库，支持实际工作流

#### Week 4-5: 数据层连接
- [ ] **NodeStore 实现**
  ```rust
  // 文件: cis-gui/src/store.rs (新建)
  // 功能: 连接 node.db 读取真实节点数据
  pub struct NodeStore {
      db: Arc<NodeDatabase>,
  }
  ```

- [ ] **MemoryStore 实现**
  ```rust
  // 文件: cis-gui/src/memory_store.rs (新建)
  // 功能: 连接 memory.db 读取记忆数据
  impl MemoryStore {
      pub fn load_conversations(&self) -> Vec<Conversation>;
      pub fn search_memories(&self, query: &str) -> Vec<Memory>;
  }
  ```

- [ ] **MatrixStore 集成**
  ```rust
  // 文件: cis-gui/src/matrix_store.rs (新建)
  // 功能: 连接 matrix-social.db 读取用户/房间数据
  impl MatrixStore {
      pub fn get_joined_rooms(&self, user_id: &str) -> Vec<Room>;
      pub fn get_room_messages(&self, room_id: &str) -> Vec<Message>;
  }
  ```

#### Week 5-6: UI 数据替换
- [ ] **替换演示数据**
  ```rust
  // 文件: cis-gui/src/app.rs
  // 修改: ManagedNode 从数据库加载而非硬编码
  impl App {
      fn load_managed_nodes(&mut self) {
          self.nodes = self.node_store.get_all_nodes();
      }
  }
  ```

- [ ] **GLM Panel 数据连接**
  ```rust
  // 文件: cis-gui/src/glm_panel.rs
  // 修改: pending_dags 从真实 DAG 管理器加载
  impl GlmPanel {
      fn refresh_dags(&mut self) {
          self.pending_dags = dag_manager.get_pending();
      }
  }
  ```

#### Week 6-7: 实时更新
- [ ] **WebSocket 集成**
  ```rust
  // 文件: cis-gui/src/ws_client.rs (新建)
  // 功能: 连接 Matrix WebSocket 接收实时消息
  impl WsClient {
      pub async fn connect(&mut self, url: &str);
      pub fn on_message<F: Fn(Message)>(&mut self, handler: F);
  }
  ```

- [ ] **记忆搜索界面**
  - [ ] 向量搜索 UI
  - [ ] 语义高亮显示
  - [ ] 时间线视图

**交付标准**:
- GUI 启动后显示真实节点数据
- 可以浏览真实记忆历史
- 可以看到 Matrix Room 的实时消息

---

### Milestone 3: WASM Skill 完整执行 (2-3周)
**目标**: WASM Skill 可以实际加载和执行

#### Week 8-9: Host API 完善
- [ ] **实现 todo!() 占位符**
  ```rust
  // 文件: cis-core/src/skill/manager.rs
  // 当前: fn execute_wasm_skill(&self, ...) { todo!() }
  // 实现: 完整的 WASM 执行流程
  ```

- [ ] **WASM Runtime 集成**
  ```rust
  // 文件: cis-core/src/wasm/runtime.rs
  // 功能: Wasmer 运行时配置
  impl WasmRuntime {
      pub fn load_module(&mut self, bytes: &[u8]) -> Result<Module>;
      pub fn execute(&self, module: &Module, input: Value) -> Result<Value>;
  }
  ```

- [ ] **Host Function 实现**
  ```rust
  // 文件: cis-core/src/wasm/host.rs
  // 实现: 暴露给 WASM 的宿主函数
  #[host_function]
  fn host_log(ctx: &mut Context, msg: String);
  
  #[host_function]
  fn host_storage_get(ctx: &mut Context, key: String) -> Option<String>;
  
  #[host_function]
  fn host_http_request(ctx: &mut Context, req: Request) -> Response;
  ```

#### Week 9-10: Skill SDK 完善
- [ ] **SDK 宏实现**
  ```rust
  // 文件: cis-skill-sdk/cis-skill-sdk-derive/src/lib.rs
  // 功能: #[skill] 宏生成 WASM 导出函数
  #[proc_macro_attribute]
  pub fn skill(args: TokenStream, input: TokenStream) -> TokenStream;
  ```

- [ ] **示例 Skill 验证**
  - [ ] `init-wizard` 编译为 WASM 并执行
  - [ ] `push-client` 编译为 WASM 并执行
  - [ ] 测试 Host Function 调用

**交付标准**:
```bash
# 可以加载 WASM Skill
cis skill run --wasm target/wasm32-unknown-unknown/release/init-wizard.wasm

# WASM Skill 可以调用 Host API
# 日志输出正常
```

---

### Milestone 4: IM 集成完善 (2-3周)
**目标**: Matrix Room 支持完整的 IM 功能

#### Week 11-12: IM Skill 完善
- [ ] **Matrix Adapter 实现**
  ```rust
  // 文件: skills/im/src/matrix_adapter.rs
  // 当前: 占位符实现
  // 实现: 真实的 Matrix Room 桥接
  impl MatrixAdapter {
      pub async fn sync_rooms(&self) -> Result<Vec<Room>>;
      pub async fn send_message(&self, room_id: &str, content: &str) -> Result<()>;
      pub async fn receive_events(&mut self) -> Result<Vec<Event>>;
  }
  ```

- [ ] **IM Database 集成**
  ```rust
  // 文件: skills/im/src/db.rs
  // 功能: 将 Matrix 事件同步到 IM 数据库
  impl ImDatabase {
      pub async fn sync_from_matrix(&self, events: Vec<MatrixEvent>) -> Result<()>;
      pub async fn get_unread(&self, user_id: &str) -> Vec<Message>;
  }
  ```

#### Week 12-13: 消息同步
- [ ] **双向同步**
  - Matrix Room → IM Database
  - IM Database → Matrix Room
  - 消息状态同步（已读/未读）

- [ ] **通知系统**
  ```rust
  // 文件: skills/im/src/notification.rs
  impl NotificationService {
      pub fn notify_new_message(&self, msg: &Message);
      pub fn notify_mention(&self, msg: &Message, user: &User);
  }
  ```

**交付标准**:
- 可以在 Element 客户端看到 CIS 发送的消息
- CIS GUI 可以看到 Element 发送的消息
- 消息状态正确同步

---

### Milestone 5: P2P 网络完善 (3-4周)
**目标**: 完整的去中心化组网

#### Week 14-15: mDNS 发现完善
- [ ] **局域网发现**
  ```rust
  // 文件: cis-core/src/p2p/discovery.rs
  impl DiscoveryService {
      pub async fn discover_local(&self) -> Vec<PeerInfo>;
      pub async fn announce(&self) -> Result<()>;
  }
  ```

- [ ] **服务发现**
  - 发现同网络中的 CIS 节点
  - 自动建立初始连接
  - 交换 DID 身份

#### Week 15-16: QUIC 传输完善
- [ ] **连接管理**
  ```rust
  // 文件: cis-core/src/p2p/quic.rs
  impl QuicTransport {
      pub async fn connect(&self, addr: SocketAddr) -> Result<Connection>;
      pub async fn accept(&self) -> Result<Connection>;
  }
  ```

- [ ] **NAT 穿透**
  - STUN 支持
  - UPnP 自动端口映射
  - 中继节点发现

#### Week 16-17: 组网协议
- [ ] **Gossip 协议**
  ```rust
  // 文件: cis-core/src/p2p/gossip.rs
  impl GossipProtocol {
      pub fn broadcast(&self, topic: &str, data: &[u8]);
      pub fn subscribe(&self, topic: &str) -> Receiver<Message>;
  }
  ```

- [ ] **数据同步**
  - 房间状态同步
  - 记忆片段同步
  - DAG 任务分发

**交付标准**:
- 同一局域网自动发现
- 可以跨网络建立连接
- 数据自动同步

---

## 📅 时间线

```
Month 1 (Week 1-4):
  ├── Week 1-2: 内存问题修复 [Milestone 1]
  └── Week 3-4: 数据库测试修复 + GUI 数据层 [Milestone 1-2]

Month 2 (Week 5-8):
  ├── Week 5-6: GUI UI 数据替换 [Milestone 2]
  └── Week 7-8: GUI 实时更新 + WASM Host API [Milestone 2-3]

Month 3 (Week 9-12):
  ├── Week 9-10: WASM 执行 + Skill SDK [Milestone 3]
  └── Week 11-12: IM Skill 完善 [Milestone 4]

Month 4 (Week 13-16):
  ├── Week 13-14: P2P 发现完善 [Milestone 5]
  └── Week 15-16: QUIC + 组网协议 [Milestone 5]

Month 4 末: v1.1.0 Release
```

---

## 🔧 实施策略

### 优先级策略

```
P0 (阻塞): SIGBUS, GUI 演示数据
P1 (重要): WASM 执行, IM 集成
P2 (增值): P2P 完善, 性能优化
```

### 并行开发

```
团队 A (核心稳定性):
  - 内存问题修复
  - 测试覆盖率提升

团队 B (GUI 产品化):
  - 数据层连接
  - UI 替换
  - 实时更新

团队 C (生态扩展):
  - WASM 执行
  - IM 集成
  - P2P 完善
```

### 代码冻结点

| 阶段 | 冻结内容 | 目标日期 |
|------|----------|----------|
| Feature Freeze | 新功能停止 | Week 14 |
| Code Freeze | 仅修复 Bug | Week 15 |
| Release Candidate | 准备发布 | Week 16 |

---

## 📈 验收标准

### v1.1.0 发布标准

```
✅ 测试:
   - cargo test --lib 100% 通过
   - 无内存安全错误
   - 核心模块覆盖率 >= 70%

✅ GUI:
   - 连接真实数据库
   - 支持完整工作流
   - 实时消息同步

✅ Skill:
   - WASM Skill 可加载执行
   - Host API 完整可用
   - 至少 3 个 WASM Skill 运行

✅ 网络:
   - 局域网自动发现
   - 跨节点数据同步
   - 房间消息联邦

✅ 文档:
   - API 文档完整
   - 部署指南
   - 用户手册
```

---

## 📝 待办清单

### 立即开始 (本周)

- [ ] 创建修复分支: `fix/stability-milestone-1`
- [ ] 复现 SIGBUS 错误并记录
- [ ] 设置 AddressSanitizer 环境
- [ ] 分配团队资源

### 依赖关系

```
Milestone 1 (稳定性)
  │
  ├─► Milestone 2 (GUI) ──► 需要稳定的数据库
  │
  ├─► Milestone 3 (WASM) ──► 需要稳定的存储
  │
  └─► Milestone 4 (IM) ──► 需要 GUI 和 WASM

Milestone 5 (P2P)
  │
  └─► 可以并行开发
```

---

## 💡 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| SIGBUS 修复困难 | 中 | 高 | 使用专业工具，预留缓冲时间 |
| GUI 重构工作量大 | 高 | 中 | 分阶段替换，保持兼容性 |
| WASM 复杂度超预期 | 中 | 中 | 简化 MVP，后续迭代 |
| P2P NAT 穿透 | 高 | 低 | 使用中继节点备选方案 |

---

## 📞 联系

- **项目负责人**: Jiang Xiaolong
- **技术负责人**: CIS Core Team
- **文档**: [CIS GitHub](https://github.com/opencode/CIS)

---

*最后更新: 2026-02-08*
