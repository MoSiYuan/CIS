# CIS 作为 Agent 安装的 Skill 引导

## 概述

CIS 可以作为 AI Agent (Claude/Kimi 等) 的 Skill 安装，为 Agent 提供：
- 长期记忆存储与检索
- 任务执行与跟踪
- 跨会话上下文保持
- 向量化语义搜索
- 分布式协作能力

## 安装方式

### 方式1: MCP (Model Context Protocol) - 推荐

#### 1. 安装 CIS

```bash
# macOS
brew install cis

# Linux
curl -fsSL https://cis.dev/install.sh | bash

# 或手动安装
git clone https://github.com/your-org/cis.git
cd cis && cargo install --path cis-node
```

#### 2. 配置 MCP

Claude Desktop 配置 (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "cis": {
      "command": "cis",
      "args": ["mcp", "serve"],
      "env": {
        "CIS_HOME": "~/.cis"
      }
    }
  }
}
```

Kimi Code 配置 (`~/.kimi/mcp.json`):

```json
{
  "servers": [
    {
      "name": "cis",
      "command": "cis mcp serve",
      "transport": "stdio"
    }
  ]
}
```

#### 3. 重启 Agent

配置完成后重启 Claude/Kimi，Agent 将自动发现 CIS Skill。

### 方式2: Native Skill 模式

#### 1. 注册 CIS Skill

```bash
# 注册到 Agent
cis agent register --name cis --type native

# 验证安装
cis agent list
```

#### 2. Agent 配置

Claude Desktop:

```json
{
  "nativeSkills": [
    {
      "name": "cis",
      "path": "/usr/local/bin/cis",
      "args": ["skill", "serve"]
    }
  ]
}
```

## 功能清单

### 记忆管理

```
Agent: 记住我喜欢使用暗黑模式
CIS: ✅ 已保存到记忆（私域，加密）

Agent: 搜索之前关于API设计的讨论
CIS: 🔍 找到3条相关记忆...

Agent: 总结今天的对话
CIS: 📝 已生成摘要并保存
```

### 任务执行

```
Agent: 分析今天的销售数据
CIS: 🎯 匹配到 data-analyzer Skill
     执行中...
     ✅ 完成：发现销售额增长15%

Agent: 创建周报任务
CIS: 📋 任务已创建（ID: task-123）
     截止时间: 本周五
```

### 上下文保持

```
[会话1] Agent: 讨论项目架构
CIS: 💾 已保存会话上下文

[会话2] Agent: 继续之前的架构讨论
CIS: 🔄 恢复上下文
     找到相关会话："项目架构设计"
     主题: microservices, kafka, postgres
```

## API 参考

### 记忆操作

```typescript
// 保存记忆
await cis.memory.set({
  key: "user-preference-theme",
  value: "dark mode",
  domain: "private",  // private | public
  category: "preference"
});

// 语义搜索
const results = await cis.memory.search({
  query: "UI theme preference",
  limit: 5,
  threshold: 0.7
});

// 获取记忆
const value = await cis.memory.get("user-preference-theme");
```

### 任务管理

```typescript
// 创建任务
const task = await cis.task.create({
  title: "Implement auth",
  description: "Add OAuth2 authentication",
  priority: "high",
  dueDate: "2024-02-01"
});

// 列出任务
const tasks = await cis.task.list({
  status: "pending",
  limit: 10
});

// 完成任务
await cis.task.complete(task.id, {
  result: "Auth implemented using JWT"
});
```

### Skill 调用

```typescript
// 自然语言调用 Skill
const result = await cis.skill.invoke({
  intent: "分析今天的销售数据",
  context: { date: "today" }
});

// 直接调用特定 Skill
const result = await cis.skill.call({
  skillId: "data-analyzer",
  action: "analyze",
  params: { dataSource: "sales", type: "summary" }
});
```

### 会话管理

```typescript
// 创建会话
const session = await cis.session.create({
  title: "Architecture Discussion",
  metadata: { project: "myapp" }
});

// 添加消息
await cis.session.addMessage({
  sessionId: session.id,
  role: "user",
  content: "Let's discuss the database design"
});

