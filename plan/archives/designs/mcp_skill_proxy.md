# CIS-MCP Skill 代理服务设计

## 核心目标

1. **本地 MCP 服务** - 每个 CIS 节点独立运行，只暴露本地
2. **统一 Skill 路由** - 解决 Claude CLI / OpenCode Go 的 skill 识别问题
3. **智能匹配** - 向量搜索 + 关键词匹配，提高识别准确率
4. **兼容层** - 适配不同客户端的调用方式

## 架构

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Claude CLI    │     │  OpenCode Go    │     │   其他 MCP 客户端  │
│  (MCP Client)   │     │  (MCP Client)   │     │                 │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │ MCP Protocol (stdio)
                                 ▼
                    ┌──────────────────────────┐
                    │      cis-mcp-server      │
                    │  ┌────────────────────┐  │
                    │  │   Skill Router     │  │
                    │  │  (向量匹配 + 路由)   │  │
                    │  └────────────────────┘  │
                    │  ┌────────────────────┐  │
                    │  │  Claude Adapter    │  │
                    │  │  OpenCode Adapter  │  │
                    │  └────────────────────┘  │
                    └───────────┬──────────────┘
                                │
                    ┌───────────┼───────────┐
                    ▼           ▼           ▼
              ┌─────────┐ ┌─────────┐ ┌─────────┐
              │Skill A  │ │Skill B  │ │Skill C  │
              │(本地)   │ │(本地)   │ │(远程)   │
              └─────────┘ └─────────┘ └─────────┘
```

## MCP Tools 设计

### 1. `skill_match` - 智能 Skill 匹配

**用途**: 根据自然语言描述找到最合适的 skill

```json
{
  "name": "skill_match",
  "description": "根据任务描述匹配最合适的 skill",
  "inputSchema": {
    "type": "object",
    "properties": {
      "description": {
        "type": "string",
        "description": "任务的自然语言描述"
      },
      "context": {
        "type": "object",
        "description": "上下文信息(工作目录、项目类型等)"
      },
      "top_k": {
        "type": "number",
        "description": "返回候选数量",
        "default": 3
      }
    },
    "required": ["description"]
  }
}
```

**返回**:
```json
{
  "matches": [
    {
      "skill_name": "git-commit",
      "confidence": 0.92,
      "reason": "描述中包含'提交代码'、'git'等关键词",
      "params": { "message": "string", "files": "array" }
    },
    {
      "skill_name": "shell-exec",
      "confidence": 0.75,
      "reason": "通用命令执行可匹配此任务"
    }
  ],
  "fallback_available": true
}
```

### 2. `skill_execute` - 执行 Skill

**用途**: 执行指定的 skill，统一接口适配不同客户端

```json
{
  "name": "skill_execute",
  "description": "执行指定的 skill",
  "inputSchema": {
    "type": "object",
    "properties": {
      "skill_name": {
        "type": "string",
        "description": "Skill 名称"
      },
      "params": {
        "type": "object",
        "description": "Skill 参数"
      },
      "client_type": {
        "type": "string",
        "enum": ["claude", "opencode", "auto"],
        "description": "客户端类型(用于适配)",
        "default": "auto"
      },
      "timeout_secs": {
        "type": "number",
        "description": "超时时间",
        "default": 60
      }
    },
    "required": ["skill_name"]
  }
}
```

**返回**:
```json
{
  "success": true,
  "output": "命令执行结果...",
  "exit_code": 0,
  "execution_time_ms": 1250
}
```

### 3. `skill_chain` - Skill 链执行

**用途**: 自动拆解复杂任务为多个 skill 顺序执行

```json
{
  "name": "skill_chain",
  "description": "自动拆解并执行 skill 链",
  "inputSchema": {
    "type": "object",
    "properties": {
      "description": {
        "type": "string",
        "description": "复杂任务描述"
      },
      "preview": {
        "type": "boolean",
        "description": "仅预览不执行",
        "default": false
      },
      "auto_confirm": {
        "type": "boolean",
        "description": "自动确认每个步骤",
        "default": false
      }
    },
    "required": ["description"]
  }
}
```

**返回**:
```json
{
  "chain": [
    { "step": 1, "skill": "git-status", "params": {}, "reason": "检查当前状态" },
    { "step": 2, "skill": "git-add", "params": { "files": ["."] }, "reason": "添加所有更改" },
    { "step": 3, "skill": "git-commit", "params": { "message": "auto" }, "reason": "提交更改" }
  ],
  "executed": true,
  "results": [...]
}
```

### 4. `skill_list` - 列出可用 Skills

```json
{
  "name": "skill_list",
  "description": "列出本地可用的 skills",
  "inputSchema": {
    "type": "object",
    "properties": {
      "filter": {
        "type": "string",
        "description": "过滤关键词"
      },
      "scope": {
        "type": "string",
        "enum": ["local", "remote", "all"],
        "default": "all"
      }
    }
  }
}
```

### 5. `memory_recall` - 记忆召回

**用途**: 基于向量搜索召回相关记忆，解决上下文丢失问题

```json
{
  "name": "memory_recall",
  "description": "召回与当前任务相关的历史记忆",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "当前任务描述"
      },
      "limit": {
        "type": "number",
        "description": "召回数量",
        "default": 5
      },
      "project_path": {
        "type": "string",
        "description": "项目路径(用于过滤)"
      }
    },
    "required": ["query"]
  }
}
```

**返回**:
```json
{
  "memories": [
    {
      "content": "项目使用 pnpm 作为包管理器",
      "similarity": 0.89,
      "source": "shell_history",
      "timestamp": "2024-01-15T10:30:00Z"
    }
  ],
  "context_enhanced": "根据历史记忆，该项目使用 pnpm..."
}
```

## Skill 识别改进策略

### 1. 多维度匹配

```rust
pub struct SkillMatcher {
    /// 向量数据库
    vector_db: Arc<VectorDb>,
    /// 关键词索引
    keyword_index: HashMap<String, Vec<String>>,
    /// 执行历史
    execution_history: Vec<ExecutionRecord>,
}

