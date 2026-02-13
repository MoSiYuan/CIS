# CIS v1.1.6 下一步行动指南

> **更新日期**: 2026-02-12
> **状态**: 所有设计和计划已完成，准备启动并行开发
> **预计完成**: 6-8 周

---

## ✅ 已完成工作

### 1. 整合所有设计文档

已整合 v1.1.6 目录下所有设计文档，创建：

**主要文档**:
1. [V1.1.6_INTEGRATED_EXECUTION_PLAN.md](./V1.1.6_INTEGRATED_EXECUTION_PLAN.md) (~14,000 行)
   - 完整的执行计划
   - Agent Pool 架构设计
   - 10 个主要任务拆分
   - 7 个 Agent Teams 定义

2. [TASKS_DEFINITIONS.toml](./TASKS_DEFINITIONS.toml) (~1,500 行)
   - 所有任务的详细定义
   - 任务 prompt 和上下文
   - 依赖关系
   - 验收标准

3. [cis-v1.1.6-start-parallel.sh](../cis-v1.1.6-start-parallel.sh) (~300 行)
   - Agent Pool 启动脚本
   - 一键启动并行开发
   - 实时监控和日志

### 2. 核心问题识别和解决方案

**问题 A: CLI 架构偏差**
- 发现: CLI handler 直接实现业务逻辑
- 解决方案: V-1 CLI 架构修复任务
- 参考: CLI_GUIDE_OPTIMIZED.md

**问题 B: 巨型模块 (17 个 >600 行)**
- 发现: 最大 3,439 行
- 解决方案: V-2 到 V-7 拆分任务
- 参考: REFACTORING_EXECUTION_PLAN.md

**问题 C: 记忆全量向量索引**
- 发现: 所有记忆都向量化，导致检索失真
- 解决方案: V-4 memory 精准索引任务
- 参考: MEMORY_INDEX_PRECISION_DESIGN.md

---

## 🚀 立即行动（今天）

### 步骤 1: 审阅执行计划

**命令**:
```bash
cd /Users/jiangxiaolong/work/project/CIS
less docs/plan/v1.1.6/V1.1.6_INTEGRATED_EXECUTION_PLAN.md
```

**确认**:
- [ ] 任务拆分合理（10 个主要任务）
- [ ] Teams 定义正确（7 个 Teams）
- [ ] 优先级明确（P0 → P3）
- [ ] 依赖关系清晰
- [ ] 时间表可执行（6-8 周）

---

### 步骤 2: 测试启动脚本（DRY RUN）

**命令**:
```bash
cd /Users/jiangxiaolong/work/project/CIS
./cis-v1.1.6-start-parallel.sh --dry-run
```

**预期输出**:
```
======================================
CIS v1.1.6 并行开发启动
======================================

[INFO] 检查依赖...
[SUCCESS] 依赖检查完成
[INFO] 创建 Agent Pool: cis-v1.1.6-refactor
[WARNING] [DRY-RUN] 将创建 Pool: pool-xxxxxxxxx
[INFO] 定义 Agent Teams...
[WARNING] [DRY-RUN] 将添加 Team: Team-V-CLI
[WARNING] [DRY-RUN] 将添加 Team: Team-Q-Core
...
[INFO] 从 docs/plan/v1.1.6/TASKS_DEFINITIONS.toml 加载任务...
[SUCCESS] 任务加载完成: 总计 10 个任务
  - P0 (关键): 1
  - P1 (高): 3
  - P2 (中): 3
  - P3 (低): 3
[INFO] 分配任务到 Teams...
[WARNING] [DRY-RUN] 将分配 V-1 到 Team-V-CLI
...
[SUCCESS] 任务分配完成
[INFO] 启动事件总线 (端口: 7678)...
[WARNING] [DRY-RUN] 将启动事件总线: 端口 7678
[SUCCESS] 事件总线已启动
[INFO] 启动并行开发 (最大 7 个 Teams)...
[WARNING] [DRY-RUN] 将启动 7 个 Teams 并行执行
[WARNING] [DRY-RUN] 执行时间: 预计 6-8 周
[SUCCESS] 并行开发已启动
======================================
```

**确认**:
- [ ] 脚本可执行
- [ ] 依赖检查通过
- [ ] 所有 Teams 定义加载
- [ ] 所有任务加载
- [ ] 任务分配正确
- [ ] 无错误输出

---

### 步骤 3: 准备 Agent Pool 实现

**检查**:
```bash
# 检查是否已有 Agent Pool 实现
ls -la cis-core/src/agent/pool/
```

**如果不存在**，需要先实现基础框架：

**最小可行实现** (可选):
```bash
# 创建 agent pool 模块
mkdir -p cis-core/src/agent/pool

# 创建核心文件
touch cis-core/src/agent/pool/mod.rs
touch cis-core/src/agent/pool/pool.rs
touch cis-core/src/agent/pool/team.rs
touch cis-core/src/agent/pool/task.rs
```

**或者**：先使用手动方式（推荐）

---

## 📋 Week 1 执行计划

### Day 1-2: V-1 CLI 架构修复

