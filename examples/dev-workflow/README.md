# 开发工作流示例

这个示例展示如何使用 CIS 构建自动化开发工作流，包括代码审查、CI/CD 和项目管理集成。

## 功能特性

- 🔄 自动化 CI/CD 流水线
- 🔍 智能代码审查
- 📝 自动生成发布说明
- 📊 项目指标跟踪
- 🐛 智能 Bug 分析

## 目录结构

```
dev-workflow/
├── README.md
├── config.toml
├── dags/
│   ├── ci-pipeline.dag.toml
│   ├── code-review.dag.toml
│   ├── release.dag.toml
│   └── daily-report.dag.toml
├── scripts/
│   ├── lint.sh
│   ├── test.sh
│   └── deploy.sh
└── .github/
    └── workflows/
        └── cis-trigger.yml
```

## 快速开始

### 1. 克隆示例

```bash
cd examples/dev-workflow
```

### 2. 配置环境

```bash
# 初始化 CIS
cis init

# 配置 Git 集成
export GITHUB_TOKEN="your-github-token"
```

### 3. 运行示例

```bash
# 运行 CI 流水线
cis dag run ci-pipeline --arg branch=main

# 代码审查
cis dag run code-review --arg pr=123

# 发布
cis dag run release --arg version=1.2.0
```

## DAG 说明

### ci-pipeline.dag.toml

完整的 CI/CD 流水线：

```toml
[dag]
name = "ci-pipeline"
description = "CI/CD 流水线"

[vars]
branch = "main"
commit = ""

[step.checkout]
command = "git checkout {{branch}}"

[step.lint]
command = "./scripts/lint.sh"
depends_on = ["checkout"]

[step.test]
command = "./scripts/test.sh"
depends_on = ["lint"]

[step.build]
command = "cargo build --release"
depends_on = ["test"]

[step.deploy]
command = "./scripts/deploy.sh"
depends_on = ["build"]
when = "{{branch}} == 'main'"
```

### code-review.dag.toml

AI 辅助代码审查：

```toml
[dag]
name = "code-review"
description = "AI 辅助代码审查"

[vars]
pr = ""

[step.fetch-diff]
command = "gh pr diff {{pr}} > /tmp/pr.diff"

[step.analyze]
command = "cis skill do '审查以下代码变更：' < /tmp/pr.diff"
depends_on = ["fetch-diff"]

[step.check-issues]
command = "cis skill do '检查潜在问题和改进建议'"
depends_on = ["analyze"]

[step.post-comment]
command = "gh pr comment {{pr}} --body-file /tmp/review.md"
depends_on = ["check-issues"]
```

### release.dag.toml

自动化发布：

```toml
[dag]
name = "release"
description = "自动化发布流程"

[vars]
version = ""

[step.update-changelog]
command = "cis skill do '生成版本 {{version}} 的变更日志'"

[step.version-bump]
command = "cargo set-version {{version}}"
depends_on = ["update-changelog"]

[step.build-release]
command = "cargo build --release"
depends_on = ["version-bump"]

[step.create-tag]
command = "git tag v{{version}} && git push origin v{{version}}"
depends_on = ["build-release"]

[step.github-release]
command = "gh release create v{{version}} --generate-notes target/release/*"
depends_on = ["create-tag"]
```

### daily-report.dag.toml

每日项目报告：

```toml
[dag]
name = "daily-report"
description = "生成每日项目报告"
schedule = "0 18 * * 1-5"  # 工作日晚上 6 点

[step.collect-commits]
command = "git log --since='24 hours ago' --pretty=format:'%h %s' > /tmp/commits.txt"

[step.collect-issues]
command = "gh issue list --state all --limit 50 > /tmp/issues.txt"
depends_on = ["collect-commits"]

[step.generate-report]
command = "cis skill do '根据提交和 Issue 生成项目日报'"
depends_on = ["collect-issues"]

[step.send-notification]
command = "cis skill do '发送日报到团队频道'"
depends_on = ["generate-report"]
```

## GitHub Actions 集成

### cis-trigger.yml

在 GitHub Actions 中触发 CIS DAG：

```yaml
name: CIS Trigger

on:
  push:
    branches: [main, develop]
  pull_request:
    types: [opened, synchronize]

jobs:
  cis:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup CIS
        run: |
          curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash
          cis init --skip-did
      
      - name: Trigger CI Pipeline
        env:
          CIS_NODE_URL: ${{ secrets.CIS_NODE_URL }}
        run: |
          cis dag run ci-pipeline \
            --arg branch=${{ github.ref_name }} \
            --arg commit=${{ github.sha }}
```

## 项目管理集成

### Linear 集成

```toml
[step.sync-linear]
command = "cis skill do '同步 Linear Issue 状态'"

[step.update-status]
command = "linear issue update --id {{issue_id}} --state Done"
depends_on = ["sync-linear"]
```

### Jira 集成

```toml
[step.create-jira-ticket]
command = "jira issue create --project PROJ --type Bug --summary '{{summary}}'"
```

## 指标跟踪

### 代码质量指标

```toml
[step.analyze-coverage]
command = "cargo tarpaulin --out xml"

[step.upload-metrics]
command = "cis skill do '解析并记录代码覆盖率'"
depends_on = ["analyze-coverage"]
```

### 性能基准

```toml
[step.run-benchmarks]
command = "cargo bench"

[step.track-performance]
command = "cis skill do '记录性能基准数据'"
depends_on = ["run-benchmarks"]
```

## 使用方法

### 本地开发

```bash
# 运行代码检查
cis dag run ci-pipeline --arg branch=$(git branch --show-current)

# 请求代码审查
cis dag run code-review --arg pr=42
```

### 发布新版本

```bash
# 发布补丁版本
cis dag run release --arg version=1.0.1

# 发布 minor 版本
cis dag run release --arg version=1.1.0
```

### 查看报告

```bash
# 生成日报
cis dag run daily-report

# 查看历史报告
cis memory search "daily report"
```

## 高级用法

### 条件执行

```toml
[step.deploy-prod]
command = "./scripts/deploy.sh production"
when = "{{branch}} == 'main' && {{test_result}} == 'passed'"
```

### 并行执行

```toml
[step.test-unit]
command = "cargo test --lib"

[step.test-integration]
command = "cargo test --test '*'"

[step.coverage]
command = "cargo tarpaulin"
depends_on = ["test-unit", "test-integration"]
```

### 错误处理

```toml
[step.notify-failure]
command = "cis skill do '通知团队 CI 失败'"
on_failure = true
```

## 安全最佳实践

1. **密钥管理**: 使用 CIS 的密钥存储或环境变量
2. **访问控制**: 限制 DAG 执行权限
3. **审计日志**: 所有操作记录在 CIS 日志中

```toml
[step.deploy]
command = "./scripts/deploy.sh"
required_env = ["DEPLOY_KEY", "AWS_CREDENTIALS"]
```

## 故障排除

### DAG 执行失败

```bash
# 查看任务日志
cis task logs <task-id>

# 重新运行
cis dag run <dag-name> --retry
```

### 网络连接问题

```bash
# 检查节点状态
cis node status

# 检查网络连接
cis network ping <peer-id>
```

## 参考

- [CI/CD 最佳实践](../../docs/ci-cd-best-practices.md)
- [Git 集成](../../docs/git-integration.md)
- [项目管理](../../docs/project-management.md)
