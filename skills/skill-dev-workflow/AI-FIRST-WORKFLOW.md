# AI-First Skill 开发工作流程

## 设计理念

CIS 是一个 AI Agent 系统，其 Skill 开发流程应该**充分利用 AI 能力**，实现"AI 执行、人类决策"的高效协作模式。

### 核心原则

1. **AI 执行，人类决策** - AI 处理重复性工作，人类专注于关键决策
2. **迭代优化** - AI 持续优化，人类 Review
3. **多模态理解** - AI 理解代码、文档、架构图
4. **上下文感知** - AI 理解 CIS 架构约束

## AI-First 六阶段工作流

### 阶段 1: AI 辅助需求分析

**人类角色**: 提供初始需求，回答 AI 问题
**AI 角色**: 澄清模糊需求，识别风险

```yaml
workflow:
  human:
    - 提供自然语言需求描述
    - 回答 AI 的澄清问题
    - 确认需求理解

  ai:
    - 分析需求语义
    - 识别模糊点和矛盾
    - 提出澄清问题 (What/Why/How)
    - 生成 FAQ

  artifacts:
    - 需求分析报告 (AI 生成，人类确认)
    - 问题清单 (AI 提问，人类回答)
    - 风险评估 (AI 分析)

  example:
    user: "我需要一个监控 Skill"
    ai: |
      1. 监控什么？(CPU/内存/磁盘/网络?)
      2. 监控频率？(实时/定时?)
      3. 告警方式？(日志/Webhook/邮件?)
      4. 数据存储？(本地/远程?)
```

**验收标准**:
- [ ] AI 理解需求正确 (人类确认)
- [ ] 无关键歧义
- [ ] 技术可行性已评估

---

### 阶段 2: AI 自动化文档生成

**人类角色**: Review 和修订
**AI 角色**: 自动生成结构化文档

```yaml
workflow:
  input:
    - 阶段 1 的需求分析
    - 现有架构文档
    - 代码库

  ai_tasks:
    - 提取需求关键点
    - 生成技术规格 (API 定义、数据结构)
    - 生成接口文档 (OpenAPI/Protobuf)
    - 生成架构图 (Mermaid)

  human_tasks:
    - Review AI 生成的文档
    - 修订不准确的部分
    - 补充遗漏的细节

  artifacts:
    - requirements/spec.md (AI 生成 80%, 人类补充 20%)
    - design/api.md (AI 生成)
    - design/data-model.md (AI 生成)

  example:
    ai_generated: |
      # API Specification

      ## 1. Monitor trait
      ```rust
      #[async_trait]
      pub trait Monitor {
          async fn collect(&self) -> Result<Metric>;
          async fn start(&self) -> Result<()>;
          async fn stop(&self) -> Result<()>;
      }
      ```

      ## 2. Data structures
      ```rust
      pub struct Metric {
          pub name: String,
          pub value: f64,
          pub timestamp: DateTime<Utc>,
      }
      ```
```

**验收标准**:
- [ ] 文档结构完整
- [ ] API 定义合理 (人类 Review)
- [ ] 数据模型设计合理

---

### 阶段 3: AI 生成设计方案

**人类角色**: 选择方案，权衡取舍
**AI 角色**: 生成多个方案，分析优劣

```yaml
workflow:
  ai_tasks:
    - 分析需求约束
    - 生成 2-3 个备选方案
    - 分析每个方案的优劣
    - 推荐最佳方案

  human_tasks:
    - Review AI 生成的方案
    - 选择方案或提出修改
    - 确认最终设计

  artifacts:
    - design/scheme-A.md (AI 生成)
    - design/scheme-B.md (AI 生成)
    - design/decision.md (人类选择)

  example:
    ai_analysis: |
      ## 方案对比

      ### 方案 A: 轮询模式
      优点: 实现简单，资源占用低
      缺点: 实时性差，可能漏掉峰值

      ### 方案 B: 事件驱动
      优点: 实时性高，资源效率好
      缺点: 实现复杂，依赖系统事件

      ## 推荐: 方案 B (优先性能)
      理由: CIS 是高性能系统，应优先保证实时性
```

