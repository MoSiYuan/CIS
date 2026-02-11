# CIS v1.1.0 任务快速开始

**目标**: 让 AI Agent 快速理解并执行任务  
**阅读时间**: 5 分钟  

---

## 🚀 5分钟快速开始

### Step 1: 确认你的角色 (30秒)

| 如果你擅长... | 你的角色 | 立即执行 |
|--------------|---------|---------|
| Rust + 内存安全 | Agent-A | [P1-1 内存安全](phase1/P1-1_memory_safety.md) |
| 网络 + WebSocket | Agent-B | [P1-2 WebSocket测试](phase1/P1-2_websocket_tests.md) |
| Rust + 测试 | Agent-C | P1-3 项目注册表 |
| CI/CD + DevOps | Agent-D | P1-5 CI/CD强化 |
| Rust + 代码质量 | Agent-E | P1-6 编译警告 |
| 技术写作 | Agent-F | P1-7 文档测试 |

### Step 2: 阅读你的任务文档 (2分钟)

```bash
# 例如 Agent-A 执行:
cat plan/tasks/phase1/P1-1_memory_safety.md

# 重点关注:
# - 问题描述
# - 原子任务列表
# - 验收标准
```

### Step 3: 开始执行 (立即)

```bash
# 1. 创建分支
git checkout -b feat/phase1-p1-1-memory-safety

# 2. 查看相关代码
code cis-core/src/memory/service.rs

# 3. 按任务文档执行...
```

---

## 📊 当前可并行任务 (Week 1)

**无需等待，可立即开始**:

| 任务 | 文件 | 预估 | 冲突风险 |
|------|------|------|----------|
| P1-1 内存安全 | memory/service.rs, storage/db.rs | 3天 | 低 |
| P1-2 WebSocket | matrix/websocket/server.rs | 2天 | 低 |
| P1-3 注册表 | skill/project_registry.rs | 1.5天 | 低 |
| P1-5 CI/CD | .github/workflows/ | 2天 | 无 |
| P1-6 警告 | 全局 | 1天 | 中 |
| P1-7 文档 | 全局 | 1.5天 | 低 |

**冲突提示**:
- P1-6 (编译警告) 可能触及多个文件，建议最后执行
- 其他任务文件隔离，可安全并行

---

## ✅ 任务完成检查清单

每个任务完成后必须检查:

```markdown
## 代码检查
- [ ] 编译通过: `cargo build -p cis-core`
- [ ] 测试通过: `cargo test -p cis-core --lib`
- [ ] 无新警告: `cargo clippy -p cis-core`

## 文档检查  
- [ ] 任务文档已更新 (勾选完成的任务)
- [ ] 代码注释完整
- [ ] 如有API变更，文档已更新

## 提交检查
- [ ] 提交信息规范
- [ ] 无敏感信息泄露
- [ ] 无大文件提交
```

---

## 🔗 关键链接

### 必读
- [任务索引](TASK_INDEX.md) - 所有任务清单
- [v1.1.0 路线图](../v1.1.0_ROADMAP.md) - 整体规划

### 参考
- [原始评估报告](../archives/kimi_agent.md) - 当前状态 75%
- [详细计划](../CIS_PRODUCTION_READINESS_PLAN.md) - 156个原子任务

### 设计文档
- [架构设计](../ARCHITECTURE_DESIGN.md)
- [DAG设计](../DAG_SKILL_ARCHITECTURE.md)
- [GUI设计](../GUI_ELEMENT_STYLE_DESIGN.md)

---

## 💡 常见问题

### Q: 我的任务依赖其他任务怎么办？
**A**: Week 1 的任务大多无依赖。如有依赖:
1. 检查依赖任务是否已完成
2. 如未完成，可与上游Agent协商
3. 或基于上游分支开发

### Q: 发现任务文档不清楚怎么办？
**A**: 
1. 查看原始详细计划: `CIS_PRODUCTION_READINESS_PLAN.md`
2. 查看相关设计文档
3. 询问 Lead Agent

### Q: 遇到技术难题怎么办？
**A**:
1. 记录问题到 `plan/tasks/issues/{task-id}.md`
2. 尝试备选方案
3. 如阻塞超过半天，请求协助

### Q: 如何报告进度？
**A**:
```markdown
## 进度报告
Agent-X, 任务 P1-X, Day Y

已完成:
- [x] 子任务1
- [x] 子任务2

进行中:
- [ ] 子任务3 (预计今天完成)

阻塞:
- 问题描述
- 尝试的解决方案
```

---

## 🎯 本周目标

**Week 1 完成标准**:
```
Phase 1 (稳定性) 80% 完成:
  ✅ P1-1 内存安全 - 修复3个测试
  ✅ P1-2 WebSocket - 修复2个测试
  ✅ P1-3 注册表 - 修复2个测试
  ✅ P1-5 CI/CD - 工作流优化
  ⚠️ P1-6 警告 - 清理到<10个
  ⚠️ P1-7 文档 - 覆盖率提升
```

**验收命令**:
```bash
# Lead Agent 在 Week 1 结束时运行:
cargo test -p cis-core --lib 2>&1 | grep "test result"
# 预期: 通过率 > 90%

cargo clippy -p cis-core 2>&1 | grep "warning:" | wc -l
# 预期: < 10 个警告
```

---

## 🏁 下一步

完成 Week 1 任务后:
1. 更新任务文档 (勾选完成)
2. 提交 PR
3. 领取 Week 2 任务

**Week 2 预告**:
- P1-4: E2E 测试套件
- P1-8: Phase 1 验收
- P2-1: WASM 执行 (新阶段)

---

**开始执行吧！** 🚀

选择你的任务:
- [ ] [P1-1 内存安全](phase1/P1-1_memory_safety.md)
- [ ] [P1-2 WebSocket](phase1/P1-2_websocket_tests.md)
- [ ] P1-3 注册表
- [ ] P1-5 CI/CD
- [ ] P1-6 编译警告
- [ ] P1-7 文档测试
