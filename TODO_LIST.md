# CIS 项目 TODO 列表

> 生成时间: $(date)
> 总计: 90 个显式 TODO + 35+ 个简化实现

## 📊 项目状态概览

| 类别 | 数量 | 完成度 |
|------|------|--------|
| 🔴 高优先级 TODO | 15 | 93% (14/15) |
| 🟡 中优先级 TODO | ~30 | ~60% |
| 🟢 低优先级 TODO | ~45 | ~30% |
| ⚠️ 简化实现 | 35+ | 跟踪中 |

### 关键简化实现提醒

以下简化实现**严重影响功能正确性**，建议优先处理：

1. **Embedding 服务** (cis-core/src/ai/embedding.rs:148) - 使用确定性向量而非真实 ONNX 推理，导致向量搜索完全失效
2. **DID 解析/验证** (network/sync.rs:455, did_verify.rs:305) - 联邦节点身份验证不可用
3. **Matrix 事件签名** (matrix/nucleus.rs:1064) - 联邦消息安全验证缺失
4. **Skill Chain 条件** (skill/chain.rs:390) - 条件判断始终为真，决策逻辑失效

---

## 🔴 高优先级 (核心功能)

### Worker 进程管理 (cis-node/src/commands/worker.rs) - 12个
| 行号 | TODO | 说明 | 状态 |
|------|------|------|------|
| ~282 | ~~worker stop/status 实际实现~~ | ~~进程管理~~ | ✅ 已完成 |
| ~297 | ~~Query actual worker status~~ | ~~状态查询~~ | ✅ 已完成 |
| ~305 | ~~List all active workers~~ | ~~Worker列表~~ | ✅ 已完成 |
| ~322 | ~~Integrate with actual node initialization~~ | ~~节点初始化~~ | ✅ 已完成 |
| ~351 | ~~Integrate with actual Matrix client~~ | ~~Matrix集成~~ | ✅ 已完成 |
| ~387 | ~~Integrate with actual Matrix event polling~~ | ~~事件轮询~~ | ✅ 已完成 |
| ~410 | ~~Implement task cancellation~~ | ~~任务取消~~ | ✅ 已完成 |
| 2199 | ~~Integrate with SkillManager~~ | ~~Skill调用~~ | ✅ 已实现（在execute_skill_task中初始化SkillManager） |
| ~693 | ~~Send actual event to Matrix room~~ | ~~事件发送~~ | ✅ 已完成 |
| ~726 | ~~Send actual heartbeat event~~ | ~~心跳机制~~ | ✅ 已完成 |
| ~735 | ~~Implement actual health checks~~ | ~~健康检查~~ | ✅ 已完成 |
| ~751 | ~~Implement actual cleanup~~ | ~~资源清理~~ | ✅ 已完成 |

### GLM API (cis-core/src/glm/mod.rs)
| 行号 | TODO | 说明 | 状态 |
|------|------|------|------|
| 457 | ~~通过 Matrix Room 发送~~ | ~~DAG执行通知~~ | ✅ 已实现（通过MatrixHttpClient） |

### Skill 执行器 (cis-core/src/scheduler/skill_executor.rs)
| 行号 | TODO | 说明 | 状态 |
|------|------|------|------|
| 152 | ~~实现 WASM 运行时调用~~ | WASM执行 | 🟡 推迟到后续版本 |
| 155 | ~~WASM execution not yet implemented~~ | 同152 | 🟡 推迟到后续版本 |
| ~279 | ~~实现用户确认机制~~ | 四级决策 | ✅ 基础实现完成（需 Matrix 集成） |
| ~286 | ~~实现仲裁机制~~ | 四级决策 | ✅ 基础实现完成（需投票协议） |

### Skill 管理器 (cis-core/src/skill/manager.rs)
| 行号 | TODO | 说明 | 状态 |
|------|------|------|------|
| 379 | ~~启动 Skill 的事件循环~~ | 事件处理 | ✅ 已完成 |
| 405 | ~~停止 Skill 的事件循环~~ | 生命周期 | ✅ 已完成 |

### Matrix Nucleus (cis-core/src/matrix/nucleus.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 1110 | ~~根据 content 类型返回对应的事件类型~~ | ~~事件路由~~ | ✅ 已实现（通过JSON字段检查和type_name） |

---

## 🟡 中优先级 (功能完善)

