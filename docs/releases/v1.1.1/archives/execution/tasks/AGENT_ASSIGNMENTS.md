# AI Agent 任务分配 - Week 1 并行执行

**启动时间**: 立即  
**并行度**: 6 个 Agent 同时执行  
**目标**: Phase 1 (稳定性加固) Week 1 完成 80%

---

## 🚀 立即执行任务

### Agent-A: 内存安全修复 (P1-1)

```bash
# 1. 阅读任务文档
cat plan/tasks/phase1/P1-1_memory_safety.md

# 2. 创建分支
git checkout -b feat/phase1-p1-1-memory-safety

# 3. 开始执行
# 任务位置: cis-core/src/memory/service.rs
# 任务位置: cis-core/src/storage/db.rs

# 4. 完成后提交
git add .
git commit -m "fix(phase1): P1-1 修复内存安全问题

- 修复 test_memory_service_delete 竞态条件
- 修复 test_core_db 测试隔离问题
- 添加 Arc<Mutex> 保护

fixes #task-P1-1"
```

**关键文件**:
- `cis-core/src/memory/service.rs` (第 ~450 行删除逻辑)
- `cis-core/src/storage/db.rs` (测试数据库创建)

**验收命令**:
```bash
cargo test -p cis-core --lib memory::service::tests::test_memory_service_delete
cargo test -p cis-core --lib storage::db::tests::test_core_db
```

---

### Agent-B: WebSocket 测试修复 (P1-2)

```bash
# 1. 阅读任务文档
cat plan/tasks/phase1/P1-2_websocket_tests.md

# 2. 创建分支
git checkout -b feat/phase1-p1-2-websocket-tests

# 3. 开始执行
# 任务位置: cis-core/src/matrix/websocket/server.rs

# 4. 完成后提交
git commit -m "fix(phase1): P1-2 修复 WebSocket 测试

- 修复 test_sync_response_handling 超时问题
- 修复 test_sync_request_handling 端口冲突
- 添加 portpicker 动态端口分配

fixes #task-P1-2"
```

**关键文件**:
- `cis-core/src/matrix/websocket/server.rs`
- `cis-core/src/matrix/websocket/tests.rs`

**新增依赖**:
```toml
[dev-dependencies]
portpicker = "0.1"
```

**验收命令**:
```bash
cargo test -p cis-core --lib matrix::websocket::server::tests
```

---

### Agent-C: 项目注册表测试修复 (P1-3)

```bash
# 1. 创建分支
git checkout -b feat/phase1-p1-3-project-registry

# 2. 开始执行
# 任务位置: cis-core/src/skill/project_registry.rs

# 3. 修复内容
# - 使用 tempfile::TempDir 替代手动清理
# - 修复 test_project_skill_config

# 4. 完成后提交
git commit -m "fix(phase1): P1-3 修复项目注册表测试

- 使用 tempfile::TempDir RAII 模式
- 修复 test_project_skill_config 目录清理问题

fixes #task-P1-3"
```

**关键文件**:
- `cis-core/src/skill/project_registry.rs`

**验收命令**:
```bash
cargo test -p cis-core --lib skill::project_registry::tests
```

---

### Agent-D: CI/CD 流水线强化 (P1-5)

```bash
# 1. 创建分支
git checkout -b feat/phase1-p1-5-ci-cd

# 2. 开始执行
# 任务位置: .github/workflows/

# 3. 需要添加的 workflow:
# - 覆盖率检查 (cargo-tarpaulin)
# - 跨平台构建 (macOS, Linux)
# - 性能基准测试
# - 安全扫描 (cargo-audit)

# 4. 完成后提交
git commit -m "feat(phase1): P1-5 强化 CI/CD 流水线

- 添加测试覆盖率检查 (>80%)
- 添加跨平台构建 (macOS, Linux)
- 添加性能基准测试
- 添加安全扫描 (cargo-audit)

fixes #task-P1-5"
```

**关键文件**:
- `.github/workflows/enhanced-ci.yml` (新建)
- `.github/workflows/coverage.yml` (新建)

**参考配置**:
```yaml
# .github/workflows/enhanced-ci.yml
name: Enhanced CI
on: [push, pull_request]

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/tarpaulin@v0.1
        with:
          args: '--timeout 300 --out Html'
      - name: Upload coverage
        uses: actions/upload-artifact@v3
        with:
          name: coverage-report
          path: tarpaulin-report.html
```

---

### Agent-E: 编译警告清理 (P1-6)

