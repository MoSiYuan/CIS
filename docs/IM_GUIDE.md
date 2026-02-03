# CIS 即时通讯 (IM) 使用指南

## 概述

CIS IM 提供分布式即时通讯能力，支持：
- 一对一私聊
- 群组聊天
- 频道广播
- 语义搜索历史消息
- 跨设备同步

## 快速开始

### 启动 IM 服务

```bash
# IM 已内置于 CIS，无需单独启动
cis init
```

### 基本命令

```bash
# 创建群组会话
cis im create "开发团队" --type group --participants user1,user2,user3

# 发送消息
cis im send session-abc123 "大家好，项目进展如何？"

# 查看会话列表
cis im list --limit 20

# 查看消息历史
cis im history session-abc123 --limit 50

# 语义搜索消息
cis im search "项目截止日期" --session session-abc123

# 标记已读
cis im read session-abc123 --all
```

## 会话类型

### Direct (私聊)
一对一私密对话，消息端到端加密。

```bash
cis im create "张三" --type direct --participants zhangsan
```

### Group (群组)
多人讨论组，支持 2-500 人。

```bash
cis im create "产品讨论" --type group --participants alice,bob,charlie
```

### Channel (频道)
广播式通讯，支持无限订阅者，适合公告、日志等场景。

```bash
cis im create "系统公告" --type channel
```

## 消息类型

### 文本消息
```bash
cis im send session-xxx "普通文本消息"
```

### 富文本
支持 Markdown、代码块、提及(@用户)。

### 文件分享
```bash
cis im send session-xxx --file ./document.pdf
```

### 图片
```bash
cis im send session-xxx --image ./screenshot.png
```

## 语义搜索

IM 集成 CIS Vector Intelligence，支持语义搜索：

```bash
# 搜索"项目进度"相关消息（不仅匹配关键词）
cis im search "项目进度" --session session-xxx

# 使用向量相似度搜索
cis im search "上周讨论的API设计" --semantic
```

## 会话管理

### 添加参与者
```bash
cis im participant add session-xxx newuser
```

### 移除参与者
```bash
cis im participant remove session-xxx olduser
```

### 设置权限
```bash
cis im role set session-xxx user admin
```

角色：owner > admin > member > guest

## 数据同步

### 跨设备同步
IM 数据自动通过 P2P 网络同步：
- 私域记忆：本地存储，不跨设备
- 公域记忆：同步到所有节点

### 离线消息
消息在本地队列缓存，联网后自动发送。

## 配置

编辑 `~/.cis/config.toml`：

```toml
[im]
# 消息保留天数
message_retention_days = 365

# 启用端到端加密
enable_e2ee = true

# 自动下载文件
auto_download_files = false

# 最大文件大小 (MB)
max_file_size = 100

# 语义搜索阈值
semantic_search_threshold = 0.7
```

## API 使用

```rust
use cis_core::im::{ImContext, MessageContent};

// 创建会话
let session_id = im.create_session(
    SessionType::Group,
    "开发团队",
    vec!["user1", "user2"]
).await?;

// 发送消息
im.send_message(
    &session_id,
    MessageContent::Text("Hello".to_string())
).await?;

// 语义搜索
let results = im.search_messages(
    "项目进度",
    Some(&session_id),
    10
).await?;
```

## 故障排除

### 消息发送失败
- 检查网络连接: `cis doctor --network`
- 检查 P2P 状态: `cis p2p status`

### 同步延迟
- 查看同步队列: `cis p2p sync-queue`
- 手动触发同步: `cis p2p sync --force`

### 搜索无结果
- 确认消息已索引: `cis im index --check`
- 重建索引: `cis im index --rebuild`

## 安全

- 私聊默认启用 E2EE
- 群组消息可选择加密
- 支持消息自毁定时