### IM 命令 (cis-node/src/commands/im.rs) - 8个
| 行号 | TODO | 说明 |
|------|------|------|
| 168 | ~~调用 IM Skill 发送消息~~ | ~~send~~ | ✅ 已实现（通过 SkillManager 发送事件） |
| 185 | ~~调用 IM Skill 获取会话列表~~ | ~~list~~ | ✅ 已实现（通过 SkillManager 发送事件） |
| 212 | ~~调用 IM Skill 获取消息历史~~ | ~~history~~ | 🟡 已实现框架（需要完整 Skill 响应处理） |
| 247 | ~~调用 IM Skill 搜索消息~~ | ~~search~~ | 🟡 已实现框架（需要完整 Skill 响应处理） |
| 284 | ~~调用 IM Skill 创建会话~~ | ~~create~~ | 🟡 已实现框架（需要完整 Skill 响应处理） |
| 302 | ~~调用 IM Skill 批量标记已读~~ | ~~mark-read~~ | 🟡 已实现框架（需要完整 Skill 响应处理） |
| 305 | ~~调用 IM Skill 标记单条消息已读~~ | ~~mark-read~~ | 🟡 已实现框架（需要完整 Skill 响应处理） |
| 319 | ~~调用 IM Skill 获取会话信息~~ | ~~info~~ | 🟡 已实现框架（需要完整 Skill 响应处理） |

### 网络命令 (cis-node/src/commands/network.rs) - 5个
| 行号 | TODO | 说明 |
|------|------|------|
| 226 | ~~Broadcast to peers if connected~~ | ~~广播~~ | ✅ 已实现框架（需要系统 P2P 实例集成） |
| 420 | ~~Implement broadcast~~ | ~~广播实现~~ | ✅ 已实现框架（需要系统 P2P 实例集成） |
| 424 | ~~Implement sync from specific peer~~ | ~~同步~~ | ✅ 已实现框架（需要系统 P2P 实例集成） |
| 439 | Load from audit logger | 审计日志 |
| 525 | Remove this when acl.bump_version() is public | API更新 |

### DAG CLI (cis-node/src/commands/dag.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 584 | ~~Query worker_instances table~~ | ~~Worker查询~~ | ✅ 已实现（使用 WorkerService） |

### WASM Host (cis-core/src/wasm/host.rs) - 3个
| 行号 | TODO | 说明 |
|------|------|------|
| 298 | 实现实际的搜索功能 | search |
| 467 | 从 core db 读取配置 | config |
| 515 | 实际实现配置存储 | config store |

### Skill Router (cis-core/src/skill/router.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 959 | 保存到 skill_compatibility 表 | 兼容性存储 |

### Skill Chain (cis-core/src/skill/chain.rs) - 2个
| 行号 | TODO | 说明 |
|------|------|------|
| 391 | 实现条件表达式解析 | 条件解析 |
| 578 | 使用向量相似度匹配 | 语义匹配 |

### 网络同步 (cis-core/src/network/sync.rs) - 2个
| 行号 | TODO | 说明 |
|------|------|------|
| 401 | Verify signature | 签名验证 |
| 457 | Implement proper DID resolution | DID解析 |

---

## 🟢 低优先级 (优化/增强)

### GUI - 远程会话 (cis-gui/src/remote_session.rs) - 8个
| 行号 | TODO | 说明 |
|------|------|------|
| 67 | Establish WebSocket connection | WebSocket |
| 68 | Perform DID challenge/response | 身份验证 |
| 69 | Spawn Agent process on remote | 远程代理 |
| 85 | Send agent start command | 启动命令 |
| 86 | Setup PTY forwarding | PTY转发 |
| 99 | Wrap in PTY data frame | 数据封装 |
| 119 | Send disconnect | 断开连接 |
| 120 | Close WebSocket | 关闭连接 |

### GUI - 主应用 (cis-gui/src/app.rs) - 5个
| 行号 | TODO | 说明 |
|------|------|------|
| 335 | Initiate remote session | 远程会话 |
| 345 | Open verification dialog | 验证对话框 |
| 358 | Call API to confirm DAG | DAG确认 |
| 363 | Call API to reject DAG | DAG拒绝 |
| 368 | Fetch from API | API获取 |

### GUI - 节点标签 (cis-gui/src/node_tabs.rs) - 3个
| 行号 | TODO | 说明 |
|------|------|------|
| 239 | Emit event | 事件发射 |
| 244 | Emit event | 事件发射 |
| 249 | Open verification dialog | 验证对话框 |

### GUI - 终端面板 (cis-gui/src/terminal_panel.rs) - 2个
| 行号 | TODO | 说明 |
|------|------|------|
| 62 | Send to PTY | PTY发送 |
| 67 | Send resize to PTY | PTY调整大小 |

