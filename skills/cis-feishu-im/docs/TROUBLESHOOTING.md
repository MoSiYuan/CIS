# CIS 飞书机器人 - 问题排查指南

## 🎯 问题：机器人无法对话

### ✅ 诊断结果

**系统状态**：
- ✅ 应用已创建（App ID: `cli_a90a99e490f95cc7`）
- ✅ 应用已发布（版本 0.1.0）
- ✅ 权限已申请（`im:message`, `im:chat`）
- ✅ API 连接正常
- ✅ 访问令牌获取成功
- ✅ 轮询器正在运行

**问题所在**：
- ❌ **会话列表为空** - 机器人还没有被添加到任何对话

---

## 💡 解决方案

### 步骤 1: 在飞书中找到并添加机器人

#### 方法 A: 通过搜索添加

1. **打开飞书 PC 端**
2. **点击左上角搜索框**
3. **搜索以下内容之一**：
   - `CIS AI Assistant`
   - `cli_a90a99e490f95cc7`
   - 您创建应用时填写的名称

4. **点击搜索结果中的机器人**
5. **点击「添加好友」或「发消息」**

#### 方法 B: 通过应用商店添加

1. **访问飞书应用商店**：
   ```
   https://open.feishu.cn/app/cli_a90a99e490f95cc7
   ```

2. **点击「添加到工作台」**
3. **在飞书中打开应用**

---

### 步骤 2: 发起对话测试

#### 测试私聊（推荐）

1. **打开与机器人的私聊**
2. **发送测试消息**：
   ```
   你好
   ```

3. **查看是否收到回复**（可能需要等待最多 10 秒）

#### 测试群聊

1. **将机器人添加到群聊**：
   - 打开群聊 → 设置 → 添加机器人
   - 搜索 `CIS AI Assistant`
   - 点击添加

2. **在群聊中 @机器人**：
   ```
   @CIS AI Assistant 你好
   ```

---

### 步骤 3: 验证轮询器工作

#### 查看实时日志

```bash
# 在另一个终端窗口运行
tail -f /tmp/feishu_poller.log
```

**预期看到**（收到消息时）：
```
✅ 收到新消息: 你好
📨 处理消息中...
💭 AI 回复: 你好！我是CIS AI助手...
✅ 回复已发送
```

#### 重新运行 API 测试

```bash
cargo run --example test_api
```

**预期看到**（添加机器人后）：
```
✅ 会话列表获取成功: 1 个会话
  1. CIS AI Assistant
     ID: oc_xxxxx
     类型: p2p
```

---

## 🔍 如果仍然无法对话

### 检查清单

#### 1. 确认机器人已添加

在飞书中：
- [ ] 可以搜索到机器人
- [ ] 已经发起私聊或添加到群聊
- [ ] 机器人状态显示为在线

#### 2. 确认轮询器运行

在终端中：
```bash
ps aux | grep run_poller | grep -v grep
```

应该看到进程在运行。如果没有，重新启动：
```bash
cargo run --example run_poller
```

#### 3. 确认配置正确

```bash
cat ~/.cis/config/feishu_im.toml | grep -E "app_id|app_secret|runtime_mode"
```

应该显示：
```
app_id = "cli_a90a99e490f95cc7"
app_secret = "bfwq9pZbbPNddQwU8MpKwdM2bZetMxas"
runtime_mode = "both"  # 或 "polling_only"
```

#### 4. 重新测试 API

```bash
cargo run --example test_api
```

查看会话列表是否为空。如果不为空但仍然无法对话，继续排查。

---

### 高级排查

#### 检查飞书开放平台配置

访问以下页面确认配置：

1. **权限管理**：
   ```
   https://open.feishu.cn/app/cli_a90a99e490f95cc7/permission
   ```
   - ✅ `im:message` 权限已开通
   - ✅ `im:chat` 权限已开通

2. **版本发布**：
   ```
   https://open.feishu.cn/app/cli_a90a99e490f95cc7/release
   ```
   - ✅ 有已发布的版本
   - ✅ 版本状态为「可用」

3. **机器人能力**：
   ```
   https://open.feishu.cn/app/cli_a90a99e490f95cc7/bot
   ```
   - ✅ 机器人能力已启用
   - ✅ 机器人状态为「在线」

---

## 📊 轮询器工作原理

### 轮询间隔

- **默认间隔**: 10 秒
- **配置位置**: `~/.cis/config/feishu_im.toml`
  ```toml
  [polling]
  http_interval = 10  # 秒
  ```

### 工作流程

```
每 10 秒：
  1. 获取会话列表
  2. 对每个会话：
     - 获取新消息（last_message_time 之后）
     - 如果有新消息：
       - 处理消息（调用 AI）
       - 发送回复
       - 更新会话状态
```

### 冷冻模式

当前配置为冷冻模式：
```toml
process_history = false
```

- **含义**: 不处理轮询器启动之前的消息
- **好处**: 避免处理历史消息导致重复回复
- **注意**: 轮询器关闭期间的消息会被丢弃

---

## 🎯 成功标志

当一切正常工作时，您应该看到：

### 飞书客户端
- ✅ 机器人显示为「在线」
- ✅ 发送消息后 10 秒内收到回复
- ✅ 回复内容与您的提问相关

### 轮询器日志
```
✅ 收到新消息
📨 处理消息中...
💭 AI 回复: [回复内容]
✅ 回复已发送: [message_id]
```

### API 测试
```bash
$ cargo run --example test_api

✅ 会话列表获取成功: 1 个会话
  1. CIS AI Assistant
     ID: oc_xxxxx
     类型: p2p

✅ 消息获取成功: 2 条消息
  - user_xxxxx: 你好
  - bot_xxxxx: 你好！我是CIS AI助手...
```

---

## 📞 获取帮助

如果按照上述步骤操作后仍然无法对话：

1. **收集诊断信息**：
   ```bash
   # 保存日志
   cp /tmp/feishu_poller.log /tmp/feishu_debug.log

   # 运行 API 测试
   cargo run --example test_api > /tmp/api_test.log 2>&1
   ```

2. **检查常见问题**：
   - [常见问题文档](./COMMON_ISSUES.md)
   - [架构设计文档](./ARCHITECTURE_V2.md)
   - [会话管理文档](./SESSION_INTEGRATION.md)

3. **查看详细日志**：
   ```bash
   # 启用调试日志
   export RUST_LOG=debug
   cargo run --example run_poller
   ```

---

## ✅ 快速检查清单

使用以下命令快速检查系统状态：

```bash
# 1. 检查轮询器是否运行
echo "=== 检查轮询器进程 ==="
ps aux | grep run_poller | grep -v grep

# 2. 检查配置文件
echo -e "\n=== 检查配置文件 ==="
cat ~/.cis/config/feishu_im.toml | grep -v "app_secret"

# 3. 测试 API 连接
echo -e "\n=== 测试 API 连接 ==="
cargo run --example test_api 2>&1 | grep -E "✅|❌|⚠️"

# 4. 查看最新日志
echo -e "\n=== 最新日志（最后 10 行）==="
tail -10 /tmp/feishu_poller.log 2>/dev/null || echo "日志文件不存在"
```

运行以上命令，如果所有检查都通过（显示 ✅），说明系统配置正确。

**下一步**：在飞书中搜索并添加机器人，发起私聊测试。
