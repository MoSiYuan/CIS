# OpenCode 后台/持久化模式调研报告

## 重要发现

**OpenCode CLI 实际上没有 `-p` (persistent/background) 参数。**

用户提到的 `-p` 模式可能是基于误解或与其他工具（如 `claude-code` 或 `codex`）混淆。在 OpenCode 中，`-p` 参数仅在 `opencode attach` 命令中表示 `--password`。

OpenCode 的持久化/后台功能通过以下机制实现：
- `opencode serve` - Headless HTTP 服务器模式
- `opencode acp` - Agent Client Protocol 服务器模式
- `opencode attach <url>` - 连接到运行中的服务器

---

## 1. 命令参考手册

### 1.1 基础命令结构

```bash
# 查看帮助
opencode --help
opencode run --help
opencode serve --help
opencode acp --help
opencode attach --help

# 版本信息
opencode --version  # 当前测试版本: 1.1.53
```

### 1.2 后台模式命令

#### `opencode serve` - Headless HTTP 服务器

启动一个无界面的 HTTP 服务器，提供 Web 界面和 API 端点。

```bash
# 基础用法
opencode serve

# 指定端口和主机
opencode serve --port 8080 --hostname 127.0.0.1

# 启用日志输出
opencode serve --port 8080 --print-logs --log-level DEBUG

# 启用 mDNS 服务发现
opencode serve --mdns --mdns-domain myagent.local

# 配置 CORS
opencode serve --cors "https://example.com"
```

**参数说明：**

| 参数 | 短形式 | 默认值 | 说明 |
|------|--------|--------|------|
| `--port` | | 0 (随机) | 监听端口 |
| `--hostname` | | 127.0.0.1 | 监听主机 |
| `--mdns` | | false | 启用 mDNS 服务发现 |
| `--mdns-domain` | | opencode.local | mDNS 域名 |
| `--cors` | | [] | 额外的 CORS 域名 |
| `--print-logs` | | false | 输出日志到 stderr |
| `--log-level` | | INFO | 日志级别 (DEBUG/INFO/WARN/ERROR) |

#### `opencode acp` - Agent Client Protocol 服务器

启动 ACP 兼容服务器，通过 stdio 与编辑器通信。

```bash
# 基础用法 - 通过 stdio 进行 JSON-RPC 通信
opencode acp

# 指定工作目录
opencode acp --cwd /path/to/project

# 网络配置
opencode acp --port 8080 --hostname 127.0.0.1
```

**注意：** ACP 模式主要用于编辑器集成（Zed、JetBrains、Neovim 等），默认通过 stdio 而非网络端口通信。

#### `opencode attach` - 连接到运行中的服务器

```bash
# Attach 到本地服务器
opencode attach http://localhost:8080

# 指定目录
opencode attach http://localhost:8080 --dir /path/to/project

# 指定会话
opencode attach http://localhost:8080 --session <session-id>

# 使用密码认证
opencode attach http://localhost:8080 --password mypassword
# 或使用环境变量
OPENCODE_SERVER_PASSWORD=mypassword opencode attach http://localhost:8080
```

#### `opencode run --attach` - 单次命令模式

```bash
# 向运行中的服务器发送单个命令
opencode run --attach http://localhost:8080 -- "Hello, process this for me"

# 指定模型
opencode run --attach http://localhost:8080 -m anthropic/claude-sonnet-4 -- "Hello"
```

### 1.3 其他相关命令

```bash
# 查看会话列表
opencode session list

# 导出会话
opencode export <session-id>

# 查看路径配置
opencode debug paths

# 查看完整配置
opencode debug config
```

---

## 2. 进程管理

### 2.1 后台进程启动

OpenCode **没有内置的守护进程管理**。需要使用外部工具：

```bash
# 使用 nohup
nohup opencode serve --port 8080 > /var/log/opencode.log 2>&1 &
echo $! > /var/run/opencode.pid

# 使用 systemd (Linux)
# /etc/systemd/system/opencode.service
```

```ini
[Unit]
Description=OpenCode Server
After=network.target

[Service]
Type=simple
Environment="OPENCODE_SERVER_PASSWORD=your-secure-password"
ExecStart=/usr/local/bin/opencode serve --port 8080 --hostname 0.0.0.0
Restart=always
User=opencode

[Install]
WantedBy=multi-user.target
```