### GUI - 节点管理 (cis-gui/src/node_manager.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 124 | Count | 计数显示 |

### GUI - GLM面板 (cis-gui/src/glm_panel.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 278 | 打开详细视图 | 详情视图 |

### Skill - IM (skills/im/src/) - 4个
| 文件 | 行号 | TODO | 说明 |
|------|------|------|------|
| handler.rs | 284 | 实现批量标记 | 消息标记 |
| handler.rs | 337 | 实现加入会话逻辑 | 加入会话 |
| handler.rs | 349 | 实现离开会话逻辑 | 离开会话 |
| message.rs | 235 | 发送到远程节点 | 联邦消息 |

### Skill - 飞书IM (skills/cis-feishu-im/src/) - 14个
| 文件 | 行号 | TODO | 说明 |
|------|------|------|------|
| feishu/mod.rs | 208 | 实现飞书 API 调用 | API |
| feishu/mod.rs | 216 | 实现飞书 API 调用 | API |
| feishu/mod.rs | 223 | 实现飞书 API 调用 | API |
| feishu/mod.rs | 235 | 实现飞书 API 调用 | API |
| poller.rs | 406 | 解析消息内容检测@ | @检测 |
| poller.rs | 454 | 向"节点监控群"发送上线消息 | 通知 |
| poller.rs | 462 | 向"节点监控群"发送离线消息 | 通知 |
| session.rs | 245 | 实现数据库持久化 | 持久化 |
| session.rs | 252 | 从数据库加载会话 | 加载 |
| context.rs | 148 | 实现 SQLite 持久化 | 持久化 |
| context.rs | 164 | 实现 SQLite 加载 | 加载 |
| webhook.rs | 128 | 实现签名验证 | 安全 |
| webhook.rs | 265 | 检测 @ 提及 | @检测 |
| webhook.rs | 280 | 调用 AI 生成回复 | AI回复 |

### 项目会话 (cis-core/src/project/session.rs) - 4个
| 行号 | TODO | 说明 |
|------|------|------|
| 71 | 从 manifest 解析 skill 元数据 | 元数据解析 |
| 188 | 实现具体的执行逻辑 | 执行逻辑 |
| 203 | 实现记忆存储 | 记忆存储 |
| 213 | 实现记忆读取 | 记忆读取 |

### Matrix WebSocket (cis-core/src/matrix/websocket/client.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 314 | 建立 UDP 直连 | UDP穿透 |

### Matrix Sync (cis-core/src/matrix/sync/consumer.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 227 | 实现 request-response 模式 | 同步模式 |

### Local Executor (cis-core/src/scheduler/local_executor.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 228 | 通过 Matrix Room 发送事件 | 事件发送 |

### DID 验证 (cis-core/src/network/did_verify.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 304 | Implement proper DID resolution | DID解析 |

### Skill 命令 (cis-node/src/commands/skill.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 487 | Implement actual skill method invocation | Skill调用 |

### 任务命令 (cis-node/src/commands/task.rs)
| 行号 | TODO | 说明 |
|------|------|------|
| 255 | Actually execute tasks | 任务执行 |

---

## 按模块汇总

| 模块 | TODO数量 | 优先级 |
|------|----------|--------|
| cis-node/commands/worker.rs | 12 | 🔴 高 |
| cis-node/commands/im.rs | 8 | 🟡 中 |
| cis-gui/remote_session.rs | 8 | 🟢 低 |
| cis-node/commands/network.rs | 5 | 🟡 中 |
| cis-gui/app.rs | 5 | 🟢 低 |
| cis-feishu-im | 14 | 🟡 中 |
| cis-core/scheduler | 5 | 🔴 高 |
| cis-core/skill | 5 | 🟡 中 |
| 其他 | 28 | 🟢 低 |

---

## 建议处理顺序

### 第一阶段 (核心功能完善)
1. ✅ Worker 任务执行与结果回传 (已完成)
2. Matrix Room 实际事件收发 (worker.rs: 351, 693, 726)
3. Worker 进程管理 (worker.rs: 282, 297, 305)
4. GLM API Matrix 发送 (glm/mod.rs: 457)

### 第二阶段 (功能扩展)
5. IM Skill 完整调用 (im.rs: 8个TODO)
6. WASM 运行时 (skill_executor.rs: 152-155)
7. 四级决策机制 (skill_executor.rs: 279, 286)

### 第三阶段 (GUI与优化)
8. GUI 远程会话 (remote_session.rs: 8个TODO)
9. GUI 事件集成 (app.rs, node_tabs.rs)
10. 飞书IM完善 (cis-feishu-im: 14个TODO)

