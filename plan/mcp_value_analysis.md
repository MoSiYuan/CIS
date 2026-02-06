# CIS-MCP 价值分析与方案决策

## 核心价值判断

### ❌ 不推荐的方案

**1. 纯 Skill 代理层（仅 match/execute）**
- 价值有限：只是加了层匹配算法，Agent 本身也有意图识别
- 复杂度：需要维护向量索引、适配层
- 替代性：Claude/OpenCode 本身就在改进识别能力

**2. 完整 CIS 功能暴露（大而全）**
- 过度设计：Agent 不需要直接操作 Matrix 节点、Worker 池
- 认知负担：工具太多，Agent 选择困难
- 维护成本：需要同步 CIS 所有功能

---

## ✅ 推荐方案：上下文增强型任务执行

### 核心定位
**不是替代 Agent 的决策，而是增强 Agent 的执行能力**

```
┌─────────────────────────────────────────┐
│              AI Agent                    │
│  (决策、规划、推理 - 保持原有能力)        │
└──────────────────┬──────────────────────┘
                   │
    ┌──────────────┼──────────────┐
    ▼              ▼              ▼
┌───────┐    ┌─────────┐    ┌──────────┐
│ Skill │    │ Context │    │  Memory  │
│Store  │    │ Extract │    │  Recall  │
└───┬───┘    └────┬────┘    └────┬─────┘
    │             │              │
    └─────────────┴──────────────┘
                   │
                   ▼
        ┌─────────────────┐
        │   CIS-MCP       │
        │  (轻量执行层)   │
        └────────┬────────┘
                 │
    ┌────────────┼────────────┐
    ▼            ▼            ▼
┌────────┐ ┌────────┐ ┌──────────┐
│ Skill  │ │ File   │ │  Memory  │
│ Exec   │ │ Watch  │ │  Store   │
└────────┘ └────────┘ └──────────┘
```

---

## 关键价值点

### 1. 持久化记忆（解决上下文丢失）

**痛点**：
- Agent 会话重启后完全失忆
- 项目特定偏好需要反复告知
- 无法学习用户习惯

**价值**：
```
用户: "用 pnpm 安装依赖"
Agent: 调用 memory_recall(query="包管理器偏好")
       ↓
CIS: 返回 "该项目使用 pnpm，之前执行过 5 次"
       ↓
Agent: 直接执行 pnpm install，不再询问
```

### 2. 文件监控 + 自动 Skill 触发

**痛点**：
- Agent 无法感知文件变化
- 需要手动触发重复任务

**价值**：
```
用户配置: 当 *.go 文件变化时，自动执行 gofmt + go build

CIS: 监控文件 → 检测到变化 → 通知 Agent
Agent: 收到通知 → 决定是否执行 → 调用 skill_execute
```

### 3. 项目感知执行

**痛点**：
- Agent 需要反复确认项目结构
- 容易在项目根目录外执行命令

**价值**：
```
Agent: 执行 "npm install"
CIS: 自动检测最近的 package.json
     找到 /home/user/project-a/package.json
     在正确目录执行命令
```

### 4. 执行历史追踪

**痛点**：
- Agent 做了操作但无法回溯
- 错误难以复现

**价值**：
```
CIS 记录: 2024-01-15 10:30:00
         Skill: git-commit
         Params: {message: "fix bug"}
         Result: success
         Exit: 0
         CWD: /project-a

用户: "刚才改了什么？"
Agent: 查询 CIS 历史 → 准确回答
```

---

## 精简后的 MCP Tools

### 核心工具（3个）

#### 1. `context_extract` - 上下文提取
```json
{
  "name": "context_extract",
  "description": "提取当前项目上下文信息",
  "inputSchema": {
    "type": "object",
    "properties": {
      "include": {
        "type": "array",
        "items": {
          "enum": ["project_type", "git_status", "dependencies", "recent_changes"]
        }
      }
    }
  }
}
```

**返回**:
```json
{
  "project_root": "/home/user/myapp",
  "project_type": "nodejs",
  "package_manager": "pnpm",
  "git_branch": "feature/auth",
  "recent_commits": ["feat: add login", "fix: typo"],
  "uncommitted_changes": 3
}
```

#### 2. `memory_operate` - 记忆操作
```json
{
  "name": "memory_operate",
  "description": "存储或查询项目相关记忆",
  "inputSchema": {
    "type": "object",
    "properties": {
      "action": {
        "enum": ["store", "recall", "forget"]
      },
      "key": {
        "type": "string",
        "description": "如: 'test_command', 'deploy_target'"
      },
      "value": {
        "type": "string",
        "description": "存储的内容（action=store 时）"
      },
      "scope": {
        "enum": ["global", "project", "session"],
        "default": "project"
      }
    },
    "required": ["action", "key"]
  }
}
```

