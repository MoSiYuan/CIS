# 需求分析: cis-feishu-im Skill

## 需求描述

实现 CIS 与飞书 IM 的对接功能，使 Agent 能够：
1. **监听飞书消息** - 接收来自飞书群聊或私聊的消息
2. **Agent 对话响应** - 使用 AI Provider (Claude/Kimi) 生成回复
3. **消息推送** - 将 AI 响应发送回飞书

## 功能清单

### 核心功能 (P0)
- [ ] Webhook 服务器 - 接收飞书事件推送
- [ ] 消息解析 - 解析飞书消息格式
- [ ] AI 对话集成 - 调用 cis-core AI Provider
- [ ] 消息回复 - 发送回复到飞书

### 扩展功能 (P1)
- [ ] 多轮对话支持 - 维护对话上下文
- [ ] 事件订阅 - 订阅飞书事件（如用户加入、退出等）
- [ ] 富文本消息 - 支持卡片、图片等

### 高级功能 (P2)
- [ ] 多租户支持 - 支持多个飞书应用
- [ ] 消息加密 - 加密存储敏感对话
- [ ] 群管理 - 管理机器人所在的群组

## 技术约束

### CIS 架构约束
- 必须使用 `cis-core::ai` 模块的 AI Provider
- 遵循 CIS 第一性原理（本地优先）
- 使用奥卡姆剃刀原则（保持简洁）

### 飞书 API 约束
- 飞书使用 Webhook 模式推送事件
- 需要验证请求签名（保证安全性）
- 支持 JSON 格式的事件数据

### 参考资源
- [飞书开放平台](https://open.feishu.cn/)
- [自定义机器人使用指南](https://open.feishu.cn/document/ukTMukTMukTM/ucTM5YjL3ETO24yNxkjN)
- [larkrs-client](https://docs.rs/larkrs-client/) - Rust SDK

## 待澄清问题

### 1. 部署模式
**问题**: Skill 将如何部署？
- **选项 A**: 作为独立 HTTP 服务器运行
- **选项 B**: 集成到 cis-node 中
- **选项 C**: 两者都支持

**推荐**: 选项 A - 独立服务器，保持解耦

### 2. Webhook URL
**问题**: Webhook URL 如何配置？
- **选项 A**: 用户手动配置到飞书后台
- **选项 B**: 自动配置（需要飞书 API）
- **选项 C**: 配置文件指定

**推荐**: 选项 A - 最简单，符合奥卡姆剃刀

### 3. 对话模式
**问题**: 对话如何触发？
- **选项 A**: @机器人触发（仅当被@时响应）
- **选项 B**: 私聊自动响应
- **选项 C**: 群聊所有消息都响应

**推荐**: 选项 A + 选项 B - 可配置

### 4. 上下文管理
**问题**: 如何维护对话历史？
- **选项 A**: 内存存储（重启丢失）
- **选项 B**: SQLite 持久化
- **选项 C**: 传递给 cis-core memory

**推荐**: 选项 B - SQLite 持久化，符合 CIS 本地主权原则

### 5. AI Provider 选择
**问题**: 使用哪个 AI Provider？
- **选项 A**: Claude CLI (默认)
- **选项 B**: Kimi Code
- **选项 C**: 可配置

**推荐**: 选项 C - 可配置，利用 cis-core 已有的抽象

### 6. 消息格式
**问题**: 支持哪些消息类型？
- **选项 A**: 仅文本
- **选项 B**: 文本 + 富文本卡片
- **选项 C**: 全部（文本、卡片、图片、文件）

**推荐**: 选项 A → 选项 B (分阶段实现)

## 风险评估

### 风险 1: Webhook 安全性
**描述**: Webhook 端点可能被恶意调用
**影响**: 高
**缓解方案**:
- 验证飞书签名
- 使用 HTTPS
- IP 白名单（可选）

### 风险 2: 并发处理
**描述**: 高并发下消息处理顺序问题
**影响**: 中
**缓解方案**:
- 使用异步运行时 (tokio)
- 消息队列（如果需要）

### 风险 3: 飞书 API 变更
**描述**: 飞书 API 可能更新
**影响**: 低
**缓解方案**:
- 使用官方 SDK (larkrs-client)
- 版本锁定

## 性能要求

- 响应时间: P99 < 3s（从接收消息到发送回复）
- 并发支持: 至少 100 req/s
- 内存占用: < 100MB（空闲状态）

## 依赖模块

### CIS Core
- `cis-core::ai` - AI Provider 抽象
- `cis-core::types` - 基础类型
- `cis-core::error` - 错误处理

### 外部 Crate
- `larkrs-client` - 飞书 SDK
- `tokio` - 异步运行时
- `axum` / `actix-web` - HTTP 服务器
- `sqlx` - SQLite（可选，用于上下文持久化）

## 兼容性

- **Rust 版本**: 1.70+
- **操作系统**: Linux / macOS / Windows
- **飞书环境**: 所有支持 Webhook 的环境

## 交付物清单

1. **代码** (`skills/cis-feishu-im/`)
   - `src/lib.rs` - Skill 入口
   - `src/webhook.rs` - Webhook 处理
   - `src/messenger.rs` - 消息处理
   - `src/context.rs` - 对话上下文
   - `src/config.rs` - 配置管理

2. **配置**
   - `config/feishu-im.toml` - 示例配置

3. **文档**
   - `docs/README.md` - 使用指南
   - `docs/DEPLOYMENT.md` - 部署指南
   - `docs/API.md` - API 文档

4. **测试**
   - `tests/webhook_test.rs` - Webhook 测试
   - `tests/integration_test.rs` - 集成测试

## 下一步

请确认以上需求分析，特别是：
1. ✅ 部署模式选择
2. ✅ Webhook URL 配置方式
3. ✅ 对话触发模式
4. ✅ AI Provider 选择

确认后进入阶段 2: 技术设计

---

**文档版本**: v1.0
**创建时间**: 2026-02-02
**作者**: CIS Team
