# CIS 飞书 IM Skill - 完整设置指南

本指南将引导你完成飞书机器人集成的完整流程，从本地开发到生产部署。

## 📋 目录

1. [前置准备](#前置准备)
2. [本地开发环境设置](#本地开发环境设置)
3. [飞书开放平台配置](#飞书开放平台配置)
4. [配置文件填写](#配置文件填写)
5. [启动服务](#启动服务)
6. [测试与验证](#测试与验证)
7. [生产环境部署](#生产环境部署)

---

## 前置准备

### 系统要求

- Rust 1.70+
- macOS/Linux (Windows 支持通过 WSL)
- 飞书账号（企业或个人）

### 已完成的步骤

✅ 飞书应用已创建
- App ID: `cli_a90a99e490f95cc7`
- App Secret: `bfwq9pZbbPNddQwU8MpKwdM2bZetMxas`

---

## 本地开发环境设置

由于飞书事件订阅需要可访问的 Webhook URL，本地开发需要使用内网穿透工具。

### 方案一：使用 ngrok（推荐）

#### 1. 安装 ngrok

```bash
# macOS
brew install ngrok

# Linux
curl -s https://ngrok-agent.s3.amazonaws.com/ngrok.asc | \
  sudo tee /etc/apt/trusted.gpg.d/ngrok.asc >/dev/null && \
  echo "deb https://ngrok-agent.s3.amazonaws.com buster main" | \
  sudo tee /etc/apt/sources.list.d/ngrok.list && \
  sudo apt update && sudo apt install ngrok
```

#### 2. 配置 ngrok authtoken（首次使用）

访问 https://dashboard.ngrok.com/get-started/your-authtoken 获取 authtoken

```bash
ngrok config add-authtoken <your-authtoken>
```

#### 3. 启动 ngrok 隧道

```bash
# 在一个终端窗口中运行
ngrok http 8080
```

你会看到类似输出：

```
Forwarding  https://xxxx-xx-xx-xx-xx.ngrok-free.app -> http://localhost:8080
```

**重要**: 复制这个 `https://xxxx.ngrok-free.app` 地址，这是你的公网 Webhook URL。

#### 4. 保存 Webhook URL

你的完整 Webhook URL 将是：
```
https://xxxx-xx-xx-xx-xx.ngrok-free.app/webhook/feishu
```

### 方案二：使用本地tunnel（备选）

```bash
# 安装
npm install -g localtunnel

# 启动
lt --port 8080

# 会得到: https://random-name.loca.lt
```

### 方案三：使用 Cloudflare Tunnel（长期推荐）

```bash
# 安装 cloudflared
brew install cloudflared

# 登录
cloudflared tunnel login

# 创建隧道
cloudflared tunnel create cis-feishu-dev

# 启动隧道
cloudflared tunnel --url http://localhost:8080
```

---

## 飞书开放平台配置

### 步骤 1：申请必要权限

访问 https://open.feishu.cn/app/[你的AppID]/app/app

在「权限管理」中申请以下权限：

| 权限名称 | 权限值 | 用途 |
|---------|-------|------|
| 获取与发送消息 | `im:message` | 接收和发送消息 |
| 获取群组信息 | `im:message.group_at_msg` | 群聊 @ 机器人 |
| 获取并读取用户信息 | `im:chat` | 读取聊天信息 |
| 获取用户与机器人会话 | `im:conversation` | 私聊对话 |

**重要**:
- 逐个申请权限
- 选择「全员可访问」或指定测试用户
- 等待审批通过（个人应用通常即时通过）

### 步骤 2：配置事件订阅

访问 https://open.feishu.cn/app/[你的AppID]/event

#### 2.1 订阅「接收消息」事件

1. 点击「添加事件」
2. 选择 `im.message.receive_v1`
3. 填写请求地址：
   ```
   https://xxxx.ngrok-free.app/webhook/feishu
   ```
4. 点击「保存」

#### 2.2 生成加密凭证

在保存事件订阅后，飞书会生成：
- **Encrypt Key**: 用于解密事件内容的密钥
- **Verification Token**: 用于验证请求来源的令牌

**重要**: 这两个值只会显示一次，请立即复制保存！

### 步骤 3：（可选）订阅其他事件

| 事件 | 事件类型 | 用途 |
|------|---------|------|
| 群成员加入 | `im.chat.member.added_v1` | 新成员欢迎消息 |
| 群成员移除 | `im.chat.member.removed_v1` | 成员离开处理 |

### 步骤 4：发布机器人

1. 访问 https://open.feishu.cn/app/[你的AppID]/bot
2. 点击「发布」或「更新版本」
3. 填写机器人简介和头像
4. 提交发布（个人应用即时生效）

---

## 配置文件填写

### 步骤 1：初始化配置

```bash
cd skills/cis-feishu-im/config
bash ../scripts/init-config.sh
```

这会创建 `~/.cis/config/feishu_im.toml` 配置文件。

### 步骤 2：编辑配置文件

```bash
nano ~/.cis/config/feishu_im.toml
```

填写完整的配置：

```toml
# ==================== 飞书应用配置 ====================
app_id = "cli_a90a99e490f95cc7"
app_secret = "bfwq9pZbbPNddQwU8MpKwdM2bZetMxas"

# 从飞书开放平台事件订阅页面获取
encrypt_key = "从飞书复制粘贴这里"
verify_token = "从飞书复制粘贴这里"

# Webhook 签名验证（推荐开启）
verify_signature = true

# ==================== 对话触发模式 ====================
# 推荐使用: private_and_at_mention
trigger_mode = "private_and_at_mention"

# ==================== AI Provider 配置 ====================
[ai_provider]
provider_type = "claude"  # 或 "kimi"

# ==================== 对话上下文配置 ====================
[context_config]
persist_context = true        # 持久化对话历史
max_turns = 20               # 最大轮次
context_timeout_secs = 1800  # 30分钟超时
sync_to_memory = true        # 同步到记忆系统

# ==================== 数据库路径配置 ====================
im_db_path = "~/.cis/data/feishu_im.db"
memory_db_path = "~/.cis/data/memory.db"

# ==================== Webhook 服务器配置 ====================
[webhook]
bind_address = "0.0.0.0"
port = 8080
path = "/webhook/feishu"

# ==================== 高级配置 ====================
message_timeout = 30
max_response_length = 2000
debug = true  # 开发环境建议开启
```

### 步骤 3：验证配置

检查配置文件语法：

```bash
# 简单检查
grep -E "^(app_id|app_secret|encrypt_key|verify_token)" ~/.cis/config/feishu_im.toml

# 确保没有空字符串（除了 debug 等可选字段）
```

---

## 启动服务

### 方式一：使用 cis-node（推荐）

```bash
cd CIS
cargo run --bin cis-node -- --skill cis-feishu-im
```

### 方式二：直接运行 Skill 测试

```bash
cd skills/cis-feishu-im
cargo run --example feishu_bot
```

### 方式三：使用脚本启动

创建启动脚本 `start-feishu-skill.sh`：

```bash
#!/bin/bash
set -e

echo "🚀 启动 CIS 飞书 IM Skill"

# 确保配置文件存在
if [ ! -f ~/.cis/config/feishu_im.toml ]; then
    echo "❌ 配置文件不存在，请先运行 init-config.sh"
    exit 1
fi

# 确保数据目录存在
mkdir -p ~/.cis/data
mkdir -p ~/.cis/logs

# 启动服务
cd "$(dirname "$0")/../.."
cargo run --bin cis-node -- --skill cis-feishu-im
```

使用：

```bash
chmod +x start-feishu-skill.sh
./start-feishu-skill.sh
```

---

## 测试与验证

### 步骤 1：检查 Webhook 服务

启动服务后，你应该看到：

```
✅ FeishuImSkill 初始化成功
📡 Webhook 服务器启动: http://0.0.0.0:8080/webhook/feishu
```

### 步骤 2：测试本地端点

```bash
# 健康检查
curl http://localhost:8080/health

# 应该返回: {"status":"ok"}
```

### 步骤 3：通过飞书发送测试消息

1. **在飞书中找到你的机器人**
   - 搜索你的机器人名称
   - 或直接访问：https://open.feishu.cn/app/[你的AppID]/bot

2. **发送私聊消息**
   ```
   你好
   ```
   或
   ```
   @机器人 你好
   ```

3. **检查日志输出**

你应该看到：

```
[INFO] 收到飞书事件: im.message.receive_v1
[INFO] 消息类型: text
[INFO] 发送者: ou_xxxxx
[INFO] 触发模式: private_and_at_mention (私聊)
[INFO] 用户消息: 你好
[INFO] AI 回复中...
[INFO] 发送飞书消息成功
```

### 步骤 4：验证对话持久化

```bash
# 检查数据库
sqlite3 ~/.cis/data/feishu_im.db

sqlite> .tables
conversations sessions messages

sqlite> SELECT * FROM sessions;
1|ou_xxxxx|1704067200000|1704067260000|active

sqlite> SELECT * FROM messages;
1|1|user|你好|1704067200000
2|1|assistant|你好！有什么可以帮助你的吗？|1704067260000
```

### 步骤 5：测试群聊 @ 机器人

1. 创建一个测试群聊
2. 添加你的机器人到群聊
3. 发送：`@机器人 帮我总结一下`

### 常见问题排查

#### 问题 1: 没有收到消息

**检查**:
1. ngrok 是否正常运行？访问 ngrok URL 是否显示 "Tunnel xxx.ngrok-free.app not found"
   - **原因**: Webhook 服务未启动
   - **解决**: 确保 `cargo run --bin cis-node -- --skill cis-feishu-im` 正在运行

2. 飞书事件订阅配置的 URL 是否正确？
   - **原因**: URL 拼写错误或缺少路径
   - **解决**: 确保 URL 是 `https://xxxx.ngrok-free.app/webhook/feishu`（包含路径）

3. Encrypt Key 和 Verify Token 是否正确？
   - **原因**: 复制时包含空格或换行符
   - **解决**: 重新从飞书控制台复制，注意去掉引号

#### 问题 2: 收到消息但机器人不回复

**检查**:
```bash
# 查看详细日志
RUST_LOG=debug cargo run --bin cis-node -- --skill cis-feishu-im
```

可能原因：
1. `trigger_mode` 设置为 `at_mention_only`，但收到的是私聊消息
2. AI Provider 配置错误（Claude CLI 未安装或配置）
3. 飞书 API 权限不足

#### 问题 3: AI 回复报错

**Claude CLI 相关**:
```bash
# 测试 Claude CLI
claude "你好"

# 如果报错，配置 Claude CLI
claude auth login
```

**Kimi 相关**:
```bash
# 确保 KIMI_API_KEY 环境变量已设置
echo $KIMI_API_KEY
```

---

## 生产环境部署

### 方案一：云服务器部署

#### 1. 准备服务器

推荐配置：
- CPU: 2核+
- 内存: 4GB+
- 带宽: 5Mbps+
- 操作系统: Ubuntu 22.04 LTS

#### 2. 配置域名和 HTTPS

使用 Nginx 反向代理 + Let's Encrypt：

```nginx
# /etc/nginx/sites-available/cis-feishu
server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl;
    server_name your-domain.com;

    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;

    location /webhook/feishu {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

#### 3. 使用 systemd 管理服务

创建 `/etc/systemd/system/cis-feishu.service`：

```ini
[Unit]
Description=CIS Feishu IM Skill
After=network.target

[Service]
Type=simple
User=cis
WorkingDirectory=/opt/cis
ExecStart=/opt/cis/cis-node --skill cis-feishu-im
Restart=always
RestartSec=10
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

启动服务：

```bash
sudo systemctl daemon-reload
sudo systemctl enable cis-feishu
sudo systemctl start cis-feishu
sudo systemctl status cis-feishu
```

#### 4. 更新飞书配置

将 Webhook URL 更新为生产域名：
```
https://your-domain.com/webhook/feishu
```

### 方案二：Docker 部署

创建 `Dockerfile`：

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin cis-node

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates sqlite3
COPY --from=builder /app/target/release/cis-node /usr/local/bin/
EXPOSE 8080
CMD ["cis-node", "--skill", "cis-feishu-im"]
```

构建和运行：

```bash
docker build -t cis-feishu:latest .
docker run -d \
  --name cis-feishu \
  -p 8080:8080 \
  -v ~/.cis:/root/.cis \
  --restart unless-stopped \
  cis-feishu:latest
```

### 方案三：Kubernetes 部署

创建 `k8s/deployment.yaml`：

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: cis-feishu
spec:
  replicas: 2
  selector:
    matchLabels:
      app: cis-feishu
  template:
    metadata:
      labels:
        app: cis-feishu
    spec:
      containers:
      - name: cis-feishu
        image: cis-feishu:latest
        ports:
        - containerPort: 8080
        env:
        - name: RUST_LOG
          value: "info"
        volumeMounts:
        - name: config
          mountPath: /root/.cis
      volumes:
      - name: config
        persistentVolumeClaim:
          claimName: cis-config-pvc
---
apiVersion: v1
kind: Service
metadata:
  name: cis-feishu-service
spec:
  selector:
    app: cis-feishu
  ports:
  - port: 443
    targetPort: 8080
  type: LoadBalancer
```

---

## 下一步

完成基础设置后，你可以：

1. **自定义对话行为**: 修改 `src/webhook.rs` 中的消息处理逻辑
2. **添加命令支持**: 实现 `/help`, `/status` 等命令
3. **集成记忆系统**: 配置 `sync_to_memory = true` 启用长期记忆
4. **监控和日志**: 集成 Prometheus + Grafana 监控
5. **扩展功能**: 添加更多事件处理（文件上传、卡片交互等）

---

## 参考资源

- [飞书开放平台文档](https://open.feishu.cn/document)
- [larkrs-client 文档](https://docs.rs/larkrs-client)
- [CIS 项目文档](../../README.md)
- [CIS 架构设计](../../docs/ARCHITECTURE_V2.md)

---

**问题反馈**: 如遇到问题，请提交 Issue 到 CIS 项目仓库。
