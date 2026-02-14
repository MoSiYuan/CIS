# 贡献指南

感谢您对 CIS (Cluster of Independent Systems) 项目的关注！本文档将帮助您了解如何参与到 CIS 的开发中。

## 目录

- [行为准则](#行为准则)
- [如何贡献](#如何贡献)
- [开发环境设置](#开发环境设置)
- [代码规范](#代码规范)
- [提交信息规范](#提交信息规范)
- [Pull Request 流程](#pull-request-流程)
- [版本发布](#版本发布)

---

## 行为准则

参与本项目即表示您同意遵守以下准则：

- 尊重所有参与者，无论其背景如何
- 接受建设性的批评，并以优雅的方式处理
- 关注对社区最有利的事情
- 对其他社区成员表示同理心

---

## 如何贡献

### 报告 Bug

如果您发现了 Bug，请通过 [GitHub Issues](https://github.com/MoSiYuan/CIS/issues) 报告，并包含以下信息：

1. **问题描述**：清晰简洁地描述 Bug
2. **复现步骤**：详细的步骤说明
3. **期望行为**：说明您期望发生什么
4. **实际行为**：说明实际发生了什么
5. **环境信息**：
   - 操作系统及版本
   - CIS 版本 (`cis --version`)
   - Rust 版本 (`rustc --version`)
6. **附加信息**：截图、日志等

### 建议新功能

我们欢迎新功能建议！请通过 GitHub Issues 提交，并包含：

1. **功能描述**：清晰描述您想要的功能
2. **使用场景**：说明为什么需要这个功能
3. **预期行为**：描述功能的具体行为
4. **可能的实现方案**（可选）

### 提交代码

1. Fork 本仓库
2. 创建您的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交您的修改 (`git commit -m 'feat: add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启一个 Pull Request

---

## 开发环境设置

### 前置要求

- **Rust** 1.70 或更高版本
- **Git**
- **SQLite** 3.40+ (用于开发测试)

### 快速开始

```bash
# 克隆仓库
git clone https://github.com/MoSiYuan/CIS.git
cd CIS

# 安装开发依赖
./scripts/install/setup-dev.sh

# 构建项目
cargo build --release

# 运行测试
cargo test --all-features

# 本地安装
cargo install --path cis-node
```

### 开发工具推荐

- **IDE**: VS Code 或 RustRover
- **VS Code 扩展**:
  - rust-analyzer
  - Even Better TOML
  - CodeLLDB (调试)

### 项目结构

```
CIS/
├── cis-core/          # 核心库
│   ├── src/
│   │   ├── agent/     # Agent 抽象
│   │   ├── memory/    # 记忆系统
│   │   ├── network/   # 网络模块
│   │   ├── p2p/       # P2P 网络
│   │   ├── matrix/    # Matrix 联邦
│   │   ├── scheduler/ # DAG 调度器
│   │   ├── skill/     # Skill 系统
│   │   └── ...
│   └── tests/         # 集成测试
├── cis-node/          # CLI 入口
│   └── src/commands/  # 命令实现
├── cis-gui/           # GUI 应用
├── cis-skill-sdk/     # Skill SDK
├── skills/            # 内置 Skill
└── docs/              # 文档
```

---

## 代码规范

### Rust 编码规范

我们遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) 和以下规则：

#### 格式化

```bash
# 自动格式化代码
cargo fmt

# 检查格式
cargo fmt -- --check
```

#### Lint

```bash
# 运行 Clippy
cargo clippy --all-features -- -D warnings
```

#### 命名规范

- **模块/文件**: `snake_case`
- **类型/结构体**: `PascalCase`
- **函数/变量**: `snake_case`
- **常量**: `SCREAMING_SNAKE_CASE`
- **特性**: `snake_case`

#### 文档

- 所有公共 API 必须有文档注释
- 使用 `///` 为函数和类型添加文档
- 使用 `//!` 为模块添加文档
- 包含示例代码

```rust
/// 计算两个数的和
///
/// # Examples
///
/// ```
/// let sum = add(2, 3);
/// assert_eq!(sum, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

#### 错误处理

- 使用 `thiserror` 定义错误类型
- 使用 `anyhow` 进行错误传播
- 避免使用 `unwrap()` 和 `expect()`，使用 `?` 操作符

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, MyError>;
```

### 测试规范

#### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn test_add_overflow() {
        add(i32::MAX, 1);
    }
}
```

#### 集成测试

集成测试放在 `tests/` 目录下：

```rust
// tests/integration_test.rs
use cis_core::*;

#[tokio::test]
async fn test_memory_operations() {
    // 测试代码
}
```

运行测试：

```bash
# 运行所有测试
cargo test --all-features

# 运行特定测试
cargo test test_name

# 运行带日志的测试
RUST_LOG=debug cargo test -- --nocapture
```

---

## 提交信息规范

我们遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

### 格式

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Type

| 类型 | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 仅文档修改 |
| `style` | 不影响代码含义的修改（空格、格式化等） |
| `refactor` | 代码重构（既不是修复也不是功能） |
| `perf` | 性能优化 |
| `test` | 添加或修正测试 |
| `chore` | 构建过程或辅助工具的变动 |

### Scope

可选，用于指定修改的范围：

- `core`: cis-core
- `node`: cis-node
- `gui`: cis-gui
- `skill`: Skill 系统
- `network`: 网络模块
- `storage`: 存储层
- `docs`: 文档

### 示例

```
feat(skill): add vector-based skill routing

Implement semantic skill routing using embeddings.
This allows natural language skill discovery.

Closes #123
```

```
fix(storage): resolve SQLite deadlock on concurrent writes

Add retry logic with exponential backoff for database
operations that fail with "database is locked" error.

Fixes #456
```

```
docs(readme): update installation instructions

Add Homebrew installation method and clarify
portable mode usage.
```

---

## Pull Request 流程

### 准备 PR

1. **更新文档**：如果您的修改涉及 API 变更，请更新相关文档
2. **添加测试**：确保新功能有测试覆盖
3. **运行检查**：
   ```bash
   cargo fmt -- --check
   cargo clippy --all-features -- -D warnings
   cargo test --all-features
   ```
4. **更新 CHANGELOG.md**：在 Unreleased 部分添加您的修改

### PR 模板

```markdown
## 描述
简要描述这个 PR 做了什么

## 类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 破坏性变更
- [ ] 文档更新
- [ ] 性能优化
- [ ] 代码重构

## 检查清单
- [ ] 代码遵循项目规范
- [ ] 添加了测试
- [ ] 所有测试通过
- [ ] 更新了文档
- [ ] 更新了 CHANGELOG.md

## 相关 Issue
Fixes #(issue 编号)
```

### 审查流程

1. 至少需要一个维护者的批准
2. CI 检查必须通过
3. 冲突必须解决

---

## 版本发布

### 版本号规范

遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)：

- **MAJOR**: 不兼容的 API 修改
- **MINOR**: 向下兼容的功能性新增
- **PATCH**: 向下兼容的问题修正

### 发布流程

1. 更新 `CHANGELOG.md`
2. 更新版本号（`Cargo.toml`）
3. 创建 Git tag：`git tag -a v1.0.0 -m "Release version 1.0.0"`
4. Push tag：`git push origin v1.0.0`
5. GitHub Actions 会自动构建并发布

---

## 获取帮助

- **GitHub Issues**: [问题讨论](https://github.com/MoSiYuan/CIS/issues)
- **文档**: [项目文档](https://github.com/MoSiYuan/CIS/tree/main/docs)

---

再次感谢您的贡献！🎉
