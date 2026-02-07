# CIS 配置指南

本文档详细介绍 CIS 的配置文件结构和选项。

---

## 配置文件位置

CIS 使用分层配置策略：

### 1. 系统默认

- **macOS/Linux**: `~/.cis/config.toml`
- **Windows**: `%USERPROFILE%\.cis\config.toml`

### 2. 项目级配置

在 Git 仓库中：`.cis/config.toml`

优先级高于全局配置。

### 3. 环境变量覆盖

```bash
export CIS_DATA_DIR=/custom/path
export CIS_CONFIG_FILE=/path/to/config.toml
export CIS_LOG_LEVEL=debug
```

---

## 完整配置示例

```toml
# CIS 主配置文件

# ==================== 节点配置 ====================
[node]
# 节点名称（显示用）
name = "my-workstation"

# 节点密钥（自动生成，不要手动修改）
# key = "..."

# 节点标签（用于分组）
tags = ["dev", "work"]

# ==================== AI 配置 ====================
[ai]
# 默认 AI Provider
default_provider = "claude"

# 默认模型参数
default_max_tokens = 4096
default_temperature = 0.7

# Claude 配置
[ai.claude]
api_key = "sk-ant-..."
model = "claude-sonnet-4-20250514"
max_tokens = 4096

# Kimi 配置
[ai.kimi]
api_key = "sk-..."
model = "kimi-latest"

# OpenAI 配置
[ai.openai]
api_key = "sk-..."
model = "gpt-4"

# Ollama（本地模型）配置
[ai.ollama]
base_url = "http://localhost:11434"
model = "llama3.1"

# ==================== 存储配置 ====================
[storage]
# 数据目录（默认自动检测）
# data_dir = "/path/to/data"

# WAL 模式
wal_mode = true

# 自动清理
cleanup_interval_hours = 24

# 保留天数
retention_days = 365

# ==================== P2P 网络配置 ====================
[p2p]
# 启用 P2P
enabled = true

# 监听端口
listen_port = 7677

# 启用 DHT 发现
enable_dht = true

# 启用 NAT 穿透
enable_nat_traversal = true

# 启动节点
[p2p.bootstrap]
nodes = [
    # "did:cis:xxx@192.168.1.100:7677",
]

# ==================== Matrix 联邦配置 ====================
[matrix]
# 启用 Matrix 协议
enabled = true

# 监听地址
bind_address = "0.0.0.0:7676"

# 联邦端口
federation_port = 6767

# 服务器名称
server_name = "localhost"

# ==================== 网络访问控制 ====================
[network]
# 网络模式: whitelist, open, solitary, quarantine
mode = "whitelist"

# 自动接受已知节点
auto_accept_known = false

# ACL 同步间隔（分钟）
acl_sync_interval_minutes = 60

# ==================== 安全配置 ====================
[security]
# 加密私域记忆
encrypt_private_memory = true

# 密钥派生迭代次数
key_derivation_iterations = 100000

# 会话超时（分钟）
session_timeout_minutes = 30

# ==================== WASM 运行时配置 ====================
[wasm]
# 最大内存（MB）
max_memory_mb = 512

# 执行超时（秒）
execution_timeout_seconds = 30

# 允许的网络域名
allowed_hosts = ["api.example.com"]

# ==================== 日志配置 ====================
[log]
# 日志级别: trace, debug, info, warn, error
level = "info"

# 日志文件
file = "~/.cis/logs/cis.log"

# 日志轮转
rotation = "daily"

# 保留天数
retention_days = 30

# ==================== GUI 配置 ====================
[gui]
# 默认主题: system, light, dark
theme = "system"

# 字体大小
font_size = 14

# 窗口大小
window_width = 1200
window_height = 800

# ==================== 遥测配置 ====================
[telemetry]
# 启用遥测
enabled = true

# 匿名统计
anonymous_stats = true

# 错误报告
error_reporting = true
```

---

## 配置项详解

### AI Provider 配置

#### 多 Provider 配置

```toml
[ai]
# 默认 Provider
default_provider = "claude"

# 为不同场景指定 Provider
[ai.providers]
# 代码相关任务使用 Claude
code = "claude"
# 快速响应用 Kimi
quick = "kimi"
# 离线场景用 Ollama
offline = "ollama"
```

#### 模型参数覆盖