**负责**: Team V (手动或 Agent)

**步骤**:
1. **审阅所有 CLI handler**
   ```bash
   find cis-node/src/cli/handlers -name "*.rs" -type f
   ```

2. **识别问题代码**（查找直接使用 std::fs, std::process）
   ```bash
   grep -r "std::fs" cis-node/src/cli/handlers/
   grep -r "std::process" cis-node/src/cli/handlers/
   ```

3. **重构为 Server API 调用**
   - 参考: CLI_GUIDE_OPTIMIZED.md 中的正确模式
   - 示例:
     ```rust
     // ❌ 错误
     fs::create_dir_all(&cis_dir)?;

     // ✅ 正确
     let request = InitProjectRequest { path, name, force };
     let response = ctx.server.handle(Box::new(request)).await?;
     ```

4. **更新文档和测试**
   - 更新 CLI_GUIDE_OPTIMIZED.md
   - 编写单元测试
   - 运行 `cargo test --package cis-node --test cli`

**验收**:
- [ ] 无 `std::fs` 直接使用（除了测试代码）
- [ ] 所有 handler 调用 `ctx.server.handle()`
- [ ] 测试覆盖率 > 80%

---

### Day 3-5: V-2 scheduler 设计

**负责**: Team Q (手动或 Agent)

**步骤**:
1. **阅读现有代码**
   ```bash
   less cis-core/src/scheduler/mod.rs
   # 3,439 行，需要拆分
   ```

2. **设计拆分方案**
   - 参考: REFACTORING_EXECUTION_PLAN.md 中的设计
   - 输出: scheduler 拆分设计文档

3. **创建目录结构**
   ```bash
   mkdir -p cis-core/src/scheduler/core
   mkdir -p cis-core/src/scheduler/execution
   mkdir -p cis-core/src/scheduler/events
   mkdir -p cis-core/src/scheduler/persistence
   mkdir -p cis-core/src/scheduler/notification
   ```

4. **编写设计文档** (~1 天)
   - 每个 sub-module 的职责
   - 接口定义
   - 数据流图

**验收**:
- [ ] 设计文档完整
- [ ] 每个模块职责清晰
- [ ] 接口定义明确
- [ ] 与现有代码兼容

---

## 🎯 关键里程碑

### Week 1 结束

**预期成果**:
- ✅ V-1 完成（CLI 架构修复）
- ✅ V-2 设计完成（scheduler 拆分方案）
- ✅ Agent Pool 稳定运行（如已实现）

**检查点**:
```bash
# CLI 架构检查
grep -r "std::fs" cis-node/src/cli/handlers/ | wc -l  # 应该 0

# scheduler 设计检查
ls -la cis-core/src/scheduler/*/  # 应该有 5 个子目录
```

---

### Week 2-3 结束

**预期成果**:
- ✅ V-2 完成（scheduler 拆分实现）
- ✅ V-3 完成（config 拆分）
- ✅ V-4 启动（memory 精准索引）

**检查点**:
```bash
# 模块行数检查
tokei cis-core/src/scheduler/
# 应该看到每个模块 <500 行

tokei cis-core/src/config/
# 应该看到拆分后的子模块
```

---

### Week 4-5 结束

**预期成果**:
- ✅ V-4 完成（memory 精准索引）
- ✅ V-5 完成（skill 拆分）
- ✅ V-6 完成（p2p 拆分）
- ✅ V-7 完成（agent/其他拆分）

**检查点**:
```bash
# 整体代码质量检查
cargo tokei --exclude tests  # 平均行数应该降低
cargo geiger  # 圈复杂度应该降低
```

---

### Week 6-8 结束

**预期成果**:
- ✅ V-8 完成（全面测试）
- ✅ V-9 完成（性能优化）
- ✅ V-10 完成（文档更新）
- ✅ v1.1.6 发布

**检查点**:
```bash
# 测试覆盖率
cargo tarpaulin --exclude-files "tests/*"
# 应该 >85%

# 编译时间
hyperfine "cargo build --release"
# 增量编译应该 <60s

# 文档完整性
grep -r "TODO\|FIXME\|XXX" docs/ | wc -l
# 应该很少或 0
```

---

## 🔧 监控和调试

### 实时监控命令

```bash
# 查看 Agent Pool 状态
cis agent pool status <pool-id>

# 查看所有 Teams
cis agent pool list-teams <pool-id>

# 查看任务队列
cis agent pool list-tasks <pool-id>

# 实时查看日志
cis agent pool logs <pool-id> --follow

# 查看性能指标
cis agent pool metrics <pool-id>
```

### 系统监控

```bash
# CPU/内存监控
htop

# 磁盘 I/O
iotop

# 网络流量（如果使用 P2P）
nethogs

# Git 状态
git status
git log --oneline -10
```

---

## 📞 支持和问题排查

### 常见问题

**Q1: Agent Pool 命令不存在**
```bash
# 解决方案：使用手动模式
# 1. 手动创建任务列表
# 2. 使用现有 Agent（Claude Code）直接执行
# 3. 定期更新进度到文档
```