**验收标准**:
- [ ] 生成 2+ 个方案
- [ ] 优劣分析清晰
- [ ] 人类确认最终方案

---

### 阶段 4: AI 辅助代码生成

**人类角色**: Code Review，关键代码编写
**AI 角色**: 生成大部分代码，编写测试

```yaml
workflow:
  ai_tasks:
    - 生成数据结构代码
    - 生成核心业务逻辑
    - 生成单元测试 (覆盖率 > 80%)
    - 生成文档注释

  human_tasks:
    - Code Review (重点: 安全、性能)
    - 编写关键算法 (如有)
    - 修复 Bug

  iteration:
    - 人类提出反馈
    - AI 迭代优化
    - 重复直到满意

  artifacts:
    - src/ (AI 生成 70-80%, 人类 20-30%)
    - tests/ (AI 生成 90%)
    - src/lib.rs (AI 生成文档注释)

  example:
    interaction: |
      人类: "性能不够好，O(n²) 复杂度"
      AI: "已优化为 O(n) 使用 HashMap，请 Review"
      人类: "✅ 通过"

    code_pattern: |
      // AI 生成的代码
      pub struct MetricsCollector {
          metrics: HashMap<String, Metric>,
          last_update: DateTime<Utc>,
      }

      impl MetricsCollector {
          pub fn new() -> Self {
              Self {
                  metrics: HashMap::new(),
                  last_update: Utc::now(),
              }
          }
      }
```

**验收标准**:
- [ ] 所有测试通过
- [ ] 代码覆盖 > 80%
- [ ] 人类 Code Review 通过
- [ ] 性能达标

---

### 阶段 5: AI 自动化测试

**人类角色**: 分析测试结果，决策是否发布
**AI 角色**: 执行测试，生成报告

```yaml
workflow:
  ai_tasks:
    - 运行单元测试
    - 运行集成测试
    - 执行性能基准测试
    - 执行安全扫描
    - 生成测试报告

  human_tasks:
    - Review 测试报告
    - 分析失败原因
    - 决策: 通过/修复/降级

  artifacts:
    - testing/unit-report.md (AI 生成)
    - testing/performance-report.md (AI 生成)
    - testing/security-scan.md (AI 生成)

  example:
    ai_report: |
      # 测试报告

      ✅ 单元测试: 45/45 通过 (覆盖率 87%)
      ✅ 集成测试: 12/12 通过
      ⚠️  性能测试: 内存占用略高于预期 (120MB vs 100MB)
      ✅ 安全扫描: 无高危漏洞

      ## 建议
      内存占用可接受，建议发布 v1.0.0
```

**验收标准**:
- [ ] 所有 P0/P1 测试通过
- [ ] 性能指标达标 (或人类豁免)
- [ ] 无高危安全漏洞
- [ ] 人类签字确认

---

### 阶段 6: AI 自动化文档和总结

**人类角色**: Review 最终文档
**AI 角色**: 自动生成所有文档

```yaml
workflow:
  ai_tasks:
    - 生成用户手册
    - 生成 API 文档
    - 生成维护指南
    - 提取经验教训
    - 生成项目总结

  human_tasks:
    - Review 文档
    - 补充使用经验
    - 确认发布

  artifacts:
    - docs/user-guide.md (AI 生成)
    - docs/api-reference.md (AI 生成)
    - docs/maintenance.md (AI 生成)
    - docs/lessons-learned.md (AI 提取)

  example:
    ai_generated: |
      # 用户手册

      ## 快速开始

      ```rust
      use cis_core::monitor::CpuMonitor;

      #[tokio::main]
      async fn main() -> Result<()> {
          let monitor = CpuMonitor::new();
          monitor.start().await?;

          let metric = monitor.collect().await?;
          println!("CPU: {}%", metric.value);

          Ok(())
      }
      ```

      ## 配置
      - 监控间隔: 默认 5s
      - 告警阈值: CPU > 80%
```

