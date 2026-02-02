# CIS 未完成任务清单

## ✅ 已完成 (2026-02-02)

### Phase 1: 基础架构 ✅
- [x] 跨平台目录结构 (macOS/Linux/Windows)
- [x] 数据库隔离 (核心/Split 分离)
- [x] 热插拔支持
- [x] 私域/公域记忆系统
- [x] 记忆加密模块
- [x] Agent Provider 抽象 (Claude/Kimi/Aider)
- [x] 双向集成架构 (CIS ↔ Agent)
- [x] Skill Manifest 标准
- [x] 项目配置系统
- [x] 初始化向导

### Phase 2: WASM Runtime + CLI ✅
- [x] wasmer WASM Runtime 集成
- [x] Host API 实现 (memory_get, memory_set, ai_chat, log, http_post)
- [x] CLI 工具框架 (clap)
- [x] 基础命令实现 (init, skill, memory, task, agent, doctor)

### Phase 3: MATRIX-final 联邦架构 ✅
- [x] MatrixNucleus 统一核心
- [x] DID 身份系统 (Ed25519 + did:cis:)
- [x] Skill = Matrix Room 视图
- [x] Room 联邦标记 (federate)
- [x] Cloud Anchor 服务发现
- [x] Noise Protocol XX 握手
- [x] 事件联邦广播
- [x] 强类型 Skill 消息 (io.cis.*)
- [x] 断线同步队列消费者

---

## 🚧 未完成

### Phase 3: 功能完善

#### 3.1 WASM 功能完善
- [ ] Host API 内存访问完整实现
- [ ] WASI 支持
- [ ] WASM Skill 与 SkillManager 完整集成
- [ ] 内存管理（malloc/free）

#### 3.2 CLI 功能完善
- [ ] 配置文件解析集成
- [ ] 异步命令完整实现
- [ ] 输出格式化（JSON/Table）
- [ ] Shell 补全脚本

#### 3.3 测试
- [ ] WASM 模块测试
- [ ] CLI 集成测试
- [ ] 端到端测试

---

### Phase 4: IM Skill (Claude)

```rust
// skills/im/src/lib.rs
use cis_skill_sdk::{Skill, Event, Result};
use cis_skill_sdk::im::*;

pub struct ImSkill {
    db: ImDatabase,
}

impl Skill for ImSkill {
    fn name(&self) -> &str { "im" }
    
    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        // 处理 IM 消息
    }
}
```

- [ ] 消息收发接口
- [ ] 会话管理
- [ ] 用户/群组管理
- [ ] 消息存储 (skills/data/im/data.db)
- [ ] 历史记录查询

---

### Phase 5: P2P 网络 (预留)

- [ ] 节点发现 (mDNS/DHT)
- [ ] 连接管理 (QUIC)
- [ ] 公域记忆同步 (Gossip)
- [ ] 冲突解决 (CRDT)

---

### Phase 6: 发布准备

#### 6.1 构建脚本
- [ ] macOS `.app` bundle + `dmg`
- [ ] Linux AppImage + deb
- [ ] Windows MSI

#### 6.2 文档
- [ ] 用户指南
- [ ] Skill 开发文档
- [ ] API 文档

#### 6.3 质量
- [ ] 单元测试覆盖 > 80%
- [ ] 集成测试
- [ ] 跨平台测试

---

## 📊 进度概览

| 阶段 | 进度 | 关键任务 |
|------|------|---------|
| Phase 1 | 100% | ✅ 基础架构完成 |
| Phase 2 | 90% | ✅ WASM + CLI 核心完成，需完善 |
| Phase 3 | 20% | 🚧 功能完善中 |
| Phase 4 | 0% | ⏳ IM Skill 待 Claude |
| Phase 5 | 0% | ⏳ P2P 预留 |
| Phase 6 | 0% | ⏳ 发布准备 |

---

## 🎯 下一步建议

### 方案 A: 完善当前功能
1. 修复 WASM 编译警告
2. 完善 CLI 命令实现
3. 添加测试

### 方案 B: 并行推进
- **Claude**: 开发 IM Skill (使用 Native 模式先跑通)
- **开发**: 完善 WASM 和 CLI
- **测试**: 编写集成测试

### 方案 C: 发布准备
1. 冻结功能
2. 完善文档
3. 构建发布版本
