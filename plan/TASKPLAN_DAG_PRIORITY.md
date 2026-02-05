# CIS 工程修复计划：DAG 优先

**制定日期**: 2026-02-02  
**版本**: 1.0  
**原则**: 聚焦核心，消除过度设计，快速可用

---

## 🔍 过度设计排查结果

### 发现的设计过度点

| # | 问题 | 严重程度 | 建议 |
|---|------|----------|------|
| 1 | **四级决策过于复杂** | 🟡 中 | 当前 GUI 实现了完整四级，但 CLI 只实现了 Mechanical。建议先只保留 Mechanical 和 Confirmed 两级 |
| 2 | **GUI 功能过度** | 🟡 中 | decision_panel.rs (650+ 行) 实现了完整界面，但无后端。建议简化，先让基础功能可用 |
| 3 | **类型定义过度拆分** | 🟢 低 | Task、SkillTask、DagTaskDefinition 有重叠。建议统一 |
| 4 | **WASM 预留过度** | 🟢 低 | WASM Skill 框架完整但无实际执行。建议先专注 Native/Dag Skill |
| 5 | **IM 功能过早** | 🟡 中 | IM 命令占用了开发资源但实现度仅 20%。建议延后 |

### 设计合理的部分 ✅

- DAG 调度器核心设计
- Skill 作为 DAG 节点的理念
- 债务机制（Ignorable/Blocking）
- 网络层（DID/ACL）架构

---

## 🎯 核心原则：最小可用产品 (MVP)

**DAG 优先策略**：
1. **Skill = DAG 节点**（单一定位）
2. **Mechanical 级别优先**（自动执行）
3. **CLI 优先**（GUI 延后）
4. **Native Skill 优先**（WASM 延后）

---

## 📋 Task Plan

### Phase 1: DAG 核心修复（1 周）

**目标**: 让 `cis skill do "..."` 真正能执行 Skill

#### Task 1.1: 统一类型定义（2h）
```
优先级: 🔴 P0
文件: cis-core/src/types.rs
依赖: 无

行动:
- 删除 scheduler/skill_executor.rs 中的重复定义
- 统一使用 types.rs 中的 SkillExecutionResult
- 统一使用 skill/manifest.rs 中的 DagDefinition
- 添加类型转换测试

验收:
- cargo check 无类型重复警告
- 所有测试通过
```

#### Task 1.2: 修复 Skill 执行链（1 天）
```
优先级: 🔴 P0
文件: 
- cis-core/src/skill/router.rs
- cis-core/src/scheduler/skill_executor.rs
- cis-node/src/commands/skill.rs
依赖: Task 1.1

行动:
1. SkillRouter.route_by_intent() 返回匹配的技能链
2. 调用 SkillDagExecutor.execute_dag_skill()
3. 对于每个 SkillTask:
   - 如果是 Native: 执行二进制
   - 如果是 Dag: 递归执行
4. 返回结果给 CLI

简化方案（去除过度设计）:
- 暂不支持 WASM（标记 todo!）
- 暂只支持 Mechanical 级别（自动执行）
- 暂不支持热修改

验收:
- cis skill do "分析代码" 能真正执行匹配的 skill
- 输出结果到终端
```

#### Task 1.3: DAG 持久化基础（4h）
```
优先级: 🔴 P0
文件: cis-core/src/scheduler/mod.rs
依赖: 无

行动:
- 添加 DagRun 的序列化/反序列化
- 保存运行状态到 SQLite
- 重启后能恢复运行状态

简化方案:
- 只保存状态，不保存中间结果
- 恢复后从上次位置继续

验收:
- cis dag run 后，重启程序，cis dag list 能看到之前的运行
```

---

### Phase 2: 安全与质量（2-3 天）

**目标**: 修复安全红线，提升代码质量

#### Task 2.1: 修复记忆加密（4h）
```
优先级: 🔴 P0
文件: cis-core/src/memory/encryption.rs
依赖: 无

行动:
- 引入 chacha20poly1305 crate
- 替换 XOR 实现
- 添加 HKDF 密钥派生

参考代码:
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce
};

验收:
- 加密/解密单元测试通过
- 旧数据迁移方案（或清除）
```

#### Task 2.2: 清理编译警告（2h）
```
优先级: 🟡 P1
命令: cargo fix --all && cargo clippy --fix
依赖: 无

行动:
- 自动修复未使用导入
- 手动修复复杂警告
- 保留 ~20 个以内可接受的警告

验收:
- cargo check --all 警告 < 20 个
```

#### Task 2.3: 替换同步锁（4h）
```
优先级: 🟡 P1
文件: 所有使用 std::sync::Mutex 的异步代码
依赖: 无

行动:
- memory/service.rs: Arc<Mutex<MemoryDb>>
- 其他类似位置

替换为:
use tokio::sync::Mutex;

验收:
- 编译通过
- 运行时无死锁
```

---

### Phase 3: DAG 增强（1 周）

**目标**: 完善 DAG 执行，支持债务和回滚

#### Task 3.1: 实现债务机制（1 天）
```
优先级: 🟡 P1
文件: cis-core/src/scheduler/mod.rs
依赖: Phase 1

行动:
- Ignorable Debt: 标记失败但继续下游
- Blocking Debt: 冻结 DAG，记录到数据库
- cis debt list/resolve 命令连接后端

简化方案:
- CLI 先实现，GUI 延后
- 债务只保存到 SQLite，不广播

验收:
- 任务失败时可以选择 Ignorable/Blocking
- cis debt list 显示累积债务
```

