# CIS Shell 集成

这个目录包含 CIS 的 Shell 集成脚本，提供命令补全、别名、快捷函数和目录钩子等功能。

## 支持的 Shell

- **Bash** (`cis.bash`)
- **Zsh** (`cis.zsh`)
- **Fish** (`cis.fish`)

## 安装方法

### 手动安装

#### Bash

```bash
# 添加到 ~/.bashrc
echo 'source /usr/local/share/cis/cis.bash' >> ~/.bashrc

# 或者从项目目录直接加载
source packaging/shell/cis.bash
```

#### Zsh

```zsh
# 添加到 ~/.zshrc
echo 'source /usr/local/share/cis/cis.zsh' >> ~/.zshrc

# 如果使用 Oh-My-Zsh，可以放在 custom/plugins 目录
mkdir -p ~/.oh-my-zsh/custom/plugins/cis
cp packaging/shell/cis.zsh ~/.oh-my-zsh/custom/plugins/cis/cis.plugin.zsh
# 然后在 ~/.zshrc 的 plugins 中添加 cis
```

#### Fish

```fish
# 添加到 config.fish
echo 'source /usr/local/share/cis/cis.fish' >> ~/.config/fish/config.fish

# 或者安装到 Fish 的函数目录
mkdir -p ~/.config/fish/conf.d
cp packaging/shell/cis.fish ~/.config/fish/conf.d/cis.fish
```

### Homebrew 安装（推荐）

如果使用 Homebrew 安装 CIS，Shell 集成会自动安装：

```bash
# 对于 Bash
echo 'source $(brew --prefix)/share/cis/cis.bash' >> ~/.bashrc

# 对于 Zsh
echo 'source $(brew --prefix)/share/cis/cis.zsh' >> ~/.zshrc

# 对于 Fish
# 自动加载，无需额外配置
```

## 功能特性

### 1. 命令补全

自动为 `cis-node` 和 `cis-cli` 生成命令补全。

### 2. 快捷别名

| 别名 | 命令 | 说明 |
|------|------|------|
| `cis-start` | `cis node start` | 启动节点 |
| `cis-stop` | `cis node stop` | 停止节点 |
| `cis-status` | `cis node status` | 查看状态 |
| `cis-dag-list` | `cis dag list` | 列出 DAG |
| `cis-dag-run` | `cis dag run` | 运行 DAG |
| `cis-task-list` | `cis task list` | 列出任务 |
| `cis-mem-search` | `cis memory search` | 搜索记忆 |
| `cis-peers` | `cis network list` | 列出对等节点 |
| `cis-health` | `cis doctor` | 健康检查 |
| `cis-logs` | `tail -f ~/.cis/logs/cis.log` | 查看日志 |

### 3. 快捷函数

#### `cis-run <dag-name>`
快速执行 DAG：
```bash
cis-run my-dag --arg1 value1
```

#### `cis-search <query>`
快速搜索记忆：
```bash
cis-search "暗黑模式配置"
```

#### `cis-watch [task-id]`
实时监控任务状态：
```bash
# 监控所有任务
cis-watch

# 监控特定任务
cis-watch task-abc123
```

#### `cis-cd [project]`
进入 CIS 项目目录：
```bash
# 进入项目根目录
cis-cd

# 进入特定项目
cis-cd my-project
```

### 4. chpwd 钩子

当切换目录时，自动检测 `.cis/config.toml` 文件：

- 如果进入 CIS 项目目录，自动设置 `CIS_HOME` 环境变量
- 显示项目信息（如果 `CIS_CHPWD_VERBOSE=1`）
- 自动加载 `.cisrc` 文件（如果存在）

### 5. fzf 集成（Zsh/Fish）

如果安装了 [fzf](https://github.com/junegunn/fzf)，提供交互式命令：

#### `cis-dag-fzf` / `cdf`
交互式选择并运行 DAG：
```bash
cis-dag-fzf
# 或
alias cdf='cis-dag-fzf'
cdf
```

#### `cis-mem-fzf` / `cmf`
交互式搜索记忆：
```bash
cis-mem-fzf "搜索关键词"
# 或
alias cmf='cis-mem-fzf'
cmf "搜索关键词"
```

### 6. 提示符集成

可选在提示符中显示 CIS 节点状态（需要手动启用）：

```bash
# Bash - 添加到 ~/.bashrc
CIS_CHPWD_VERBOSE=1
PS1='$(__cis_prompt)'$PS1

# Zsh - 添加到 ~/.zshrc
CIS_CHPWD_VERBOSE=1
setopt prompt_subst
PROMPT='$(__cis_prompt)'$PROMPT

# Fish - 添加到 config.fish
set -g CIS_CHPWD_VERBOSE 1
function fish_prompt
    printf '%s%s > ' (__cis_prompt) $PWD
end
```

显示效果：
```
# 节点运行中且有 3 个对等节点
🟢[3] ~/projects/my-project >

# 节点未运行
~/projects/other-project >
```

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `CIS_EDITOR` | `$EDITOR` 或 `nano` | 默认编辑器 |
| `CIS_LOG_LEVEL` | `info` | 日志级别 |
| `CIS_HOME` | `~/.cis` | CIS 数据目录 |
| `CIS_CHPWD_VERBOSE` | `0` | 是否显示目录切换信息 |
| `CIS_SHELL_SILENT` | `0` | 是否禁用欢迎信息 |

## 自定义配置

可以在 shell 配置文件（如 `~/.bashrc` 或 `~/.zshrc`）中覆盖默认设置：

```bash
# 使用 vim 作为默认编辑器
export CIS_EDITOR=vim

# 设置日志级别为 debug
export CIS_LOG_LEVEL=debug

# 启用详细模式
export CIS_CHPWD_VERBOSE=1

# 禁用欢迎信息
export CIS_SHELL_SILENT=1
```

## 示例 .cisrc 文件

在项目根目录创建 `.cisrc` 文件，进入目录时自动加载：

```bash
# 项目特定配置
export CIS_PROJECT_NAME="my-awesome-project"
export CIS_DAG_PATH="./dags"
export CIS_ENV="development"

# 快捷别名
alias run-test='cis dag run test-pipeline'
alias deploy='cis dag run deploy'
```

## 故障排除

### 补全不生效

```bash
# 检查 cis-node 是否在 PATH 中
which cis-node

# 手动生成补全脚本
cis-node completions bash > /tmp/cis-completion.bash
source /tmp/cis-completion.bash
```

### chpwd 钩子不生效

```bash
# 检查是否正确加载了脚本
type __cis_chpwd

# 手动触发钩子
cd .
```

### 权限问题

```bash
# 确保脚本有执行权限
chmod +x /usr/local/share/cis/cis.bash
```