```bash
# 1. 创建分支
git checkout -b feat/phase1-p1-6-clippy-warnings

# 2. 自动修复
cargo fix --all --allow-dirty

# 3. 手动检查剩余警告
cargo clippy -p cis-core 2>&1 | grep "warning:" | head -20

# 4. 手动修复复杂警告
# - 未使用的导入
# - 复杂的类型转换
# - 可能的性能问题

# 5. 启用严格模式
echo '#![deny(warnings)]' >> cis-core/src/lib.rs

# 6. 完成后提交
git commit -m "refactor(phase1): P1-6 清理编译警告

- 自动修复 cargo fix
- 手动修复复杂警告
- 启用 #![deny(warnings)]

fixes #task-P1-6"
```

**验收命令**:
```bash
cargo build -p cis-core 2>&1 | grep "warning:" | wc -l
# 预期: 0
```

---

### Agent-F: 文档测试 (P1-7)

```bash
# 1. 创建分支
git checkout -b feat/phase1-p1-7-doc-tests

# 2. 检查当前 DocTest 状态
cargo test --doc -p cis-core

# 3. 为公共 API 添加示例代码
# 文件: cis-core/src/lib.rs
# 文件: cis-core/src/types.rs
# 文件: cis-core/src/skill/mod.rs

# 4. 示例格式
# ```rust
# /// 示例
# /// ```
# /// use cis_core::Skill;
# /// let skill = Skill::new("test");
# /// ```
# ```

# 5. 完成后提交
git commit -m "docs(phase1): P1-7 添加文档测试

- 为核心 API 添加 DocTest 示例
- 验证文档代码可运行
- 添加 DocTest 到 CI

fixes #task-P1-7"
```

**验收命令**:
```bash
cargo test --doc -p cis-core
```

---

## 📊 执行时间表

| 时间 | Agent-A | Agent-B | Agent-C | Agent-D | Agent-E | Agent-F |
|------|---------|---------|---------|---------|---------|---------|
| Day 1 | 阅读文档<br>开始修复 | 阅读文档<br>开始修复 | 阅读文档<br>开始修复 | 阅读文档<br>配置CI | 阅读文档<br>运行fix | 阅读文档<br>检查DocTest |
| Day 2 | 修复P1-1.1 | 修复P1-2.1 | 修复P1-3.1 | 配置覆盖率 | 手动修复警告 | 添加示例代码 |
| Day 3 | 修复P1-1.2<br>测试 | 修复P1-2.2<br>添加重连测试 | 测试 | 配置跨平台 | 启用deny(warnings) | 添加更多示例 |
| Day 4 | 修复P1-1.3<br>提交 | 测试<br>提交 | 提交 | 配置安全扫描<br>提交 | 测试<br>提交 | 测试<br>提交 |

---

## 🔄 每日同步

每个 Agent 每天结束时更新进度：

```bash
# 在 project root 创建进度文件
echo "## Agent-A Day 1 进度

已完成:
- [x] 阅读 P1-1 任务文档
- [x] 分析 test_memory_service_delete 失败原因
- [x] 添加 Arc<Mutex> 保护 (50%)

进行中:
- [ ] P1-1.1 修复完成并测试

阻塞:
- 无

明日计划:
- 完成 P1-1.1 和 P1-1.2" > plan/tasks/progress/agent-a-day1.md
```

---

## ⚠️ 冲突避免

### 文件冲突检查

执行前检查是否有其他 Agent 在修改同一文件：

```bash
# 检查文件修改状态
git status

# 检查分支
git branch -a | grep feat/phase1

# 如果发现冲突风险，在群里协调
```

### 冲突文件提示

| 文件 | 可能冲突 Agent | 建议 |
|------|---------------|------|
| `Cargo.toml` | 多个 | Agent-D (CI) 最后修改 |
| `cis-core/src/lib.rs` | Agent-E | 单独修改，最后合并 |
| `Cargo.lock` | 多个 | 不要手动修改，CI自动生成 |

---

## ✅ 完成标准

每个 Agent 完成时必须：

```markdown
- [ ] 代码编译通过: `cargo build -p cis-core`
- [ ] 测试通过: `cargo test -p cis-core --lib`
- [ ] 无新警告: `cargo clippy -p cis-core`
- [ ] 提交到分支: `git push origin feat/phase1-p1-X-xxx`
- [ ] 更新任务文档: 勾选完成的任务
- [ ] 创建 PR (可选，可在Week 2统一合并)
```

---

## 📞 紧急联系

如有阻塞问题：
1. 在任务文档中添加 `## 阻塞` 部分
2. 记录已尝试的解决方案
3. 请求 Lead Agent 协助

---

**开始执行吧！选择你的任务立即开始。** 🚀
