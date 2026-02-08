# CIS 与 Element 集成指南

本文档介绍如何使用 Element（Matrix 客户端）连接 CIS 节点，实现图形化界面管理。

---

## 目录

- [概述](#概述)
- [安装 Element](#安装-element)
- [启动 CIS Matrix 服务器](#启动-cis-matrix-服务器)
- [连接 Element 到 CIS](#连接-element-到-cis)
- [功能说明](#功能说明)
- [故障排除](#故障排除)

---

## 概述

CIS 实现了 Matrix 协议的服务器端，允许使用 Element 等 Matrix 客户端进行连接。通过 Element，你可以：

- 💬 与 CIS 节点进行即时通讯
- 📋 查看和管理 DAG 任务
- 🔐 管理节点信任和网络 ACL
- 📊 查看系统状态和日志

### 架构

```
┌─────────────┐      Matrix API      ┌─────────────┐
│   Element   │ ◄──────────────────► │  CIS Node   │
│  (Client)   │   HTTP/WebSocket     │  (Server)   │
│             │      Port 7676       │             │
└─────────────┘                      └─────────────┘
```

---

## 安装 Element

### macOS

```bash
# 使用 Homebrew
brew install --cask element

# 或从官网下载
open https://element.io/download
```

### Linux

```bash
# Ubuntu/Debian (官方仓库)
sudo apt install element-desktop

# 或使用 Flatpak
flatpak install flathub im.riot.Riot

# 或使用 Snap
sudo snap install element-desktop
```

### Windows

从官网下载安装程序：
```bash
start https://element.io/download
```

### 验证安装

```bash
# 检查 CIS 是否能检测到 Element
cis matrix detect
```

预期输出：
```
✅ Found 1 Element app(s):

   [1] Element
       Path: /Applications/Element.app/Contents/MacOS/Element
       Bundle: /Applications/Element.app
```

---

## 启动 CIS Matrix 服务器

### 基本启动

```bash
# 启动 Matrix 服务器（前台运行）
cis matrix start
```

服务器将在 `http://localhost:7676` 启动。

### 自动启动 Element

```bash
# 启动服务器并自动打开 Element
cis matrix start --launch
```

### 指定端口

```bash
# 使用自定义端口
cis matrix start --port 8080
```

### 后台运行（开发中）

```bash
# 以后台模式启动（当前版本会显示警告，实际仍为前台）
cis matrix start --daemon
```

---

## 连接 Element 到 CIS

### 第一步：启动 CIS Matrix 服务器

在终端中运行：

```bash
cis matrix start
```

看到以下输出表示启动成功：
```
🚀 Starting Matrix server...
   Port: 7676
   URL: http://localhost:7676

📡 Matrix server is ready!
   Clients can connect to: http://localhost:7676

💡 Connection info for Element:
   Homeserver URL: http://localhost:7676
```

### 第二步：配置 Element

1. **打开 Element 应用**

   如果使用了 `--launch` 参数，Element 会自动打开。

2. **编辑服务器配置**

   - 在登录界面点击 **"Edit"**（编辑）
   - 或者点击服务器名称旁的齿轮图标

3. **输入 Homeserver URL**

   ```
   http://localhost:7676
   ```

4. **点击 "Continue"**（继续）

   Element 会验证服务器连接。

### 第三步：登录

CIS 使用 DID 身份验证：

1. **用户名格式**
   ```
   @did:cis:<node_id>:<public_key_short>
   ```

   例如：
   ```
   @did:cis:abc123:def456
   ```

2. **获取你的 DID**

   ```bash
   cis status
   ```

   查看输出中的 `DID` 字段。

3. **密码**

   当前版本使用空密码或任意密码（开发中）。

4. **点击 "Sign In"**（登录）

---

## 功能说明

### 支持的 Matrix 功能

| 功能 | 状态 | 说明 |
|------|------|------|
| 文本消息 | ✅ 可用 | 基本的聊天功能 |
| 房间管理 | ✅ 可用 | 创建/加入房间 |
| 用户状态 | ⚠️ 部分 | 在线状态显示 |
| 文件传输 | ❌ 未实现 | 计划中 |
| 端到端加密 | ❌ 未实现 | 计划中 |

### CIS 特定功能

通过 Element 可以：

1. **查看 DAG 任务**
   - 加入 `!dag:localhost` 房间
   - 查看任务列表和状态

2. **管理节点**
   - 加入 `!nodes:localhost` 房间
   - 查看已连接的节点

3. **系统通知**
   - 任务完成通知
   - 节点连接/断开通知

---

## 故障排除

### 问题：Element 无法连接

**症状**："Cannot reach homeserver"

**检查步骤**：

1. **确认 CIS Matrix 服务器已启动**
   ```bash
   cis matrix test
   ```

2. **检查端口占用**
   ```bash
   lsof -i :7676
   ```

3. **检查防火墙**
   ```bash
   # macOS
   sudo /usr/libexec/ApplicationFirewall/socketfilterfw --list

   # Linux
   sudo ufw status
   ```

4. **尝试重启服务器**
   ```bash
   # Ctrl+C 停止当前服务器
   # 然后重新启动
   cis matrix start
   ```

### 问题：登录失败

**症状**："Invalid username or password"

**解决方案**：

1. **确认 DID 格式正确**
   - 必须以 `@` 开头
   - 格式：`@did:cis:<node_id>:<key>`

2. **查看正确的 DID**
   ```bash
   cis network status
   ```

3. **尝试空密码**
   - 当前版本可能接受空密码

### 问题：Element 未检测到

**症状**：`cis matrix detect` 显示未找到

**解决方案**：

1. **确认 Element 已安装**
   ```bash
   # macOS
   ls /Applications/Element.app

   # Linux
   which element-desktop
   ```

2. **手动指定路径启动**
   ```bash
   # macOS
   open /Applications/Element.app

   # Linux
   element-desktop &
   ```

3. **重新安装 Element**
   ```bash
   # macOS
   brew reinstall --cask element
   ```

### 问题：连接后无响应

**症状**：Element 显示连接但无内容

**检查**：

1. **查看 CIS 日志**
   ```bash
   tail -f ~/.cis/logs/cis.log
   ```

2. **确认房间已创建**
   ```bash
   cis matrix test
   ```

3. **重启 Element**
   - 完全退出 Element
   - 重新打开并连接

---

## 高级配置

### 自定义端口

如果 7676 端口被占用：

```bash
# 使用 8080 端口
cis matrix start --port 8080
```

然后在 Element 中连接：
```
http://localhost:8080
```

### 远程访问

要从其他机器访问：

1. **绑定到所有接口**
   ```bash
   # 编辑配置
   cis config edit
   ```

   添加：
   ```toml
   [matrix]
   bind_address = "0.0.0.0"
   port = 7676
   ```

2. **使用实际 IP 连接**
   ```
   http://<cis-node-ip>:7676
   ```

3. **注意安全性**
   - 确保防火墙只允许受信任的 IP
   - 使用 `cis network allow` 添加信任的 DID

---

## 命令速查

| 命令 | 说明 |
|------|------|
| `cis matrix detect` | 检测 Element 安装 |
| `cis matrix start` | 启动 Matrix 服务器 |
| `cis matrix start --launch` | 启动并打开 Element |
| `cis matrix status` | 查看状态 |
| `cis matrix test` | 测试连接 |
| `cis matrix stop` | 停止服务器 |

---

## 获取帮助

- **GitHub Issues**: https://github.com/MoSiYuan/CIS/issues
- **Matrix 房间**: `#cis:matrix.org`

---

**最后更新**: 2026-02-07  
**CIS 版本**: 0.1.0  
**Element 版本**: 1.11.0+