```bash
# macOS LaunchAgent
# ~/Library/LaunchAgents/com.opencode.server.plist
```

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.opencode.server</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/opencode</string>
        <string>serve</string>
        <string>--port</string>
        <string>8080</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

### 2.2 进程状态查看

```bash
# 检查进程
ps aux | grep opencode

# 检查端口
lsof -i :8080
netstat -tlnp | grep 8080

# 测试服务器
# 返回 HTML 界面（SPA）
curl http://localhost:8080/

# 检查日志
ls -la ~/.local/share/opencode/log/
cat ~/.local/share/opencode/log/$(date +%Y-%m-%d)*.log
```

### 2.3 优雅关闭

```bash
# 通过 PID
kill -TERM <pid>
# 或
kill -INT <pid>

# 强制关闭
kill -KILL <pid>
```

### 2.4 日志位置

| 类型 | 路径 |
|------|------|
| 日志文件 | `~/.local/share/opencode/log/YYYY-MM-DDTHHMMSS.log` |
| 数据存储 | `~/.local/share/opencode/storage/` |
| 配置文件 | `~/.config/opencode/` |
| 缓存 | `~/.cache/opencode/` |
| 状态 | `~/.local/state/opencode/` |

---

## 3. 通信协议规范

### 3.1 Serve 模式 - HTTP/WebSocket

**基础信息：**
- 协议：HTTP/1.1 或 HTTP/2 (自动协商)
- 默认绑定：127.0.0.1 (可通过 `--hostname` 修改)
- 认证：HTTP Basic Auth (通过 `OPENCODE_SERVER_PASSWORD`)

**端点列表：**

| 端点 | 方法 | 说明 |
|------|------|------|
| `/` | GET | Web 界面 (SPA) |
| `/api/*` | * | API 路由 (返回 SPA，由前端处理) |
| `/global/event` | GET | Server-Sent Events (全局事件) |
| `/socket.io/*` | * | Socket.io WebSocket (推测) |
| `/assets/*` | GET | 静态资源 |

**认证示例：**

```bash
# 设置密码
export OPENCODE_SERVER_PASSWORD="my-secure-password"

# 启动服务器
opencode serve --port 8080

# 访问 API
curl -u ":$OPENCODE_SERVER_PASSWORD" http://localhost:8080/api/sessions
```

### 3.2 ACP 模式 - JSON-RPC

**协议规范：**
- 传输：stdio (标准输入/输出)
- 格式：JSON-RPC 2.0
- 编码：UTF-8

**通信流程：**

```
Editor (Client)          OpenCode ACP (Server)
      |                           |
      |---> {"jsonrpc":"2.0",...} --->|
      |                           |
      |<--- {"jsonrpc":"2.0",...} <---|
      |                           |
```

**示例消息：**

```json
// 请求
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "capabilities": {}
  }
}

// 响应
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "capabilities": {
      "tools": [...],
      "agents": [...]
    }
  }
}
```

### 3.3 Server-Sent Events (SSE)

**端点：** `/global/event`

**事件流格式：**

```
event: message
data: {"type":"session.updated","payload":{...}}

event: message
data: {"type":"message.part.updated","payload":{...}}
```

**事件类型：**

| 事件类型 | 说明 |
|----------|------|
| `session.updated` | 会话状态更新 |
| `message.updated` | 消息更新 |
| `message.part.updated` | 消息部分内容更新 |
| `session.diff` | 会话差异 |
| `file.watcher.updated` | 文件系统变更 |
| `command.executed` | 命令执行 |

---

## 4. Attach/Detach 机制

### 4.1 Attach 流程

```bash
# 1. 启动服务器
opencode serve --port 8080 &
SERVER_PID=$!

# 2. Attach TUI 界面
opencode attach http://localhost:8080

# 或 Attach 并指定目录
opencode attach http://localhost:8080 --dir /path/to/project
```

**Attach 行为：**
- 启动 TUI 界面连接到远程服务器
- 共享服务器的状态和会话
- 多个客户端可以同时 attach 到同一服务器

### 4.2 Detach 机制

