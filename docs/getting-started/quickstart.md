# CIS 快速开始指南

5 分钟内上手 CIS！

## 前提条件

- macOS、Linux 或 Windows
- 网络连接（用于下载和 AI 服务）
- 可选：Git 仓库（用于项目模式）

---

## 步骤 1：安装 CIS

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.ps1 | iex
```

### Homebrew (macOS/Linux)

```bash
brew tap mosiyuan/cis
brew install cis
```

### 验证安装

```bash
cis --version
cis --help
```

---

## 步骤 2：初始化 CIS

### 交互式初始化（推荐）

```bash
cis init
```

跟随提示：
1. 选择 AI Provider（Claude / Kimi / OpenAI / Ollama）
2. 输入 API 密钥
3. 配置节点名称

### 快速初始化

```bash
# 使用 Claude，跳过交互
cis init --non-interactive --provider claude
```

### 项目级初始化

在 Git 项目中：

```bash
cd my-project
cis init --project
```

这会创建 `.cis/` 目录，配置仅对当前项目生效。

---

## 步骤 3：检查环境

```bash
cis doctor
```

预期输出：
```
✅ Rust toolchain: 1.75.0
✅ SQLite: 3.43.0
✅ Configuration: /Users/xxx/.cis/config.toml
✅ Node identity: did:cis:abc123...
⚠️  P2P network: not started
💡 Run 'cis node start' to start P2P network
```

如果有 ❌，运行 `cis doctor --fix` 自动修复。

---

## 步骤 4：基础使用

### 查看技能列表

```bash
cis skill list
```

### 使用自然语言调用技能

```bash
cis skill do "总结昨天的代码提交"
```

### 语义搜索记忆

```bash
cis memory search "暗黑模式配置"
```

### 与 AI 对话

```bash
cis agent "如何优化这个函数？"
```

### 启动 P2P 网络

```bash
cis node start
```

---

## 步骤 5：配置 AI Provider

编辑配置文件：

```bash
# 打开配置文件
cis system edit-config
```

### Claude 配置

```toml
[ai]
default_provider = "claude"

[ai.claude]
api_key = "sk-ant-xxx"
model = "claude-sonnet-4-20250514"
max_tokens = 4096
```

### Kimi 配置

```toml
[ai]
default_provider = "kimi"

[ai.kimi]
api_key = "sk-xxx"
model = "kimi-latest"
```

### 本地模型 (Ollama)

```toml
[ai]
default_provider = "ollama"

[ai.ollama]
base_url = "http://localhost:11434"
model = "llama3.1"
```

---

## 常用命令速查

| 命令 | 说明 |
|------|------|
| `cis init` | 初始化 CIS |
| `cis doctor` | 环境检查 |
| `cis status` | 查看状态 |
| `cis skill list` | 列出技能 |
| `cis skill do "..."` | 调用技能 |
| `cis memory search "..."` | 搜索记忆 |
| `cis agent "..."` | AI 对话 |
| `cis node start` | 启动网络 |
| `cis network list` | 查看节点 |

---

## 下一步

- 阅读 [完整使用指南](../USAGE.md)
- 学习 [开发 Skill](../SKILL_DEVELOPMENT.md)
- 了解 [架构设计](../ARCHITECTURE.md)
- 遇到问题时查看 [故障排除](../TROUBLESHOOTING.md)

---

## 获取帮助

```bash
# 查看命令帮助
cis --help
cis <command> --help

# 生成补全脚本
cis completion bash > ~/.bash_completion.d/cis
cis completion zsh > ~/.zsh/completions/_cis
```