#### Task 3.2: 实现回滚（1 天）
```
优先级: 🟡 P1
文件: 
- cis-core/src/scheduler/mod.rs
- cis-core/src/types.rs
依赖: Task 3.1

行动:
- Task.rollback 字段已存在
- DagRun 失败时执行回滚命令
- 按依赖逆序回滚

简化方案:
- 只支持命令回滚（不支持自动数据恢复）

验收:
- 任务失败时自动执行 rollback 命令
- 回滚失败不影响错误报告
```

#### Task 3.3: DAG 配置文件（4h）
```
优先级: 🟢 P2
文件: cis-core/src/skill/manifest.rs
依赖: 无

行动:
- 支持从 skill.toml 加载 DAG 定义
- 验证 DAG 无环
- 转换为 TaskDag

验收:
- skill.toml 可以定义 DAG
- cis skill load 能加载 DAG skill
```

---

### Phase 4: 最小可用 GUI（可选，1 周）

**目标**: 让 GUI 显示真实数据（非演示）

#### Task 4.1: CLI ↔ GUI 通信（3 天）
```
优先级: 🟢 P2（如需要 GUI）
方案选择:
A. Unix Socket（推荐，轻量）
B. HTTP API（如需要远程访问）
C. 共享 SQLite（最简单）

推荐方案 C（共享 SQLite）:
- GUI 直接读取 CIS 数据库
- CLI 写入数据库
- 通过文件锁或 SQLite WAL 模式同步

文件:
- cis-gui/src/app.rs 连接数据库
- 替换演示数据为真实查询

验收:
- GUI 显示真实的节点列表
- GUI 显示真实的 DAG 运行状态
```

#### Task 4.2: 简化决策面板（2 天）
```
优先级: 🟢 P2
文件: cis-gui/src/decision_panel.rs
依赖: Task 4.1

行动:
- 移除 Recommended/Arbitrated 级别（过度设计）
- 只保留 Mechanical（自动）和 Confirmed（弹窗）
- 连接后端状态

验收:
- Confirmed 级别任务显示模态弹窗
- 用户确认后继续执行
```

---

## 🚫 延后项（Phase 5+）

| 功能 | 原因 | 延后到 |
|------|------|--------|
| WASM Skill 执行 | 复杂度高，Native 足够先用 | Phase 5 |
| IM 完整功能 | 占用资源多，非核心 | Phase 5 |
| P2P 完整组网 | 网络层已基础可用 | Phase 4 |
| 热修改 (amend) | 风险高，可先重启 | Phase 5 |
| 远程 Agent 会话 | GUI 可用后再做 | Phase 5 |
| 四级决策完整版 | Mechanical + Confirmed 够用 | Phase 4 |

---

## 📊 时间估算

| Phase | 时间 | 产出 |
|-------|------|------|
| Phase 1: DAG 核心 | 1 周 | Skill 能真正执行 |
| Phase 2: 安全质量 | 2-3 天 | 加密修复，警告清理 |
| Phase 3: DAG 增强 | 1 周 | 债务、回滚、配置 |
| Phase 4: GUI 基础 | 1 周（可选）| GUI 显示真实数据 |
| **总计** | **2.5-3.5 周** | **最小可用产品** |

---

## ✅ 验收标准

### MVP 完成标准

1. **Skill 执行**: `cis skill do "分析代码"` 能匹配并执行 Skill
2. **DAG 执行**: `cis dag run my-dag.toml` 能按依赖执行多个 Skill
3. **债务处理**: 失败任务可选择继续或阻塞，能查看债务列表
4. **安全保障**: 记忆使用 ChaCha20-Poly1305 加密，无 XOR
5. **代码质量**: 编译警告 < 20，无同步锁在异步代码中

### 使用场景验证

```bash
# 场景 1: 执行单个 Skill
cis skill do "生成代码审查报告"
# 预期: 找到 code-review skill，执行，输出报告路径

# 场景 2: 执行 DAG Skill
cis skill do "完整代码审查流程"
# 预期: 执行 git-diff → ai-analyze → report-gen 链

# 场景 3: 处理失败
cis skill do "部署到生产环境"
# 如果失败，提示: [1] 标记为债务继续 [2] 阻塞等待修复

# 场景 4: 查看债务
cis debt list
# 预期: 显示累积的 Ignorable/Blocking 债务
```

---

## 🎯 关键决策

### 保留的设计 ✅
- DAG 作为 Skill 执行引擎
- Task 关联 Skill（skill_id）
- 债务机制（Ignorable/Blocking）
- CLI 优先，GUI 辅助

### 简化的设计 🟡
- 四级决策 → 两级（Mechanical + Confirmed）
- GUI 功能 → 只读展示 + 简单确认
- WASM → 延后
- 热修改 → 延后

### 移除的过度设计 ❌
- Arbitrated 级别（暂时）
- Recommended 倒计时（暂时）
- IM 功能（延后）
- 复杂的 GUI 工作流（简化）

---

**结论**: 聚焦 DAG 核心，2-3 周可完成最小可用产品。