**OpenCode 没有显式的 detach 命令。**

Detach 方式：
- `Ctrl+C` / `Ctrl+D` - 关闭 TUI 客户端
- 关闭终端窗口
- 注意：这只会关闭客户端，服务器继续运行

### 4.3 多客户端场景

```
                    ┌─────────────────┐
                    │  OpenCode Server │
                    │   (Port 8080)   │
                    └────────┬────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
    ┌────▼────┐        ┌────▼────┐        ┌────▼────┐
    │ Client 1│        │ Client 2│        │ Client 3│
    │  (TUI)  │        │  (TUI)  │        │  (API)  │
    └─────────┘        └─────────┘        └─────────┘
```

---

## 5. 实际测试示例

### 5.1 基础后台服务测试

```bash
#!/bin/bash

# 启动服务器
echo "Starting OpenCode server..."
opencode serve --port 9876 --hostname 127.0.0.1 --print-logs &
SERVER_PID=$!
echo "Server PID: $SERVER_PID"

# 等待启动
sleep 3

# 测试连接
echo "Testing connection..."
curl -s http://127.0.0.1:9876/ | head -5

# 发送命令
echo "Sending command..."
opencode run --attach http://127.0.0.1:9876 -- "Say hello"

# 停止服务器
echo "Stopping server..."
kill $SERVER_PID
wait $SERVER_PID 2>/dev/null
echo "Done"
```

### 5.2 ACP 模式测试

```bash
#!/bin/bash

# ACP 模式通过 stdio 通信
# 测试 ACP 初始化流程

cat << 'EOF' | opencode acp
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
EOF
```

### 5.3 Python 客户端示例

```python
#!/usr/bin/env python3
"""
OpenCode HTTP 客户端示例
"""

import requests
import json
import os

class OpenCodeClient:
    def __init__(self, base_url="http://localhost:8080", password=None):
        self.base_url = base_url
        self.password = password
        self.session = requests.Session()
        if password:
            self.session.auth = ("", password)
    
    def health_check(self):
        """健康检查"""
        resp = self.session.get(f"{self.base_url}/")
        return resp.status_code == 200
    
    def stream_events(self):
        """监听 SSE 事件"""
        url = f"{self.base_url}/global/event"
        headers = {"Accept": "text/event-stream"}
        
        with self.session.get(url, headers=headers, stream=True) as resp:
            for line in resp.iter_lines():
                if line:
                    decoded = line.decode('utf-8')
                    if decoded.startswith('data: '):
                        data = json.loads(decoded[6:])
                        yield data

# 使用示例
if __name__ == "__main__":
    password = os.getenv("OPENCODE_SERVER_PASSWORD")
    client = OpenCodeClient("http://localhost:8080", password)
    
    if client.health_check():
        print("Server is running")
        
        # 监听事件
        for event in client.stream_events():
            print(f"Event: {event}")
    else:
        print("Server is not responding")
```

### 5.4 Node.js 客户端示例

```javascript
// opencode-client.js
const http = require('http');

class OpenCodeClient {
  constructor(baseUrl = 'http://localhost:8080', password = null) {
    this.baseUrl = new URL(baseUrl);
    this.password = password;
  }

  async request(path, method = 'GET', data = null) {
    const options = {
      hostname: this.baseUrl.hostname,
      port: this.baseUrl.port,
      path: path,
      method: method,
      headers: {}
    };

    if (this.password) {
      const auth = Buffer.from(`:${this.password}`).toString('base64');
      options.headers['Authorization'] = `Basic ${auth}`;
    }

    return new Promise((resolve, reject) => {
      const req = http.request(options, (res) => {
        let data = '';
        res.on('data', chunk => data += chunk);
        res.on('end', () => resolve({ status: res.statusCode, data }));
      });

      req.on('error', reject);
      if (data) req.write(JSON.stringify(data));
      req.end();
    });
  }

  async healthCheck() {
    const resp = await this.request('/');
    return resp.status === 200;
  }
}

// 使用示例
const client = new OpenCodeClient(
  'http://localhost:8080',
  process.env.OPENCODE_SERVER_PASSWORD
);

client.healthCheck().then(ok => {
  console.log(ok ? 'Server is running' : 'Server is not responding');
});
```