**验收标准**:
- [ ] 文档完整
- [ ] 用户可自助使用
- [ ] 人类确认发布

---

## AI-First 工作流对比

| 阶段 | 传统模式 | AI-First 模式 | 效率提升 |
|------|----------|---------------|----------|
| 需求分析 | 人类 2-3天 | AI 2h + 人类 1h | **60%** |
| 文档生成 | 人类 1-2天 | AI 30min + 人类 1h | **70%** |
| 方案设计 | 人类 2-4天 | AI 1h + 人类 2h | **75%** |
| 代码开发 | 人类 5-10天 | AI 1天 + 人类 2天 | **60%** |
| 测试验收 | 人类 2-3天 | AI 自动化 + 人类 0.5天 | **80%** |
| 文档总结 | 人类 1-2天 | AI 1h + 人类 0.5h | **80%** |

**总体效率提升**: **约 70%**

---

## 实现框架

### Skill 工作流配置 (YAML)

```yaml
name: "ai-skill-dev-workflow"
version: "2.0.0"

ai_config:
  model: "claude-sonnet-4"
  temperature: 0.3  # 低温度，确保稳定性

stages:
  analysis:
    ai_role: "需求分析师"
    tasks:
      - 分析需求语义
      - 识别歧义
      - 提出澄清问题
    output: "requirements/analysis.md"

  documentation:
    ai_role: "技术文档工程师"
    tasks:
      - 生成技术规格
      - 定义 API 接口
      - 生成数据模型
    output: "design/spec.md"

  design:
    ai_role: "架构师"
    tasks:
      - 生成多个方案
      - 分析优劣
      - 推荐最佳方案
    output: "design/schemes.md"

  development:
    ai_role: "Rust 开发工程师"
    tasks:
      - 生成代码
      - 生成测试
      - 生成文档注释
    output: "src/"

  testing:
    ai_role: "测试工程师"
    tasks:
      - 执行测试
      - 生成报告
    output: "testing/report.md"

  summary:
    ai_role: "技术文档工程师"
    tasks:
      - 生成用户手册
      - 生成维护指南
      - 提取经验教训
    output: "docs/"

quality_gates:
  - stage: "design"
    condition: "人类选择方案"
  - stage: "development"
    condition: "代码审查通过"
  - stage: "testing"
    condition: "无 P0/P1 Bug"
```

---

## 最佳实践

### 1. Prompt 工程模板

每个阶段都有优化的 Prompt 模板：

```markdown
# 阶段 X: {名称}

## 角色
你是一个经验丰富的 {角色}

## 上下文
- CIS 架构文档: {link}
- 需求文档: {link}
- 现有代码: {link}

## 任务
{具体任务}

## 约束
- 遵循 Rust 编码规范
- 遵循 CIS 第一性原理
- 性能要求: {spec}

## 输出格式
{输出模板}

## 示例
{示例}
```

### 2. 人机协作模式

```yaml
pattern:
  1. AI 执行任务
  2. AI 生成输出
  3. 人类 Review
  4. 人类反馈 (可选)
  5. AI 迭代优化 (回到 1)
  6. 人类确认

loop_limit: 3  # 最多迭代 3 次
```

### 3. 质量保证

```yaml
qa_checkpoints:
  - 阶段 3: 设计方案评审
  - 阶段 4: 代码审查 (重点: 安全、性能)
  - 阶段 5: 测试报告分析

automated_checks:
  - 代码格式化 (rustfmt)
  - 代码检查 (clippy)
  - 单元测试覆盖率
  - 性能基准测试
```

---

## 工具支持

### CLI 工具

```bash
# 创建新 Skill 项目
cis skill new my-monitor --workflow ai-first

# 执行特定阶段
cis skill run stage:analysis

# 查看进度
cis skill status

# Review AI 输出
cis skill review stage:development
```

### VSCode 集成

- AI 输出文件自动高亮
- 一键 Review/Accept/Reject
- 迭代历史记录
- 差异对比视图

---

**文档版本**: v2.0 (AI-First)
**最后更新**: 2026-02-02
**作者**: CIS Team