impl SkillMatcher {
    pub async fn match(&self, description: &str, context: &Context) -> Vec<MatchResult> {
        // 1. 向量语义匹配
        let vector_matches = self.vector_search(description).await;
        
        // 2. 关键词匹配
        let keyword_matches = self.keyword_match(description);
        
        // 3. 历史上下文匹配
        let history_matches = self.history_match(context);
        
        // 4. 项目类型匹配
        let project_matches = self.project_type_match(context.project_type);
        
        // 综合排序
        self.rank_matches(vector_matches, keyword_matches, 
                         history_matches, project_matches)
    }
}
```

### 2. 客户端适配层

```rust
pub trait ClientAdapter {
    fn detect(&self, request: &McpRequest) -> bool;
    fn adapt_params(&self, skill: &str, params: Value) -> Value;
    fn format_output(&self, output: SkillOutput) -> String;
}

pub struct ClaudeAdapter;
pub struct OpenCodeAdapter;

impl ClientAdapter for ClaudeAdapter {
    fn detect(&self, req: &McpRequest) -> bool {
        req.headers.get("client").map_or(false, |v| v.contains("claude"))
    }
    
    fn adapt_params(&self, skill: &str, params: Value) -> Value {
        // Claude 需要特定格式
        match skill {
            "git-commit" => json!({
                "message": params["description"],  // Claude 用 description
                "files": params["files"]
            }),
            _ => params
        }
    }
}

impl ClientAdapter for OpenCodeAdapter {
    fn detect(&self, req: &McpRequest) -> bool {
        req.headers.get("client").map_or(false, |v| v.contains("opencode"))
    }
    
    fn adapt_params(&self, skill: &str, params: Value) -> Value {
        // OpenCode 需要不同格式
        match skill {
            "git-commit" => json!({
                "msg": params["message"],  // OpenCode 用 msg
                "paths": params["files"]
            }),
            _ => params
        }
    }
}
```

### 3. 向量索引构建

```rust
/// 为所有 skill 构建向量索引
pub async fn build_skill_index(&self) -> Result<()> {
    let skills = self.discover_skills().await?;
    
    for skill in skills {
        // 提取描述文本
        let description = format!(
            "{} {} {}",
            skill.name,
            skill.description,
            skill.examples.join(" ")
        );
        
        // 生成向量
        let embedding = self.embedder.encode(&description).await?;
        
        // 存储到向量数据库
        self.vector_db.upsert(
            &skill.id,
            embedding,
            json!({
                "name": skill.name,
                "params": skill.parameters,
                "category": skill.category
            })
        ).await?;
    }
    
    Ok(())
}
```

## 数据存储

### 本地 SQLite 结构

```sql
-- Skill 元数据
CREATE TABLE skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT,
    source TEXT,  -- 'local', 'remote', 'builtin'
    manifest_json TEXT,
    vector_id TEXT,
    created_at INTEGER,
    updated_at INTEGER
);

