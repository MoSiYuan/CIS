# 🚀 Agent 任务启动指南

## 任务已分配完毕！

6 个 Agent 的任务分配已完成，现在可以并行开始工作。

---

## Agent 任务速查

| Agent | 任务 | 优先级 | 依赖 | 状态 |
|-------|------|-------|------|------|
| **Agent-A** | T1.1 + T3.1 | P0 | - | 🔴 可立即开始 |
| **Agent-B** | T1.2 + T4.1 | P0 | - | 🔴 可立即开始 |
| **Agent-C** | T1.3 + T2.2 + T3.3 | P0/P1 | - | 🔴 可立即开始 |
| **Agent-D** | T2.1 + T3.2 | P1 | T1.1, T1.2 | 🟡 等待中 |
| **Agent-E** | T2.3 + T3.4 | P1/P2 | - | 🔴 可立即开始 |
| **Agent-F** | T4.2 + T4.3 | P2 | T2.2 (T4.2) | 🔴 T4.3 可立即开始 |

---

## 可立即开始的任务（无依赖）

### 🔴 立即开始（5 个任务）

```bash
# Agent-A: T1.1 - mDNS 服务封装
git checkout -b agent-a/t1.1-mdns
# 阅读: plan/tasks/T1.1_mdns_service/README.md
# 阅读: plan/tasks/agent-a-t1.1/ASSIGNMENT.md

# Agent-B: T1.2 - QUIC 传输层
git checkout -b agent-b/t1.2-quic
# 阅读: plan/tasks/T1.2_quic_transport/README.md
# 阅读: plan/tasks/agent-b-t1.2/ASSIGNMENT.md

# Agent-C: T1.3 - PID 文件管理
git checkout -b agent-c/t1.3-pid
# 阅读: plan/tasks/T1.3_pid_manager/README.md
# 阅读: plan/tasks/agent-c-t1.3/ASSIGNMENT.md

# Agent-E: T2.3 - Agent 进程检测
git checkout -b agent-e/t2.3-detector
# 阅读: plan/tasks/T2.3_agent_detector/README.md
# 阅读: plan/tasks/agent-e-t2.3/ASSIGNMENT.md

# Agent-F: T4.3 - Embedding 服务替换
git checkout -b agent-f/t4.3-embedding
# 阅读: plan/tasks/T4.3_embedding_service/README.md
# 阅读: plan/tasks/agent-f-t4.3/ASSIGNMENT.md
```

---

## 等待中的任务

### 🟡 Agent-D（关键路径）

**Agent-D 必须等待**:
- Agent-A 完成 T1.1 (MdnsService)
- Agent-B 完成 T1.2 (QuicTransport)

**Agent-D 阻塞了**:
- T3.1, T3.2, T4.1, T4.2

**建议**: Agent-D 可以先阅读文档，准备整合方案。

### 🟡 Agent-F - T4.2

等待 Agent-C 完成 T2.2 (MatrixServerManager)

---

## 关键路径

```
T1.1 (Agent-A) ──┐
                 ├──→ T2.1 (Agent-D) ──→ T3.1/3.2 (Agent-A/D) ──→ ...
T1.2 (Agent-B) ──┘

T1.3 (Agent-C) ──→ T2.2 (Agent-C) ──→ T3.3 (Agent-C) ──→ T4.2 (Agent-F)
```

**关键路径时间**: 最短 12 小时交付

---

## 快速开始流程

### Step 1: 确认你的 Agent 身份
查看上面的表格，确认你的任务。

### Step 2: 阅读任务文档
```bash
# 例如 Agent-A
cat plan/tasks/T1.1_mdns_service/README.md
cat plan/tasks/agent-a-t1.1/ASSIGNMENT.md
```

### Step 3: 创建分支
```bash
git checkout -b agent-{x}/t{x}.{x}-{name}
# 例如: git checkout -b agent-a/t1.1-mdns
```

### Step 4: 开始实现
按照任务文档的接口定义实现功能。

