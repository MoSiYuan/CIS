# CIS v1.1.6 并发开发快速开始指南

> **5分钟快速上手**
> **无需等待确认，直接开始**

---

## 🚀 立即开始（3 步）

### Step 1: 初始化环境

```bash
# 克隆或更新仓库
cd /path/to/CIS
git pull origin main

# 创建开发分支
git checkout -b dev/v1.1.6

# 初始化开发环境
make -f docs/plan/v1.1.6/Makefile.tasks init-dev
```

### Step 2: 选择任务

```bash
# 查看可用任务
make -f docs/plan/v1.1.6/Makefile.tasks list-tasks

# 查看任务详情
make -f docs/plan/v1.1.6/Makefile.tasks show-task TASK_ID=P0-1.2
```

### Step 3: 开始开发

```bash
# 创建功能分支
make -f docs/plan/v1.1.6/Makefile.tasks create-branch TASK_ID=P0-1.2

# 开始编码
# ... 编写代码 ...

# 运行测试
cargo test --package cis-core wasm::tests::security
```

---

## 📋 推荐的并发任务组

### 组 1: WASM 安全加固（1 人日）

```bash
# 终端 1: 设计阶段
make -f docs/plan/v1.1.6/Makefile.tasks p0-1.1

# 终端 2-4（并行）: 实现阶段
make -f docs/plan/v1.1.6/Makefile.tasks p0-1.2  # wasmtime 集成
make -f docs/plan/v1.1.6/Makefile.tasks p0-1.3  # 燃料限制
make -f docs/plan/v1.1.6/Makefile.tasks p0-1.4  # 资源监控
```

### 组 2: 安全加固 Phase 1（5 人日，5 人并行）

```bash
# 开发者 A: WASM 沙箱
git checkout -b feature/P0-1.2
# ... 编写代码 ...

# 开发者 B: 加密改进
git checkout -b feature/P0-2.2
# ... 编写代码 ...

# 开发者 C: ACL 验证
git checkout -b feature/P0-3.2
# ... 编写代码 ...

# 开发者 D: 配置加密
git checkout -b feature/P0-4.2
# ... 编写代码 ...

# 开发者 E: 权限控制
git checkout -b feature/P0-5.2
# ... 编写代码 ...
```

### 组 3: 锁机制改进（2 人日，2 人并行）

```bash
# 开发者 A: AsyncRwLock
make -f docs/plan/v1.1.6/Makefile.tasks p0-6.2

# 开发者 B: Mutex
make -f docs/plan/v1.1.6/Makefile.tasks p0-6.3

# 同时进行，完成后合并
```

### 组 4: 向量搜索优化（2 人日，2 人并行）

```bash
# 开发者 A: 智能切换
make -f docs/plan/v1.1.6/Makefile.tasks p1-2.2

# 开发者 B: 批量加载
make -f docs/plan/v1.1.6/Makefile.tasks p1-2.3

# 同时进行
```

### 组 5: MemoryService 拆分（3 人日，3 人并行）

```bash
# 开发者 A: GET 操作
git checkout -b feature/P1-6.2

# 开发者 B: SET 操作
git checkout -b feature/P1-6.3

# 开发者 C: 搜索操作
git checkout -b feature/P1-6.4

# 同时进行，完成后合并
```

---

## 📊 并发执行时间线

### Week 1: Phase 1 安全加固（5 人团队）

```
Day 1 (Mon):          Day 2 (Tue):        Day 3 (Wed):        Day 4 (Thu):        Day 5 (Fri):
[设计阶段]           [实现阶段 A]         [实现阶段 B]         [测试阶段]          [代码审查+合并]
开发者 A: P0-1.1      P0-1.2             P0-1.3              P0-1.5              PR审查
开发者 B: P0-2.1      P0-2.2             P0-2.3              P0-2.4              PR审查
开发者 C: P0-3.1      P0-3.2             P0-3.3              P0-3.4              PR审查
开发者 D: P0-4.1      P0-4.2             P0-4.3              P0-4.4              PR审查
开发者 E: P0-5.1      P0-5.2             P0-5.3              P0-5.4              PR审查
```

### Week 2-3: Phase 3 性能优化（4 人团队）