// 搜索相关会话
const related = await cis.session.findSimilar({
  query: "database design",
  limit: 3
});
```

## 配置

### 基础配置

```toml
# ~/.cis/config.toml
[agent]
# Agent 名称
name = "claude"

# 默认记忆域
default_memory_domain = "private"

# 自动保存对话
auto_save_conversations = true

# 语义搜索阈值
search_threshold = 0.7

[mcp]
# 启用 MCP 协议
enabled = true

# 传输方式: stdio | sse
transport = "stdio"

# SSE 端口（如使用 sse 传输）
port = 3000
```

### 高级配置

```toml
[agent.memory]
# 自动索引消息
auto_index = true

# 索引延迟（秒）
index_delay = 5

# 摘要生成阈值（消息数）
summary_threshold = 10

[agent.skill]
# 置信度阈值
confidence_threshold = 0.7

# 自动确认阈值（高于此值无需确认）
auto_confirm_threshold = 0.9

# 最大候选 Skill 数
max_candidates = 5
```

## 使用示例

### 示例1: 代码助手

```
User: 帮我优化这段代码
[代码粘贴]

Claude: 我来分析这段代码...
CIS: 🎯 匹配到 code-optimizer Skill
     已保存代码到临时记忆
     分析结果：可优化3处
     
Claude: 发现3处可优化...
      1. 使用迭代器替代循环
      2. 提前返回减少嵌套
      3. 使用 const 替代 let
      
User: 应用这些优化

Claude: 应用优化中...
CIS: ✅ 已保存优化后的代码
     创建任务：性能测试（ID: task-456）
```

### 示例2: 项目管理

```
User: 创建一个新项目叫 "电商平台"

Claude: 创建项目中...
CIS: 📁 项目已初始化
     .cis/project.toml 已创建
     工作区：~/projects/ecommerce
     
User: 添加任务：设计数据库

Claude: 添加任务...
CIS: 📋 任务已创建
     ID: task-789
     标题: 设计数据库
     状态: pending
     
User: 查看今天的任务

Claude: 查看中...
CIS: 📊 今日任务 (3)
     1. [高] 设计数据库 (PENDING)
     2. [中] API 文档 (IN_PROGRESS)
     3. [低] 单元测试 (DONE)
```

### 示例3: 知识管理

```
User: 搜索之前关于微服务的讨论

Claude: 搜索中...
CIS: 🔍 找到相关记忆 (5)
     1. "微服务拆分策略" (相似度: 0.92)
     2. "服务间通信方案" (相似度: 0.88)
     3. "数据库设计原则" (相似度: 0.85)
     
Claude: 找到5条相关记忆...
      [展示搜索结果]
      
User: 总结这些讨论

Claude: 总结中...
CIS: 📝 摘要已生成
     已保存到项目知识库
```

## 故障排除

### Agent 无法发现 CIS

```bash
# 检查 CIS 是否运行
cis doctor

# 检查 MCP 配置
cis mcp config --validate

# 重启 MCP 服务
cis mcp restart
```

### 记忆搜索无结果

```bash
# 检查向量存储
cis memory index --status

# 重建索引
cis memory index --rebuild

# 检查嵌入服务
cis ai embedding --test
```

### Skill 调用失败

```bash
# 检查 Skill 列表
cis skill list --all

# 测试 Skill
cis skill test <skill-id>

# 查看日志
cis log --skill <skill-id> --tail 100
```

## 最佳实践

### 1. 记忆分类
- 使用 `category` 标签组织记忆
- 私域存放敏感信息
- 公域存放可共享知识

### 2. 会话管理
- 每个项目创建独立会话
- 定期生成会话摘要
- 使用语义搜索快速定位

### 3. 任务跟踪
- 为复杂任务创建 Task
- 设置合理的截止时间
- 及时更新任务状态

### 4. Skill 选择
- 优先使用高置信度匹配
- 对于关键操作要求确认
- 定期反馈优化匹配

## 卸载

```bash
# 从 Agent 卸载
cis agent unregister --name cis

# 卸载 CIS
brew uninstall cis  # macOS
# 或
rm -rf ~/.cis /usr/local/bin/cis
```

## 获取帮助

- 文档: https://docs.cis.dev
- 社区: https://github.com/your-org/cis/discussions
- 问题: https://github.com/your-org/cis/issues