```toml
[ai.claude]
api_key = "${CLAUDE_API_KEY}"  # 支持环境变量
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

# 特定任务参数
[ai.claude.params.code]
max_tokens = 8192
temperature = 0.2  # 更确定性

[ai.claude.params.creative]
max_tokens = 2048
temperature = 0.9  # 更创造性
```

### 网络配置

#### 静态节点配置

```toml
[p2p.bootstrap]
nodes = [
    # 格式: did@host:port
    "did:cis:abc123@192.168.1.100:7677",
    "did:cis:def456@10.0.0.50:7677",
]

# 手动信任的节点
[p2p.trusted]
nodes = [
    "did:cis:abc123",
]
```

#### 防火墙/NAT 配置

```toml
[p2p.nat]
# UPnP 自动端口映射
enable_upnp = true

# 手动指定公网 IP（如果有）
# external_ip = "203.0.113.1"

# STUN 服务器
stun_servers = [
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
]
```

### 存储配置

#### 多磁盘配置

```toml
[storage]
# 核心数据（小文件，高频率）
core_data_dir = "/fast/ssd/.cis/data"

# 向量数据（大文件，搜索用）
vector_data_dir = "/fast/ssd/.cis/vectors"

# 归档数据（历史记录）
archive_dir = "/big/hdd/.cis/archive"
```

#### 备份配置

```toml
[storage.backup]
# 启用自动备份
enabled = true

# 备份目录
backup_dir = "/backup/cis"

# 备份间隔（小时）
interval_hours = 24

# 保留备份数
retention_count = 7
```

---

## 配置验证

### 检查配置语法

```bash
cis doctor --check-config
```

### 查看生效配置

```bash
# 显示合并后的配置
cis config show

# 显示特定部分
cis config show ai

# 以 JSON 格式输出
cis config show --format json
```

### 编辑配置

```bash
# 使用默认编辑器
cis config edit

# 或直接编辑文件
$EDITOR ~/.cis/config.toml
```

---

## 配置热重载

部分配置支持热重载（无需重启）：

```bash
# 重新加载配置
cis system reload-config
```

支持热重载的配置：
- `ai.*` (除 api_key 外)
- `log.level`
- `network.mode`
- `security.session_timeout_minutes`

---

## 配置迁移

### 导出配置

```bash
cis config export > cis-config-backup.toml
```

### 导入配置

```bash
cis config import cis-config-backup.toml
```

### 迁移到新机器

```bash
# 旧机器
cis system backup --output cis-backup.tar.gz

# 新机器
cis system restore --from cis-backup.tar.gz
```

---

## 环境变量参考

| 变量 | 说明 | 示例 |
|------|------|------|
| `CIS_DATA_DIR` | 数据目录 | `/var/cis` |
| `CIS_CONFIG_FILE` | 配置文件路径 | `/etc/cis/config.toml` |
| `CIS_LOG_LEVEL` | 日志级别 | `debug` |
| `CIS_PORTABLE` | 便携模式 | `1` |
| `CIS_PROJECT_MODE` | 强制项目模式 | `1` |
| `CLAUDE_API_KEY` | Claude API 密钥 | `sk-ant-...` |
| `KIMI_API_KEY` | Kimi API 密钥 | `sk-...` |
| `OPENAI_API_KEY` | OpenAI API 密钥 | `sk-...` |

---

## 配置文件模板

### 开发者配置

```toml
[node]
name = "dev-workstation"
tags = ["dev", "personal"]

[ai]
default_provider = "ollama"

[ai.ollama]
base_url = "http://localhost:11434"
model = "codellama"

[p2p]
enabled = true
listen_port = 7677
```

### 服务器配置

```toml
[node]
name = "prod-server-01"
tags = ["production", "server"]

[ai]
default_provider = "claude"

[storage]
data_dir = "/var/lib/cis"

[p2p]
enabled = true
listen_port = 7677

[network]
mode = "whitelist"
```

---

## 故障排除

### 配置未生效

1. 检查配置文件路径：`cis status --paths`
2. 验证配置语法：`cis doctor --check-config`
3. 检查文件权限：`ls -la ~/.cis/config.toml`

### 敏感信息泄漏

不要在配置文件中硬编码 API 密钥：

```toml
# ❌ 不安全
api_key = "sk-actual-key"

# ✅ 使用环境变量
api_key = "${CLAUDE_API_KEY}"
```

### 配置冲突

项目级配置覆盖全局配置：

```bash
# 查看配置来源
cis config show --verbose
```

---

更多配置选项请参考 [架构文档](../ARCHITECTURE.md)。