-- Skill 执行历史(用于改进匹配)
CREATE TABLE execution_history (
    id INTEGER PRIMARY KEY,
    query TEXT NOT NULL,          -- 用户原始描述
    matched_skill TEXT NOT NULL,
    confidence REAL,
    success BOOLEAN,
    execution_time_ms INTEGER,
    feedback_score INTEGER,       -- 用户反馈 1-5
    created_at INTEGER
);

-- 向量存储(使用 sqlite-vec)
CREATE VIRTUAL TABLE skill_vectors USING vec0(
    id TEXT PRIMARY KEY,
    embedding FLOAT[384]  -- 根据模型调整维度
);

-- 记忆条目
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    embedding_id TEXT,
    source TEXT,      -- 'skill_exec', 'user_input', 'auto_extract'
    project_path TEXT,
    created_at INTEGER
);
```

## 配置

### cis-mcp.toml

```toml
[mcp]
name = "cis-skill-proxy"
version = "1.0.0"

[skill]
# Skill 发现路径
scan_paths = ["~/.cis/skills", "./.cis/skills"]
# 远程 skill 注册中心
registry_url = "https://registry.cis.dev"
# 自动更新间隔
auto_update_interval = "1h"

[matching]
# 向量匹配权重
vector_weight = 0.5
# 关键词匹配权重
keyword_weight = 0.3
# 历史匹配权重
history_weight = 0.2
# 最小置信度
min_confidence = 0.6
# 启用 fallback
enable_fallback = true

[memory]
# 向量数据库路径
vector_db_path = "~/.cis/vectors.db"
# 记忆召回数量
recall_limit = 5
# 相似度阈值
similarity_threshold = 0.7

[client.claude]
enabled = true
# Claude 特定配置
max_tokens = 4000

[client.opencode]
enabled = true
# OpenCode 特定配置
format = "json"
```

### Claude Desktop 配置

```json
{
  "mcpServers": {
    "cis-skills": {
      "command": "cis-mcp",
      "args": ["--config", "~/.cis/cis-mcp.toml"],
      "env": {
        "CIS_MCP_CLIENT": "claude"
      }
    }
  }
}
```

### OpenCode 配置

```yaml
# ~/.opencode/config.yaml
mcp_servers:
  cis-skills:
    command: cis-mcp
    args:
      - --config
      - ~/.cis/cis-mcp.toml
    env:
      CIS_MCP_CLIENT: opencode
```

## 工作流程

### Skill 识别执行流程

```
用户输入: "帮我提交今天的代码更改"
        ↓
┌─────────────────┐
│  1. 记忆召回     │ ← 查询相似历史操作
│  memory_recall  │
└────────┬────────┘
         ↓
┌─────────────────┐
│  2. 向量化      │ ← "提交代码更改" → [0.1, 0.3, ...]
│  embedding      │
└────────┬────────┘
         ↓
┌─────────────────┐
│  3. 多维度匹配   │ ← 向量 + 关键词 + 历史
│  skill_match    │
└────────┬────────┘
         ↓
         ├─→ git-commit (0.92)
         ├─→ git-add (0.85)
         └─→ shell-exec (0.72)
         ↓
┌─────────────────┐
│  4. 客户端适配   │ ← 根据 CIS_MCP_CLIENT 调整参数
│  adapt_params   │
└────────┬────────┘
         ↓
┌─────────────────┐
│  5. 执行 Skill   │ ← 调用实际 skill
│  skill_execute  │
└────────┬────────┘
         ↓
    "代码已提交，commit: abc123"
```

## 优势对比

| 功能 | Claude CLI 原版 | OpenCode Go | CIS-MCP |
|------|-----------------|-------------|---------|
| Skill 识别 | 易失灵 | 兼容性差 | ✅ 向量+多维度匹配 |
| 记忆管理 | 无 | 无 | ✅ 本地向量数据库 |
| 离线可用 | ✅ | ✅ | ✅ |
| 自定义 Skill | 难 | 中等 | ✅ 简单配置 |
| 执行历史 | 无 | 无 | ✅ 自动学习 |
| 项目上下文 | 弱 | 弱 | ✅ 项目感知 |

## 实现计划

### Phase 1: 基础框架 (3天)
1. MCP Server 基础实现
2. SQLite + 向量数据库
3. Skill 发现机制

### Phase 2: 匹配引擎 (2天)
1. 向量索引构建
2. 多维度匹配算法
3. 客户端适配层

### Phase 3: 记忆系统 (2天)
1. 记忆存储
2. 向量召回
3. 上下文增强

### Phase 4: 集成测试 (1天)
1. Claude CLI 测试
2. OpenCode 测试
3. 性能优化