### Step 5: 单元测试
```bash
cargo test --package cis-core your_module -- --nocapture
```

### Step 6: 提交 PR
```bash
git add -A
git commit -m "feat: Implement T1.1 mDNS service

- Add MdnsService with discover/shutdown
- Use mdns-sd for mDNS broadcast and discovery
- Add unit tests with >80% coverage

Closes T1.1"
git push origin agent-a/t1.1-mdns
```

---

## 协作规则

### 1. 接口契约
- 严格按照任务文档的接口定义实现
- 不要修改接口签名（如有需要，讨论后统一修改）

### 2. 文档同步
- 如果实现与文档有差异，更新文档并通知相关 Agent
- 在 `plan/tasks/{task}/QUESTIONS.md` 记录问题

### 3. PR 规范
```
标题: feat: T{x}.{x} - {任务名称}

内容:
- 实现了哪些接口
- 测试覆盖率
- 依赖情况
- 如何使用
```

### 4. 每日同步
建议每天汇报进度：
- 完成了什么
- 遇到了什么问题
- 是否需要帮助

---

## 常见问题

### Q: 我的任务依赖别人的任务怎么办？
**A**: 
- 如果依赖是强依赖（需要使用对方接口），等待对方完成
- 可以先准备代码结构，使用 mock 接口占位
- 或先完成不依赖的部分

### Q: 发现任务文档有问题怎么办？
**A**:
- 在 `plan/tasks/{task}/QUESTIONS.md` 记录问题
- 通知任务分配者
- 不要擅自修改其他 Agent 的任务文档

### Q: 可以修改其他人的代码吗？
**A**:
- 不建议直接修改
- 如果需要修改，通过 PR 提交，说明原因
- 紧急情况下，修改后通知相关 Agent

### Q: 测试需要真实网络环境怎么办？
**A**:
- 使用 mock/stub 进行单元测试
- 集成测试在多个任务完成后统一进行
- 在 PR 中说明测试方式

---

## 任务文档索引

### 任务规格文档
- `plan/tasks/T1.1_mdns_service/README.md`
- `plan/tasks/T1.2_quic_transport/README.md`
- `plan/tasks/T1.3_pid_manager/README.md`
- `plan/tasks/T2.1_p2p_network/README.md`
- `plan/tasks/T2.2_matrix_lifecycle/README.md`
- `plan/tasks/T2.3_agent_detector/README.md`
- `plan/tasks/T3.1_p2p_discover_cmd/README.md`
- `plan/tasks/T3.2_p2p_connect_cmd/README.md`
- `plan/tasks/T3.3_matrix_cmd/README.md`
- `plan/tasks/T3.4_agent_status_cmd/README.md`
- `plan/tasks/T4.1_dht_operations/README.md`
- `plan/tasks/T4.2_federation_events/README.md`
- `plan/tasks/T4.3_embedding_service/README.md`

### Agent 分配文档
- `plan/tasks/agent-a-t1.1/ASSIGNMENT.md`
- `plan/tasks/agent-b-t1.2/ASSIGNMENT.md`
- `plan/tasks/agent-c-t1.3/ASSIGNMENT.md`
- `plan/tasks/agent-d-t2.1/ASSIGNMENT.md`
- `plan/tasks/agent-e-t2.3/ASSIGNMENT.md`
- `plan/tasks/agent-f-t4.3/ASSIGNMENT.md`

### 总索引
- `plan/tasks/TASK_INDEX.md`
- `plan/TASK_BREAKDOWN_v1.1.3.md`

---

## 联系方式

- **技术问题**: 在任务目录创建 `QUESTIONS.md`
- **进度汇报**: 每日简短更新
- **紧急问题**: 直接联系协调者

---

## 🎯 目标

**本周目标**: 完成 Phase 1 (T1.1, T1.2, T1.3, T2.3, T4.3)  
**下周目标**: 完成 Phase 2 (T2.1, T2.2)  
**第 3 周**: 完成 Phase 3-4 + 集成测试

---

**祝各位 Agent 工作顺利！**