**Q2: 内存不足**
```bash
# 解决方案：限制并发
export CARGO_BUILD_JOBS=4  # 限制编译并发
export RAYON_NUM_THREADS=4   # 限制运行时并发
```

**Q3: 编译超时**
```bash
# 解决方案：增量编译
cargo check --package cis-core    # 快速检查
cargo build --package cis-core   # 只构建一个 crate
```

**Q4: 测试失败**
```bash
# 解决方案：隔离测试
cargo test --package cis-core --lib  # 只测试库
cargo test --test <test_name>        # 只运行一个测试
```

### 获取帮助

**文档支持**:
- [CLAUDE.md](../../CLAUDE.md) - 主引导文档
- [V1.1.6_INTEGRATED_EXECUTION_PLAN.md](./V1.1.6_INTEGRATED_EXECUTION_PLAN.md) - 完整计划
- [TASKS_DEFINITIONS.toml](./TASKS_DEFINITIONS.toml) - 任务定义

**命令支持**:
```bash
# 查看帮助
cis --help

# 查看版本
cis --version

# 查看配置
cis config show
```

---

## 📊 进度跟踪

### 每日检查点

**每日结束时**，更新进度：

```bash
# 1. 提交代码
git add .
git commit -m "progress: V-1 CLI 架构修复 (Day 1)

- 审阅 50% CLI handlers
- 重构 10 个 handler 为 Server API 调用
- 测试覆盖率: 70%

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
"

# 2. 更新进度文档
echo "$(date '+%Y-%m-%d'): V-1 进度 50%" >> docs/plan/v1.1.6/PROGRESS_LOG.txt

# 3. 推送到远程（可选）
git push
```

### 每周里程碑

**每周结束时**，创建周报告：

```bash
./cis-v1.1.6-weekly-report.sh \
    --week-number 1 \
    --tasks-completed "V-1,V-2-start" \
    --tasks-in-progress "V-2" \
    --metrics '{"coverage": 82, "build_time": 90}'
```

---

## ✅ 启动清单

在正式启动并行开发前，确认以下事项：

### 环境准备

- [ ] Rust 工具链最新版 (`rustup update`)
- [ ] Cargo 缓存清理 (`cargo clean`)
- [ ] Git 工作目录干净 (`git status`)
- [ ] 磁盘空间充足 (>10GB 可用)

### 代码准备

- [ ] 主分支最新 (`git pull origin main`)
- [ ] 创建 feature 分支 (`git checkout -b feature/v1.1.6-refactor`)
- [ ] 所有设计文档已审阅
- [ ] 任务定义已确认

### 工具准备

- [ ] IDE 配置正确（VSCode/IntelliJ）
- [ ] 代码格式化工具配置 (`rustfmt`)
- [ ] Lint 工具配置 (`clippy`)
- [ ] 监控工具就绪 (`htop`, `hyperfine`)

### 团队准备

- [ ] 所有相关人员已通知
- [ ] 期望和沟通机制已建立
- [ ] 问题升级路径已明确
- [ ] 成功标准已对齐

---

## 🚀 启动命令

### 方式 1: 使用启动脚本（推荐）

```bash
cd /Users/jiangxiaolong/work/project/CIS
./cis-v1.1.6-start-parallel.sh
```

### 方式 2: 手动启动（如脚本不可用）

```bash
# 1. 创建 Pool ID
POOL_ID="pool-$(date +%s)"

# 2. 定义第一个 Team
TEAM_V="team-v-cli-$(date +%s)"

# 3. 分配第一个任务
TASK_V1="V-1-$(date +%s)"

# 4. 启动（使用现有 Agent）
# 例如：直接在 Claude Code 中开始工作
```

---

## 📈 成功标准总结

### Phase 1 完成（Week 1）

- ✅ CLI 架构修复完成
- ✅ scheduler 拆分设计完成
- ✅ Agent Pool 运行（手动或自动）

### Phase 2 完成（Week 2-3）

- ✅ scheduler 拆分完成
- ✅ config 拆分完成
- ✅ memory 精准索引启动

### Phase 3 完成（Week 4-5）

- ✅ memory 精准索引完成
- ✅ skill 拆分完成
- ✅ p2p 拆分完成
- ✅ agent/其他拆分完成

### Phase 4 完成（Week 6-8）

- ✅ 全面测试完成
- ✅ 性能优化完成
- ✅ 文档更新完成
- ✅ v1.1.6 发布

---

## 🎯 立即执行

**当前状态**: 所有准备完成，可以启动

**下一步**:
```bash
cd /Users/jiangxiaolong/work/project/CIS
./cis-v1.1.6-start-parallel.sh --dry-run  # 先测试
./cis-v1.1.6-start-parallel.sh           # 正式启动
```

**或者手动开始第一个任务**:
```bash
# 开始 V-1: CLI 架构修复
cd cis-node/src/cli/handlers
# 按照任务定义中的 prompt 开始工作
```

---

**文档版本**: 1.0
**行动指南完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**状态**: ✅ 就绪

**预计开始时间**: 现在
**预计完成时间**: 6-8 周后
