# CIS v1.1.3 剩余工作清单

> **状态**: Phase 1 & 2 已完成，Phase 3 清理中  
> **更新时间**: 2026-02-10  
> **目标**: 清除所有虚拟/模拟/TODO 代码

---

## 工作总览

| 级别 | 数量 | 状态 | 说明 |
|-----|------|------|------|
| 🔴 P0 | 3 | ⏳ | 核心功能模拟，必须修复 |
| 🟡 P1 | 13 | ⏳ | 重要功能不完整 |
| 🟢 P2 | 95 | 📋 | 配置优化/测试相关 |

---

## 🔴 P0 - 核心功能模拟 (必须修复)

### T-P0.1: AI/Embedding 模拟实现替换

**模块**: `cis-core/src/ai/`, `cis-core/src/memory/`, `cis-core/src/task/`

**问题文件**:
- `cis-core/src/ai/embedding.rs:380` - 模拟实现注释
- `cis-core/src/memory/service.rs:929` - 基于哈希的确定性向量生成
- `cis-core/src/task/vector.rs:415` - 模拟 embedding service

**修复方案**:
```rust
// 替换为真实的 fastembed 调用
use crate::ai::embedding_service::EmbeddingService;

pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    let service = EmbeddingService::global().await?;
    service.embed(text).await
}
```

**验收标准**:
- [ ] 相同文本生成相同向量
- [ ] 相似文本余弦相似度 > 0.8
- [ ] 删除所有 "模拟" 注释

---

### T-P0.2: OpenCode 多轮对话模拟

**模块**: `cis-core/src/ai/opencode.rs:100`

**问题**: 通过 prompt 注入模拟多轮对话

**修复方案**:
- 实现真实的 OpenCode session 管理
- 使用 OpenCode CLI 的 session 功能

**验收标准**:
- [ ] 支持真实的多轮对话上下文
- [ ] 不使用 prompt 注入模拟

---

## 🟡 P1 - 重要功能不完整

### T-P1.1: Matrix CORS 配置

**模块**: `cis-core/src/matrix/`

**问题文件**:
- `server.rs:70` - `.allow_origin(Any)`
- `federation/server.rs:195` - `.allow_origin(Any)`

**修复方案**:
```rust
// 配置特定 origin
let allowed_origins = config.allowed_origins.clone();
app.layer(
    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([Method::GET, Method::POST])
);
```

**验收标准**:
- [ ] 支持配置允许的 origin 列表
- [ ] 生产环境不开放 `Any`

---

### T-P1.2: Matrix UDP 直连实现

**模块**: `cis-core/src/matrix/websocket/client.rs:323`

**问题**: `// TODO: 建立 UDP 直连（当前版本回退到 WebSocket）`

**修复方案**:
- 实现 UDP hole punching
- 或使用 TURN server 中继

**验收标准**:
- [ ] 支持 UDP 直连（同局域网）
- [ ] 支持 TURN 中继（跨网络）

---

### T-P1.3: Matrix Challenge Response

**模块**: `cis-core/src/matrix/websocket/client.rs:583`

**问题**: `// Create challenge response (placeholder)`

**修复方案**:
- 实现 Noise protocol 握手
- 或使用 TLS 证书验证

---

### T-P1.4: Matrix mDNS 发现

**模块**: `cis-core/src/matrix/federation/mod.rs:49`

**问题**: `Optional mDNS discovery (placeholder)`

**修复方案**:
- 集成已实现的 `MdnsService`
- 发现局域网 Matrix 节点

---

### T-P1.5: Scheduler 模拟等待

**模块**: `cis-core/src/scheduler/skill_executor.rs`

**问题**:
- Line 327: `// 模拟等待时间（实际应用中这里会等待用户输入）`
- Line 355: `// 模拟等待时间`

**修复方案**:
- 实现真实的用户输入等待
- 或使用异步通道等待外部事件

```rust
// 使用 tokio::sync::mpsc 等待真实输入
let (tx, rx) = tokio::sync::mpsc::channel(1);
// 等待用户确认
let response = rx.recv().await;
```

---

### T-P1.6: Matrix Cloud 配额模拟

**模块**: `cis-core/src/matrix/cloud/client.rs:779`

**问题**: `// 模拟配额使用（实际使用需要异步环境）`

**修复方案**:
- 实现真实的配额查询 API
- 缓存配额信息

---

### T-P1.7: Federation 实现占位

**模块**: `cis-core/src/matrix/federation_impl.rs:681`

**问题**: `// For now, return a placeholder response`

**修复方案**:
- 使用已实现的 `FederationClient`
- 实现真实的事件发送

---

## 🟢 P2 - 配置优化/测试相关

### T-P2.1: Agent Persistent 完善

**模块**: `cis-core/src/agent/persistent/`

**问题**:
- `claude.rs:577-579` - TODO: 从 session 获取信息
- `opencode.rs:569` - TODO: 实现进程扫描

**修复方案**:
- 实现 session 文件持久化
- 定期扫描进程状态

---

### T-P2.2: Network 模块 Mock 清理

**模块**: `cis-core/src/network/`

**问题**:
- `agent_session.rs:546` - placeholder
- `websocket_integration.rs:460` - Tests would require mocking
- `sync.rs:484` - mock implementations would be needed

**修复方案**:
- 实现真实的网络测试
- 或使用集成测试替代单元测试

---

### T-P2.3: Storage Placeholder 清理

**模块**: `cis-core/src/storage/room_types.rs`

**问题**: SQL placeholder 使用（这是正常的 SQL 参数化）

**说明**: 这不是虚拟实现，是 SQL 语法，无需修复

---

### T-P2.4: Intent Placeholder 清理

**模块**: `cis-core/src/intent/mod.rs`

**问题**: placeholder 变量命名

**说明**: 这是正常的字符串替换逻辑，无需修复

---

## 任务分配建议

| 任务 | 负责人 | 依赖 | 预估时间 |
|------|--------|------|----------|
| T-P0.1 | Agent-F | embedding_service.rs | 4h |
| T-P0.2 | Agent-A | - | 2h |
| T-P1.1 | Agent-C | MatrixConfig | 2h |
| T-P1.2 | Agent-D | P2PNetwork UDP | 6h |
| T-P1.3 | Agent-B | Noise protocol | 4h |
| T-P1.4 | Agent-E | MdnsService | 2h |
| T-P1.5 | Agent-F | Scheduler | 3h |
| T-P1.6 | Agent-A | Cloud API | 3h |
| T-P1.7 | Agent-B | FederationClient | 2h |

---

## 完成标准

### P0 完成标准
- [ ] 所有 AI/Embedding 调用使用真实的 fastembed
- [ ] 删除所有 "模拟" 注释和代码

### P1 完成标准
- [ ] Matrix 配置支持 CORS origin 列表
- [ ] Scheduler 实现真实等待机制
- [ ] Federation 使用真实事件发送

### P2 完成标准
- [ ] Agent Persistent 实现 session 持久化
- [ ] 所有 placeholder 标记为 "已实现" 或删除

---

## 验收检查

```bash
# 检查是否还有模拟代码
grep -rn "模拟\|mock\|stub\|placeholder" --include="*.rs" cis-core/src cis-node/src | grep -v "test\|Test" | wc -l

# 期望输出: 0
```