---

## 6. 潜在问题和注意事项

### 6.1 安全注意事项

| 问题 | 风险 | 解决方案 |
|------|------|----------|
| 未设置密码 | 服务器完全开放 | 始终设置 `OPENCODE_SERVER_PASSWORD` |
| 绑定 0.0.0.0 | 暴露到公网 | 使用反向代理 + TLS |
| 无访问控制 | 任何客户端可连接 | 使用防火墙限制 IP |

**安全配置示例：**

```bash
# 强密码
export OPENCODE_SERVER_PASSWORD=$(openssl rand -base64 32)

# 使用 TLS 反向代理 (nginx)
# 仅监听本地
opencode serve --port 8080 --hostname 127.0.0.1
```

### 6.2 进程管理限制

- **无内置 PID 文件管理** - 需要外部工具
- **无自动重启** - 需要 systemd/supervisord
- **无状态持久化保证** - 服务器崩溃可能丢失未保存状态

### 6.3 资源管理

```bash
# 查看资源使用
ps aux | grep opencode
lsof -p <pid>

# 限制资源 (使用 systemd)
# MemoryLimit=1G
# CPUQuota=200%
```

### 6.4 会话管理

- 会话自动保存到 `~/.local/share/opencode/storage/`
- 可通过 `opencode session list` 查看
- 可通过 `opencode export <id>` 导出

### 6.5 与其他工具的对比

| 特性 | OpenCode | Claude Code | Codex CLI |
|------|----------|-------------|-----------|
| 后台模式 | `serve` | `-p` 参数 | 类似 |
| Socket 类型 | HTTP/WebSocket | Unix Socket | ? |
| Attach 命令 | `attach <url>` | `attach <pid>` | 类似 |
| 协议 | HTTP/JSON-RPC | 自定义 | ? |

---

## 7. 总结与建议

### 7.1 关键发现

1. **OpenCode 没有 `-p` 参数** - 用户使用错误信息
2. **使用 HTTP 而非 Unix Socket** - 更易于网络访问但需要注意安全
3. **ACP 协议是 JSON-RPC over stdio** - 主要用于编辑器集成
4. **无内置进程管理** - 需要 systemd/launchd/supervisord

### 7.2 推荐架构

对于持久化 Agent 实现，建议：

```
┌─────────────────────────────────────────────┐
│           Systemd / Launchd                 │
│  ┌───────────────────────────────────────┐  │
│  │      OpenCode Serve (Port 8080)       │  │
│  │  ┌─────────────────────────────────┐  │  │
│  │  │    Session Persistence          │  │  │
│  │  │  ~/.local/share/opencode/...    │  │  │
│  │  └─────────────────────────────────┘  │  │
│  └───────────────────────────────────────┘  │
└──────────────────┬──────────────────────────┘
                   │ HTTP/WebSocket
    ┌──────────────┼──────────────┐
    ▼              ▼              ▼
┌────────┐    ┌────────┐    ┌────────┐
│ Client │    │ Client │    │ Client │
│  (TUI) │    │  (API) │    │(Editor)│
└────────┘    └────────┘    └────────┘
```

### 7.3 实施建议

1. **使用 `opencode serve` 作为后台进程**
2. **使用 systemd/launchd 管理进程生命周期**
3. **使用 nginx/Caddy 作为 TLS 反向代理**
4. **实现自定义客户端替代 `attach` 命令**

---

## 附录 A: 环境变量

| 变量 | 说明 |
|------|------|
| `OPENCODE_SERVER_PASSWORD` | 服务器认证密码 |
| `OPENCODE_API_KEY` | API 密钥 |
| `OPENCODE_MODEL` | 默认模型 |
| `OPENCODE_DIR` | 配置目录覆盖 |

## 附录 B: 配置文件路径

| 类型 | 路径 |
|------|------|
| 主配置 | `~/.config/opencode/opencode.json` |
| 替代配置 | `~/.config/opencode/opencode.jsonc` |
| 项目配置 | `./.opencode/config.json` |
| 全局 Agent | `~/.config/opencode/agents/` |

---

*调研日期: 2026-02-09*  
*OpenCode 版本: 1.1.53*  
*调研工具: Shell 测试 + 官方文档*
