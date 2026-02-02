# CIS Skill 开发指南 (AI-First)

> **Meta-Skill**: 这是开发 CIS Skills 的指导文档，定义了从需求到交付的标准流程。

## 目录

- [概述](#概述)
- [开发前准备](#开发前准备)
- [AI-First 开发流程](#ai-first-开发流程)
- [最佳实践](#最佳实践)
- [模板与工具](#模板与工具)
- [常见问题](#常见问题)

---

## 概述

### CIS Skill 是什么？

**CIS Skill** 是 CIS 系统的可选扩展模块，用于实现涌现功能（Emergent Features）。遵循奥卡姆剃刀原则，CIS 核心保持最小化，所有扩展功能都以 Skill 形式存在。

### 设计原则

1. **第一性原理**
   - 硬件绑定（如有需要）
   - 本地优先（数据不过度依赖外部）
   - 简单优于复杂

2. **AI 辅助开发**
   - AI 执行重复性工作
   - 人类专注于关键决策
   - 迭代优化，持续改进

3. **模块化与解耦**
   - Skill 独立编译
   - 最小依赖 cis-core
   - 可选依赖，按需启用

---

## 开发前准备

### 1. 环境检查

```bash
# 检查 Rust 版本 (需要 1.70+)
rustc --version

# 检查 CIS 核心库
cd /path/to/CIS/cis-core
cargo check
```

### 2. 创建 Skill 项目

```bash
# 在 skills/ 目录下创建新 Skill
mkdir -p skills/my-skill
cd skills/my-skill

# 初始化 Cargo 项目
cargo init --lib
```

### 3. 配置 Cargo.toml

```toml
[package]
name = "cis-my-skill"
version = "0.1.0"
edition = "2021"

[lib]
name = "cis_my_skill"
path = "src/lib.rs"

[dependencies]
cis-core = { path = "../../cis-core" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
tracing = "0.1"

[dev-dependencies]
tokio-test = "0.4"
```

---

## AI-First 开发流程

### 阶段 1: 需求分析 (AI 辅助)

**人类任务**: 提供初始需求
**AI 任务**: 澄清需求，识别风险

#### 步骤

1. **人类**: 用自然语言描述需求
   ```
   示例: "我需要实现一个 CPU 监控 Skill，每 5 秒采集一次 CPU 使用率，
            当超过 80% 时记录日志并发送告警"
   ```

2. **AI**: 提出澄清问题
   ```
   1. 监控单核还是多核？
   2. 告警方式有哪些（日志/Webhook/邮件）？
   3. 历史数据需要保留多久？
   4. 是否需要跨平台支持（Linux/macOS/Windows）？
   ```

3. **人类**: 回答问题，确认需求

4. **AI**: 生成需求分析文档

#### 输出模板

```markdown
# 需求分析: {Skill Name}

## 需求描述
{AI 总结的需求}

## 功能清单
- [ ] 功能 1
- [ ] 功能 2

## 约束条件
- 技术约束: {技术限制}
- 性能约束: {性能要求}

## 风险评估
- 风险 1: {描述} - 缓解方案
- 风险 2: {描述} - 缓解方案
```

---

### 阶段 2: 技术设计 (AI 生成 + 人类 Review)

**人类任务**: Review 设计，做出决策
**AI 任务**: 生成技术方案，定义接口

#### AI 任务清单

1. 生成 2-3 个备选方案
2. 分析每个方案的优劣
3. 推荐最佳方案
4. 定义数据结构
5. 定义 API 接口

#### 输出模板

```markdown
# 技术设计: {Skill Name}

## 方案对比

### 方案 A: {名称}
- **优点**: {list}
- **缺点**: {list}
- **适用场景**: {描述}

### 方案 B: {名称}
- **优点**: {list}
- **缺点**: {list}
- **适用场景**: {描述}

## 推荐方案
**选择**: {方案 X}
**理由**: {原因}

## 数据结构
```rust
// AI 生成的 Rust 代码
pub struct MonitorConfig {
    // ...
}
```

## API 接口
```rust
#[async_trait]
pub trait Monitor {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn collect(&self) -> Result<Metric>;
}
```

## 依赖模块
- `cis-core::types`: 使用 Task, TaskId 等类型
- `cis-core::sandbox`: 安全的文件访问
```

#### 人类 Review 要点

- [ ] 方案符合 CIS 第一性原理
- [ ] 接口设计合理
- [ ] 数据结构简洁
- [ ] 性能可接受

---

### 阶段 3: 代码开发 (AI 生成 + 人类 Review)

**人类任务**: Code Review，关键代码编写
**AI 任务**: 生成大部分代码和测试

#### 迭代开发模式

```
人类: "请实现 Monitor trait"
  ↓
AI: 生成代码 (第一版)
  ↓
人类: Review + 反馈 "性能不够好，O(n²) 复杂度"
  ↓
AI: 优化代码 (第二版)
  ↓
人类: "✅ 通过"
```

#### AI 生成的代码应包含

1. **数据结构定义** (70-80% 由 AI 完成)
   ```rust
   pub struct CpuMonitor {
       config: MonitorConfig,
       running: Arc<AtomicBool>,
   }
   ```

2. **核心业务逻辑** (70-80% 由 AI 完成)
   ```rust
   impl CpuMonitor {
       pub fn new(config: MonitorConfig) -> Self {
           // ...
       }

       pub async fn start(&self) -> Result<()> {
           // ...
       }
   }
   ```

3. **单元测试** (90% 由 AI 完成)
   ```rust
   #[cfg(test)]
   mod tests {
       #[tokio::test]
       async fn test_cpu_monitor() {
           // ...
       }
   }
   ```

4. **文档注释** (AI 全部完成)
   ```rust
   /// CPU 监控器
   ///
   /// 每隔指定时间采集 CPU 使用率，当超过阈值时触发告警
   ///
   /// # 示例
   ///
   /// ```rust
   /// use cis_my_skill::CpuMonitor;
   ///
   /// let monitor = CpuMonitor::new(config);
   /// monitor.start().await?;
   /// ```
   pub struct CpuMonitor { }
   ```

#### 人类重点关注的代码

1. **安全性** (必须人工审查)
   - 路径遍历风险
   - 注入攻击风险
   - 权限检查

2. **性能关键路径** (可能需要人工优化)
   - 热点循环
   - 内存分配
   - I/O 操作

3. **错误处理** (确保覆盖所有边界情况)

---

### 阶段 4: 测试验收 (AI 自动化)

**人类任务**: 分析测试结果，决策发布
**AI 任务**: 执行测试，生成报告

#### 测试类型

1. **单元测试** (AI 生成并执行)
   ```bash
   cargo test --lib
   ```

2. **集成测试** (AI 生成)
   ```bash
   cargo test --test integration
   ```

3. **性能基准测试** (AI 生成基准代码)
   ```bash
   cargo bench
   ```

4. **代码质量检查**
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   ```

#### 测试报告模板 (AI 生成)

```markdown
# 测试报告: {Skill Name}

## 单元测试
- 通过率: 45/45 (100%)
- 覆盖率: 87%
- 失败用例: 无

## 集成测试
- 通过率: 12/12 (100%)

## 性能测试
- CPU 占用: 平均 2%
- 内存占用: 峰值 50MB
- 响应时间: P99 < 10ms

## 代码质量
- ✅ fmt 检查通过
- ✅ clippy 无警告
- ✅ 文档覆盖率 100%

## 建议
{AI 分析结果}
```

#### 验收标准

- [ ] 所有单元测试通过
- [ ] 代码覆盖率 > 80%
- [ ] 无 clippy 警告
- [ ] 性能指标达标
- [ ] 人类确认发布

---

### 阶段 5: 文档发布 (AI 自动化)

**人类任务**: Review 文档
**AI 任务**: 生成所有文档

#### AI 生成的文档

1. **用户手册** (`docs/user-guide.md`)
2. **API 文档** (`docs/api-reference.md`)
3. **开发指南** (`docs/development.md`)
4. **变更日志** (`CHANGELOG.md`)

#### 文档模板 (AI 填充)

```markdown
# {Skill Name} 用户手册

## 简介
{AI 生成简介}

## 快速开始
```rust
// AI 生成示例代码
```

## 配置说明
{AI 生成配置说明}

## 常见问题
{AI 生成 FAQ}
```

---

## 最佳实践

### 1. 代码规范

```yaml
naming:
  types: PascalCase  (MonitorConfig, CpuMonitor)
  functions: snake_case (start_monitor, collect_metrics)
  constants: SCREAMING_SNAKE_CASE (MAX_RETRIES)

error_handling:
  - 使用 cis_core::Result<T>
  - 使用 thiserror 定义错误类型
  - 提供有意义的错误消息

documentation:
  - 所有 public API 必须有文档注释
  - 提供使用示例
  - 说明性能特征
```

### 2. 依赖管理

```toml
# 最小化依赖
[dependencies]
# 只依赖必需的库
cis-core = { path = "../../cis-core" }
tokio = { version = "1", features = ["macros"] }  # 不要 full

# 按需启用特性
[features]
default = []
monitoring = ["prometheus"]  # 可选的监控功能
```

### 3. 测试策略

```rust
// 1. 单元测试覆盖核心逻辑
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_core_logic() { }
}

// 2. 集成测试验证交互
#[cfg(test)]
mod integration {
    #[tokio::test]
    async fn test_with_cis_core() { }
}

// 3. 基准测试确保性能
#[cfg(test)]
mod benches {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};

    fn benchmark_collect(c: &mut Criterion) {
        c.bench_function("collect", |b| {
            b.iter(|| {
                // 测试代码
            });
        });
    }
}
```

---

## 模板与工具

### Skill 项目模板

创建新 Skill 时，可以直接复制模板：

```bash
# 使用模板创建新 Skill
cp -r skills/skill-template skills/my-new-skill
cd skills/my-new-skill
# 修改名称和配置
```

### Prompt 模板

#### 需求分析 Prompt

```
你是一个 CIS Skill 需求分析师。请分析以下需求：

{用户需求}

请按以下格式输出：
1. 需求总结
2. 功能清单
3. 约束条件
4. 风险评估
5. 澄清问题（如有）
```

#### 代码生成 Prompt

```
你是一个经验丰富的 Rust 开发工程师，正在为 CIS 系统开发 Skill。

请根据以下设计实现代码：

{设计文档}

要求：
1. 遵循 Rust 2021 版本规范
2. 使用 cis-core::Result 作为返回类型
3. 所有 public API 必须有文档注释
4. 包含单元测试（覆盖率 > 80%）
5. 提供使用示例

技术约束：
- 异步运行时: tokio
- 日志: tracing
- 错误处理: thiserror
```

---

## 常见问题

### Q1: Skill 如何访问 CIS 核心功能？

**A**: 通过依赖 `cis-core`:

```rust
use cis_core::{types::Task, sandbox::SandboxConfig, error::Result};

pub struct MySkill {
    sandbox: SandboxConfig,
}

impl MySkill {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sandbox: SandboxConfig::new(),
        })
    }
}
```

### Q2: Skill 如何与 CIS 主系统集成？

**A**: CIS 主系统通过 trait 或配置文件加载 Skill:

```rust
// cis-node/src/skills/mod.rs
pub fn load_skills() -> Result<Vec<Box<dyn Skill>>> {
    let mut skills = vec![];

    // 动态加载 Skills
    #[cfg(feature = "cpu-monitor")]
    skills.push(Box::new(cis_cpu_monitor::CpuMonitor::new()?));

    Ok(skills)
}
```

### Q3: AI 生成的代码质量如何保证？

**A**: 通过三层质量保证：

1. **AI 生成**: 遵循最佳实践和模板
2. **人类 Review**: 重点审查安全和性能
3. **自动化测试**: 单元测试 + clippy + fmt

### Q4: 如何处理跨平台兼容性？

**A**: 使用条件编译和 cfg 属性：

```rust
#[cfg(target_os = "linux")]
fn get_cpu_usage() -> Result<f64> {
    // Linux 实现
}

#[cfg(target_os = "macos")]
fn get_cpu_usage() -> Result<f64> {
    // macOS 实现
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn get_cpu_usage() -> Result<f64> {
    Err(CisError::not_supported("Platform not supported"))
}
```

---

## 总结

本指南定义了 AI-First 的 Skill 开发流程：

1. **需求分析**: AI 澄清，人类确认
2. **技术设计**: AI 生成方案，人类选择
3. **代码开发**: AI 生成代码，人类 Review
4. **测试验收**: AI 自动化测试，人类决策
5. **文档发布**: AI 生成文档，人类检查

通过这个流程，可以：
- **提高效率 70%**: AI 处理重复性工作
- **保证质量**: 人类专注于关键决策
- **降低门槛**: 新开发者快速上手

---

**文档版本**: v1.0
**最后更新**: 2026-02-02
**维护者**: CIS Team