---

## ⚠️ 简化实现跟踪 (需要完善)

这些功能目前有简化实现或占位实现，需要根据优先级逐步完善：

### 🔴 高优先级简化实现

| 文件 | 行号 | 简化内容 | 影响 | 建议优先级 |
|------|------|----------|------|------------|
| cis-core/src/ai/embedding.rs | 148-165 | ~~LocalEmbeddingService 使用确定性向量~~ | ~~向量搜索~~ | 🟡 中 (框架已实现，ONNX 推理需要进一步适配) |
| cis-core/src/ai/embedding.rs | 102-105 | ~~Tokenizer 使用默认 WordPiece~~ | ~~文本分割~~ | ✅ 已修复 (现在从模型目录加载 tokenizer) |
| cis-core/src/network/sync.rs | 455-459 | ~~DID 解析为占位实现~~ | ~~无法验证联邦节点身份~~ | ✅ 已修复 (实现 DID 解析到公钥) |
| cis-core/src/network/did_verify.rs | 305 | ~~DID 验证为 placeholder~~ | ~~联邦安全~~ | ✅ 已修复 (实现 DID 到公钥解析) |
| cis-core/src/matrix/nucleus.rs | 1064 | ~~事件签名验证为 placeholder~~ | ~~Matrix 安全~~ | ✅ 已修复 (实现基于 DID 的签名验证) |
| cis-core/src/matrix/federation/server.rs | 451 | ~~事件签名验证为占位实现~~ | ~~联邦安全~~ | ✅ 已修复 (实现完整签名验证流程) |

### 🟡 中优先级简化实现

| 文件 | 行号 | 简化内容 | 影响 | 建议优先级 |
|------|------|----------|------|------------|
| cis-core/src/skill/chain.rs | 390 | 条件表达式解析简化（始终执行） | Skill Chain 条件判断失效 | 🟡 中 |
| cis-core/src/skill/router.rs | 729 | 意图解析为简化版 | Skill 匹配准确度低 | 🟡 中 |
| cis-core/src/task/vector.rs | 187-190, 316 | 任务向量搜索使用 ID 作为标题占位 | 任务搜索结果不准确 | 🟡 中 |
| cis-core/src/p2p/dht.rs | 130, 149, 190 | DHT 路由为简化实现 | P2P 发现效率低 | 🟡 中 |
| cis-core/src/p2p/sync.rs | 324 | 获取变更列表返回空 | 同步功能不完整 | 🟡 中 |
| cis-core/src/conversation/context.rs | 400, 538 | 对话上下文简化处理 | 对话记忆功能受限 | 🟡 中 |
| cis-core/src/agent/bridge.rs | 162 | 记忆桥接简化实现 | Agent 记忆功能受限 | 🟡 中 |
| cis-core/src/wasm/host.rs | 699-753 | 多个 WASM host 函数为 stub | WASM Skill 功能受限 | 🟡 中 |
| cis-core/src/wasm/skill.rs | 51, 139 | AI 回调和事件处理简化 | WASM Skill 智能功能受限 | 🟡 中 |

### 🟢 低优先级简化实现

| 文件 | 行号 | 简化内容 | 影响 | 建议优先级 |
|------|------|----------|------|------------|
| cis-core/src/glm/mod.rs | 586, 816 | DAG 状态和诊断简化 | GLM 状态监控功能受限 | 🟢 低 |
| cis-core/src/matrix/routes/login.rs | 82 | 登录接受任意用户名密码 | 安全性问题（仅开发） | 🟢 低 |
| cis-core/src/matrix/routes/sync.rs | 172 | 同步接口简化 | 兼容性可能有问题 | 🟢 低 |
| cis-node/src/commands/worker.rs | 911, 1523, 1570, 1576, 1695, 1855, 2090, 2509 | 多个 Worker 功能为 placeholder | Windows 支持、日志、统计等功能不完整 | 🟢 低 |
| cis-node/src/commands/network.rs | 476 | 审计日志未实现 | 审计功能缺失 | 🟢 低 |
| cis-node/src/commands/task.rs | 256 | 任务执行未完全实现 | 任务功能受限 | 🟢 低 |
| cis-node/src/commands/skill.rs | 497 | Skill 方法调用未完全实现 | Skill 调用功能受限 | 🟢 低 |
| cis-node/src/commands/dag.rs | 485 | 任务修正持久化未实现 | 数据持久化问题 | 🟢 低 |

---

*注: 此列表由脚本自动生成，可能会有遗漏或过时，请以代码为准*
