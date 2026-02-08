# CIS 项目执行计划表

> 基于 kimi_agent.md 评估报告
> 当前状态: 75% | 目标: v1.1.0 (2026 Q2)

---

## 立即执行 (本周开始)

### Phase 1: 稳定性修复 [P0]

| # | 任务 | 文件 | 负责人 | 预计时间 | 状态 |
|---|------|------|--------|----------|------|
| 1 | 复现 SIGBUS 错误 | - | - | 1d | ⬜ |
| 2 | 修复 `memory::service::tests::test_memory_service_delete` | `memory/service.rs` | - | 2d | ⬜ |
| 3 | 修复 `matrix::websocket::server` 测试 | `matrix/websocket/server.rs` | - | 2d | ⬜ |
| 4 | 修复 `storage::db::tests::test_core_db` | `storage/db.rs` | - | 2d | ⬜ |
| 5 | 提升核心模块测试覆盖率 | 多文件 | - | 3d | ⬜ |

**验收**: `cargo test --lib` 全部通过

---

### Phase 2: GUI 生产化 [P0]

| # | 任务 | 文件 | 预计时间 | 状态 |
|---|------|------|----------|------|
| 1 | 创建 NodeStore 连接 node.db | `cis-gui/src/store.rs` | 2d | ⬜ |
| 2 | 创建 MemoryStore 连接 memory.db | `cis-gui/src/memory_store.rs` | 2d | ⬜ |
| 3 | 创建 MatrixStore 连接 matrix-social.db | `cis-gui/src/matrix_store.rs` | 2d | ⬜ |
| 4 | 替换 app.rs 硬编码节点数据 | `cis-gui/src/app.rs` | 1d | ⬜ |
| 5 | 替换 glm_panel.rs 演示数据 | `cis-gui/src/glm_panel.rs` | 1d | ⬜ |
| 6 | WebSocket 客户端实时更新 | `cis-gui/src/ws_client.rs` | 3d | ⬜ |
| 7 | 记忆搜索界面 | - | 3d | ⬜ |

**验收**: GUI 显示真实数据，支持实时消息

---

### Phase 3: WASM Skill 执行 [P1]

| # | 任务 | 文件 | 预计时间 | 状态 |
|---|------|------|----------|------|
| 1 | 实现 `skill::manager::execute_wasm_skill` | `skill/manager.rs` | 2d | ⬜ |
| 2 | 创建 WASM Runtime | `wasm/runtime.rs` | 3d | ⬜ |
| 3 | 实现 Host Function (log, storage, http) | `wasm/host.rs` | 3d | ⬜ |
| 4 | 完善 `#[skill]` 宏 | `cis-skill-sdk-derive` | 2d | ⬜ |
| 5 | 验证 init-wizard 编译执行 | `skills/init-wizard` | 2d | ⬜ |
| 6 | 验证 push-client 编译执行 | `skills/push-client` | 2d | ⬜ |

**验收**: WASM Skill 可加载执行，Host API 可用

---

### Phase 4: IM 集成 [P1]

| # | 任务 | 文件 | 预计时间 | 状态 |
|---|------|------|----------|------|
| 1 | 实现 MatrixAdapter | `skills/im/src/matrix_adapter.rs` | 3d | ⬜ |
| 2 | Matrix 事件同步到 IM DB | `skills/im/src/db.rs` | 2d | ⬜ |
| 3 | IM 消息同步到 Matrix | `skills/im/src/db.rs` | 2d | ⬜ |
| 4 | 消息状态同步（已读/未读） | - | 2d | ⬜ |
| 5 | 通知系统 | `skills/im/src/notification.rs` | 2d | ⬜ |

**验收**: Element ↔ CIS GUI 双向消息同步

---

### Phase 5: P2P 完善 [P2]

| # | 任务 | 文件 | 预计时间 | 状态 |
|---|------|------|----------|------|
| 1 | mDNS 局域网发现 | `p2p/discovery.rs` | 3d | ⬜ |
| 2 | 服务发现协议 | `p2p/discovery.rs` | 2d | ⬜ |
| 3 | QUIC 连接管理 | `p2p/quic.rs` | 3d | ⬜ |
| 4 | STUN/UPnP NAT 穿透 | `p2p/nat.rs` (新建) | 4d | ⬜ |
| 5 | Gossip 广播协议 | `p2p/gossip.rs` (新建) | 3d | ⬜ |
| 6 | 房间状态同步 | - | 3d | ⬜ |

**验收**: 局域网自动发现，跨节点数据同步

---

## 时间线

```
2月 (Week 5-8):
  ████████░░░░░░░░░░░░░░░░  Phase 1: 稳定性
  ████████░░░░░░░░░░░░░░░░  Phase 2: GUI (开始)

3月 (Week 9-12):
  ░░░░░░░░████████░░░░░░░░  Phase 2: GUI (完成)
  ░░░░░░░░████████░░░░░░░░  Phase 3: WASM

4月 (Week 13-16):
  ░░░░░░░░░░░░░░░░████████  Phase 4: IM
  ░░░░░░░░░░░░░░░░████████  Phase 5: P2P

4月底: v1.1.0 Release
```

---

## 快速检查清单

### 每日检查
- [ ] 新增测试是否通过
- [ ] 是否有新的编译警告
- [ ] 文档是否同步更新

### 每周检查
- [ ] 测试覆盖率变化
- [ ] 性能回归测试
- [ ] 代码审查完成

### 里程碑检查
- [ ] 所有阻塞问题已解决
- [ ] 验收标准已达成
- [ ] 文档已更新

---

## 资源需求

| 角色 | 人数 | 职责 |
|------|------|------|
| Rust 核心开发 | 2 | Phase 1, 3, 5 |
| GUI 开发 | 1 | Phase 2 |
| 网络开发 | 1 | Phase 4, 5 |
| 测试/QA | 1 | 全阶段 |

---

## 风险监控

| 风险 | 监控指标 | 应对 |
|------|----------|------|
| SIGBUS 难修复 | 修复天数 > 5d | 引入外部专家 |
| GUI 工作量大 | 完成度 < 50% (Week 7) | 削减非核心功能 |
| WASM 复杂 | 完成度 < 50% (Week 10) | 简化 MVP 范围 |

---

*执行计划版本: 1.0*
*创建日期: 2026-02-08*
*更新频率: 每周*