#### 3. `skill_run` - 执行 Skill
```json
{
  "name": "skill_run",
  "description": "执行 CIS Skill，自动处理上下文",
  "inputSchema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "自然语言命令或 skill 名称"
      },
      "work_dir": {
        "type": "string",
        "description": "工作目录（默认自动检测）"
      },
      "record": {
        "type": "boolean",
        "default": true,
        "description": "是否记录执行历史"
      }
    },
    "required": ["command"]
  }
}
```

**返回**:
```json
{
  "success": true,
  "output": "...",
  "work_dir": "/home/user/myapp",
  "duration_ms": 1200,
  "suggested_next": ["git add .", "git commit"]
}
```

---

## MCP Resources（2个）

### 1. `context://current` - 当前上下文
自动推送项目上下文变化：
- 切换目录时
- git 状态变化时
- 文件修改时

### 2. `history://recent` - 最近执行
可订阅最近执行的命令和结果。

---

## 实现架构

```
cis-mcp-lite/
├── src/
│   ├── main.rs              # MCP Server 入口
│   ├── server.rs            # MCP 协议处理
│   ├── context.rs           # 上下文提取
│   ├── memory.rs            # 记忆存储（SQLite）
│   └── skill_proxy.rs       # Skill 执行代理
├── Cargo.toml
└── cis-mcp.toml             # 配置
```

### 依赖
```toml
[dependencies]
cis-core = { path = "../cis-core" }
rmcp = "0.1"                    # Rust MCP SDK
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
notify = "6"                   # 文件监控
rusqlite = "0.30"              # SQLite
```

---

## 与 CIS 的关系

```
┌─────────────────────────────────────────┐
│           CIS Ecosystem                  │
├─────────────────────────────────────────┤
│  cis-mcp-lite  │  轻量 MCP 服务          │
│  (新)          │  - 上下文提取           │
│                │  - 记忆管理             │
│                │  - Skill 代理           │
├────────────────┼────────────────────────┤
│  cis-core      │  核心库                │
│  (已有)        │  - DAG 调度            │
│                │  - 存储管理            │
│                │  - Skill 框架          │
├────────────────┼────────────────────────┤
│  cis-node      │  CLI & 节点服务        │
│  (已有)        │                        │
└────────────────┴────────────────────────┘
```

**关系**：
- `cis-mcp-lite` 是 `cis-core` 的轻量包装
- 不依赖 Matrix、Worker 等复杂功能
- 专注于 Agent 增强场景

---

## 使用场景对比

### 场景 1：代码审查

**无 CIS-MCP**：
```
Agent: 请审查代码
用户: 这是 Go 项目，用 golangci-lint
Agent: 执行 golangci-lint run...
     报错：找不到配置文件
用户: 配置在 ./config/.golangci.yml
Agent: 执行 golangci-lint run --config ./config/.golangci.yml
```

**有 CIS-MCP**：
```
Agent: 调用 context_extract()
CIS: 返回 {project_type: "go", lint_config: "./config/.golangci.yml"}
Agent: 执行 golangci-lint run --config ./config/.golangci.yml
```

### 场景 2：重复命令

**无 CIS-MCP**：
```
会话 1：
用户: 运行测试
Agent: 执行 npm test？还是 pytest？
用户: pytest -xvs

会话 2（重启后）：
用户: 运行测试
Agent: 执行什么命令？
用户: pytest -xvs（再次说明）
```

**有 CIS-MCP**：
```
会话 1：
Agent: 执行 pytest -xvs
CIS: 记忆 {project_a: {test_command: "pytest -xvs"}}

会话 2：
用户: 运行测试
Agent: 调用 memory_recall(key="test_command")
CIS: 返回 "pytest -xvs"
Agent: 直接执行
```

---

## 实施建议

### 优先级

**P0（必须有）**：
1. `context_extract` - 项目感知
2. `memory_operate` - 记忆存储/查询
3. `skill_run` - 执行封装

**P1（应该有）**：
4. 文件监控 Resource
5. 执行历史 Resource

**P2（可以有）**：
6. 向量搜索（如有 embedding 模型）
7. 复杂 Skill 链

### 不做什么

❌ 不暴露 DAG 调度（Agent 不需要）
❌ 不暴露 Matrix 节点（过度复杂）
❌ 不暴露 Worker 管理（与 Agent 无关）
❌ 不做自然语言转 Skill（Agent 自己能做）

---

## 结论

**核心价值**：解决 Agent **上下文丢失**和**项目感知弱**的痛点

**成功标准**：
- 用户不需要反复说明项目结构
- 会话重启后记忆保留
- Agent 能自动在正确目录执行命令

**建议**：先实现 P0 的三个工具，验证价值后再扩展。
