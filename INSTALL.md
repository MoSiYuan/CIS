# CIS 安装和配置指南

## 📦 安装方式

### 方式一：自动安装脚本（推荐）

**macOS/Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/your-org/cis/main/scripts/install/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/your-org/cis/main/scripts/install/install.ps1 | iex
```

### 方式二：从源码安装

```bash
# 克隆仓库
git clone https://github.com/your-org/cis.git
cd cis

# 运行开发环境设置脚本
./scripts/install/setup-dev.sh
```

### 方式三：手动安装

```bash
# 构建 Release 版本
cargo build --release --bin cis-node

# 创建符号链接（可选）
ln -sf $(pwd)/target/release/cis-node ~/.local/bin/cis

# 初始化
~/.local/bin/cis init
```

---

## 🔧 路径策略

CIS 使用智能路径解析策略，根据运行模式自动选择数据目录：

### 1. 环境变量（最高优先级）
```bash
export CIS_DATA_DIR=/custom/path
```

### 2. Release/便携模式
当可执行文件位于 `target/release/` 目录时，或设置 `CIS_PORTABLE=1` 时：
- **数据目录**: `<可执行文件目录>/.cis/`
- **配置**: `<可执行文件目录>/.cis/config.toml`

适合：便携使用、USB 携带、无安装权限环境

### 3. Git 项目模式
在 Git 仓库中运行时：
- **数据目录**: `<Git根目录>/.cis/`
- **配置**: `<Git根目录>/.cis/config.toml`

适合：项目管理、团队协作、版本控制

### 4. 系统默认模式
不在 Git 项目中的开发模式：
- **macOS**: `~/.cis/`
- **Linux**: `~/.cis/`
- **Windows**: `%USERPROFILE%\.cis\`

适合：全局安装、多项目共享

---

## 🚀 首次启动

### 自动检测
首次运行 CIS 时，如果未初始化，会自动提示：

```bash
$ cis skill list
⚠️  CIS 尚未初始化

📁 CIS 路径信息:
───────────────────────────────────────────────────────────
  运行模式: Development (开发模式)
  Git 根目录: /Users/xxx/projects/my-project
  数据目录:   /Users/xxx/projects/my-project/.cis
  配置目录:   /Users/xxx/projects/my-project/.cis
  配置文件:   /Users/xxx/projects/my-project/.cis/config.toml
───────────────────────────────────────────────────────────

💡 请先初始化 CIS:
   cis init           # 交互式初始化
   cis init --help    # 查看初始化选项

   或使用快速初始化:
   cis init --non-interactive --provider claude

📁 检测到 Git 项目: /Users/xxx/projects/my-project
   初始化数据将存储在: /Users/xxx/projects/my-project/.cis
```

### 初始化选项

```bash
# 交互式初始化（推荐）
cis init

# 快速初始化
cis init --non-interactive --provider claude

# 项目级初始化（在 Git 项目中）
cis init --project

# 强制重新初始化
cis init --force

# 跳过环境检查
cis init --skip-checks
```

---

## 📋 配置示例

参见 `config.example.toml` 获取完整配置说明。

快速配置示例：

```toml
[node]
id = "自动生成"
name = "my-node"

[ai]
default_provider = "claude"

[ai.claude]
model = "claude-sonnet-4-20250514"
max_tokens = 4096
```

---

## 🐚 Shell 别名（可选）

添加以下内容到 `~/.bashrc` 或 `~/.zshrc`：

```bash
source /path/to/cis/scripts/shell-aliases.sh
```

提供的快捷命令：
- `cis-doctor`, `cis-status`, `cis-paths`
- `cis-skills`, `cis-do`, `cis-chain`
- `cis-search`, `cis-remember`, `cis-recall`
- `cis-chat`, `cis-ask`, `cis-context`
- `cis-tasks`, `cis-task-new`, `cis-task-done`

查看所有别名：
```bash
cis-aliases
```

---

## 🔍 故障排除

### 检查环境
```bash
cis doctor        # 完整检查
cis doctor --fix  # 自动修复
```

### 查看路径信息
```bash
cis status --paths
```

### 常见问题

**Q: CIS 找不到配置文件？**
```bash
# 检查路径配置
cis status --paths

# 手动指定数据目录
export CIS_DATA_DIR=/path/to/data
cis init
```

**Q: 如何切换便携模式？**
```bash
# 设置环境变量
export CIS_PORTABLE=1

# 或在 target/release 中运行
cargo build --release
./target/release/cis-node status --paths
```

**Q: 如何在多个项目间切换？**
```bash
# 在每个 Git 项目中独立初始化
cd project-a
cis init --project

cd project-b
cis init --project

# CIS 会自动检测当前 Git 项目并使用对应的配置
```

---

## 📚 更多信息

- [使用指南](docs/USAGE.md)
- [API 文档](docs/API.md)
- [开发文档](docs/DEVELOPMENT.md)