```
Week 2, Day 1-2:    Week 2, Day 3-4:     Week 3, Day 1-2:     Week 3, Day 3-4:
[设计阶段]           [实现阶段]           [迁移+测试]          [基准测试]
团队 A (调度): P1-1.1   P1-1.2-1.3         P1-1.4              P1-1.5
团队 B (向量): P1-2.1   P1-2.2-2.3         P1-2.4-2.6          P1-2.6
团队 C (DHT):  P1-3.1   P1-3.2              P1-3.3              P1-3.4-3.5
团队 D (缓存): P1-4.1   P1-4.2              P1-4.3              (性能验证)
```

---

## 🔧 实用命令速查

### 查看任务
```bash
# 列出所有任务
make -f docs/plan/v1.1.6/Makefile.tasks list-tasks

# 查看任务详情
make -f docs/plan/v1.1.6/Makefile.tasks show-task TASK_ID=P0-1.2
```

### 创建分支
```bash
# 自动创建功能分支
make -f docs/plan/v1.1.6/Makefile.tasks create-branch TASK_ID=P0-1.2

# 或手动创建
git checkout -b feature/P0-1.2
```

### 运行测试
```bash
# 测试单个任务
make -f docs/plan/v1.1.6/Makefile.tasks test-task TASK_ID=P0-1.2

# 测试整个 Phase
make -f docs/plan/v1.1.6/Makefile.tasks test-phase-1
make -f docs/plan/v1.1.6/Makefile.tasks test-phase-2
make -f docs/plan/v1.1.6/Makefile.tasks test-phase-3
make -f docs/plan/v1.1.6/Makefile.tasks test-phase-4
```

### 并行执行
```bash
# Phase 1 设计阶段（5个并行）
make -f docs/plan/v1.1.6/Makefile.tasks p0-1.1 p0-2.1 p0-3.1 p0-4.1 p0-5.1

# WASM 子任务（3个并行）
make -f docs/plan/v1.1.6/Makefile.tasks p0-1.2-3-4

# MemoryService 拆分（3个并行）
make -f docs/plan/v1.1.6/Makefile.tasks p1-6-memory-ops
```

### 查看进度
```bash
# 查看开发进度
make -f docs/plan/v1.1.6/Makefile.tasks progress

# 生成任务依赖图
make -f docs/plan/v1.1.6/Makefile.tasks tasks-chart
```

---

## 📝 任务模板

复制以下模板创建任务 Issue：

```markdown
## [P0-1.2] 实现 wasmtime 集成

**负责人**: @your-name
**优先级**: P0
**预计工作量**: 2 天
**依赖**: P0-1.1

### 任务描述
实现 WASM 沙箱的 wasmtime 集成，替换当前的自定义实现。

### 验收标准
- [ ] wasmtime 成功集成
- [ ] 所有现有测试通过
- [ ] 新增安全测试通过
- [ ] 代码审查通过

### 分支
- feature/P0-1.2

### 子任务
- [ ] 设计 wasmtime 集成方案
- [ ] 实现 wasmtime 主机
- [ ] 实现燃料限制
- [ ] 编写测试
- [ ] 更新文档

### 时间线
- 开始: 2026-02-15
- 预计完成: 2026-02-17
- 实际完成: _____

### 相关文档
- 设计: docs/plan/v1.1.6/TASK_BREAKDOWN.md#p0-1
- 审阅: docs/user/code-review-foundation-layer.md
```

---

## ✅ 提交检查清单

提交前确认：

- [ ] 代码通过 `cargo test`
- [ ] 代码通过 `cargo clippy`
- [ ] 代码通过 `cargo fmt --check`
- [ ] 添加了单元测试
- [ ] 更新了相关文档
- [ ] 提交信息符合规范

提交信息格式：

```
feat(P0-1.2): 实现 wasmtime 集成

- 实现 wasmtime 主机包装器
- 添加燃料限制支持
- 编写安全测试

Closes #123
```

---

## 🎯 团队协作建议

### 每日站会（5-10 分钟）

- 讨论进度和阻塞
- 识别需要协作的任务
- 调整并行策略

### 代码审查

- 每个任务至少 2 名审查者
- 审查清单：
  - [ ] 安全性
  - [ ] 性能
  - [ ] 错误处理
  - [ ] 测试覆盖

### 合并策略

- 使用 rebase + merge
- 保持主分支稳定
- 合并前运行完整测试套件

---

## 🔗 相关文档

- [任务拆分详情](TASK_BREAKDOWN.md)
- [解决方案](SOLUTION.md)
- [README](README.md)
- [代码审阅报告](../../user/code-review-summary.md)

---

**开始开发，无需等待确认！** 🚀
