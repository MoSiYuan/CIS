# CIS CLI AI-Native 设计文档

## 设计哲学

### 核心原则

```
┌─────────────────────────────────────────────────────────────┐
│                    AI-Native CLI 原则                        │
├─────────────────────────────────────────────────────────────┤
│  1. Machine First, Human Compatible                         │
│     - 默认输出机器可读，--human 标志提供人类友好输出          │
│                                                              │
│  2. Composable & Pipeable                                   │
│     - 所有命令支持 JSON 输入/输出，可管道串联                 │
│                                                              │
│  3. Self-Describing                                         │
│     - 命令自带 schema 描述，AI 可自动发现能力                 │
│                                                              │
│  4. Idempotent & Safe                                       │
│     - 命令幂等，重复执行安全，--dry-run 预览变更              │
│                                                              │
│  5. Context-Aware                                           │
│     - 自动检测环境，最小化必需参数                            │
└─────────────────────────────────────────────────────────────┘
```

---

## 命令输出规范

### 默认输出格式（JSON）

```json
{
  "status": "success",
  "code": 0,
  "data": {},
  "meta": {
    "command": "cis dag run",
    "timestamp": "2026-02-07T12:00:00Z",
    "duration_ms": 1234,
    "version": "0.1.0"
  }
}
```

### 错误输出格式

```json
{
  "status": "error",
  "code": 3,
  "error": {
    "type": "ConfigMissing",
    "message": "Vector engine not configured",
    "suggestion": "Run 'cis init' or 'cis config vector'",
    "auto_fixable": true,
    "fix_command": "cis config vector --auto"
  },
  "meta": {
    "command": "cis memory search",
    "timestamp": "2026-02-07T12:00:00Z"
  }
}
```

### 流式输出格式（SSE）

```
event: status
data: {"state": "running", "progress": 0.3}

event: output
data: {"task_id": "compile", "stdout": "Compiling...", "stderr": ""}

event: complete
data: {"state": "success", "exit_code": 0}
```

---

## 命令体系重构

### 现有命令 vs AI-Native 命令

| 场景 | 旧命令（人类） | 新命令（AI-Native） |
|------|---------------|---------------------|
| 执行技能 | `cis skill do "fix bug"` | `cis skill invoke --name code-fix --input '{"issue":"bug"}' --json` |
| 搜索记忆 | `cis memory search "架构"` | `cis memory query --text "架构" --format json --limit 10` |
| 运行 DAG | `cis dag run workflow.toml` | `cis dag execute --spec workflow.toml --json --watch` |
| 查看状态 | `cis status` | `cis system status --format json` |
| 初始化 | `cis init` | `cis system init --non-interactive --auto-config --json` |

### 新命令详细设计

#### 1. `cis skill invoke` - 技能调用

```bash
# 基础调用
cis skill invoke \
  --name "code-fix" \
  --input '{"file":"main.rs","issue":"unused import"}' \
  --json

# 输出
{
  "status": "success",
  "data": {
    "skill": "code-fix",
    "result": {
      "action": "remove",
      "line": 3,
      "content": "use std::io;"
    },
    "confidence": 0.95
  },
  "meta": {"duration_ms": 2345}
}

# 流式输出（实时反馈）
cis skill invoke --name "code-review" --input '{}' --stream
# 返回 SSE 流
```

#### 2. `cis memory query` - 记忆查询

```bash
# 语义搜索
cis memory query \
  --text "上次讨论的微服务架构" \
  --context "project:myapp" \
  --format json \
  --limit 5

# 输出
{
  "status": "success",
  "data": {
    "results": [
      {
        "id": "mem-123",
        "content": "决定使用 gRPC 作为服务间通信...",
        "similarity": 0.89,
        "context": {
          "project": "myapp",
          "timestamp": "2026-02-01T10:00:00Z",
          "source": "meeting-notes"
        }
      }
    ],
    "total": 15,
    "threshold": 0.7
  }
}

# 添加记忆
cis memory add \
  --content "新的架构决策..." \
  --tags "architecture,microservice" \
  --scope project \
  --json
```

#### 3. `cis dag execute` - DAG 执行

```bash
# 执行并获取结构化输出
cis dag execute \
  --spec build.toml \
  --params '{"target":"release"}' \
  --json \
  --watch

# 输出
{
  "status": "success",
  "data": {
    "run_id": "run-abc-123",
    "tasks": [
      {"id": "compile", "status": "completed", "duration_ms": 5000},
      {"id": "test", "status": "completed", "duration_ms": 3000},
      {"id": "package", "status": "completed", "duration_ms": 2000}
    ],
    "outputs": {
      "package": "/path/to/output.tar.gz"
    }
  }
}

# 批量执行
cis dag execute \
  --batch specs/*.toml \
  --parallel 4 \
  --json
```

#### 4. `cis agent` - AI 代理模式

```bash
# 让 AI 代理完成复杂任务
cis agent complete \
  --task "修复项目中所有 Clippy 警告" \
  --context "./src" \
  --strategy "analyze -> plan -> execute -> verify" \
  --json

# 输出（分阶段）
{
  "status": "success",
  "data": {
    "phases": [
      {
        "name": "analyze",
        "result": {
          "warnings_count": 12,
          "categories": ["unused_imports", "unnecessary_clone"]
        }
      },
      {
        "name": "plan",
        "result": {
          "fixes": [
            {"file": "lib.rs", "line": 10, "action": "remove_import"},
            {"file": "main.rs", "line": 45, "action": "remove_clone"}
          ]
        }
      },
      {
        "name": "execute",
        "result": {
          "fixed": 12,
          "failed": 0
        }
      },
      {
        "name": "verify",
        "result": {
          "warnings_remaining": 0,
          "tests_passed": true
        }
      }
    ],
    "summary": "All 12 warnings fixed, tests passing"
  }
}

# 交互式代理（类似 Claude Code）
cis agent interactive \
  --mode "pair-programming" \
  --context "./src" \
  --session-id "session-xyz"
```

#### 5. `cis system` - 系统管理

```bash
# 结构化状态检查
cis system status --format json
{
  "status": "success",
  "data": {
    "initialized": true,
    "version": "0.1.0",
    "components": {
      "vector_engine": {"status": "ok", "model": "nomic-embed-v1.5"},
      "ai_agent": {"status": "ok", "provider": "opencode"},
      "storage": {"status": "ok", "path": "~/.local/share/cis"}
    },
    "health": "healthy"
  }
}

# 环境检查（AI 决策依据）
cis system check --format json
{
  "status": "success",
  "data": {
    "ready": false,
    "missing": ["vector_engine"],
    "suggestions": [
      {
        "action": "init_vector",
        "command": "cis system init-vector --auto",
        "auto_fixable": true
      }
    ]
  }
}

# 一键修复
cis system fix --auto --json
```

---

## 退出码规范（详细）

```rust
pub enum ExitCode {
    /// 成功
    Success = 0,
    
    /// 一般错误（通用）
    GeneralError = 1,
    
    /// 需要用户确认
    /// AI 应该：请求用户输入或添加 --force
    NeedConfirmation = 2,
    
    /// 配置缺失
    /// AI 应该：运行初始化命令
    ConfigMissing = 3,
    
    /// 网络错误
    /// AI 应该：等待后重试
    NetworkError = 4,
    
    /// 权限错误
    /// AI 应该：请求提升权限
    PermissionDenied = 5,
    
    /// 资源不可用
    /// AI 应该：检查依赖服务
    ResourceUnavailable = 6,
    
    /// 超时
    /// AI 应该：增加超时时间或重试
    Timeout = 7,
    
    /// 部分成功（批量操作）
    /// AI 应该：检查失败的子任务
    PartialSuccess = 8,
    
    /// 取消（用户中断）
    Cancelled = 130,  // SIGINT
}
```

---

## Schema 自描述

### 命令发现

```bash
# 获取所有命令的 schema
cis schema --format json

# 输出
{
  "commands": [
    {
      "name": "skill invoke",
      "description": "Invoke a skill with structured input",
      "input_schema": {
        "type": "object",
        "properties": {
          "name": {"type": "string", "description": "Skill name"},
          "input": {"type": "object", "description": "Skill-specific input"}
        },
        "required": ["name"]
      },
      "output_schema": {
        "type": "object",
        "properties": {
          "status": {"type": "string", "enum": ["success", "error"]},
          "data": {"type": "object"}
        }
      }
    }
  ]
}
```

### 能力声明

```bash
# AI 查询 CIS 能力
cis capabilities --format json
{
  "version": "0.1.0",
  "features": [
    "skill_invocation",
    "memory_management",
    "dag_execution",
    "agent_mode"
  ],
  "ai_providers": ["opencode", "claude", "kimi", "aider"],
  "vector_engine": true,
  "extensions": ["mcp"]
}
```

---

## 管道与组合

### 命令组合示例

```bash
# 组合 1：查询记忆 → 分析 → 执行技能
cis memory query --text "架构决策" --json | \
  jq '.data.results[0].content' | \
  cis skill invoke --name "summarize" --input @- --json

# 组合 2：检查 → 修复 → 验证
cis system check --json | \
  jq -r '.data.suggestions[].command' | \
  xargs -I {} cis {} --auto --json

# 组合 3：批量 DAG 执行
ls specs/*.toml | \
  cis dag execute --batch - --parallel 4 --json
```

### AI 工作流示例

```bash
# AI Agent 的完整工作流
#!/bin/bash
# AI 自动修复代码风格

# 1. 检测问题
PROBLEMS=$(cis code check --format json)

# 2. 分析问题
if echo "$PROBLEMS" | jq -e '.data.issues | length > 0' > /dev/null; then
    # 3. 生成修复计划
    PLAN=$(echo "$PROBLEMS" | cis agent plan --task "fix style" --json)
    
    # 4. 执行修复
    echo "$PLAN" | cis agent execute --json
    
    # 5. 验证
    cis code check --format json | jq -e '.data.issues | length == 0'
fi
```

---

## 上下文管理

### 会话上下文

```bash
# 创建会话（保持上下文）
cis session create --name "feature-x" --context '{"branch":"feature-x"}'
# 输出: session-id-123

# 在会话中执行命令
cis session exec session-id-123 -- skill invoke --name "code-gen" --input '{}'

# 添加记忆到会话
cis session remember session-id-123 --content "重要决策..."

# 获取会话上下文
cis session context session-id-123 --format json
{
  "session_id": "session-id-123",
  "name": "feature-x",
  "context": {"branch": "feature-x"},
  "history": [...],
  "memory": [...]
}
```

### 项目上下文

```bash
# 自动检测项目配置
cis context detect --format json
{
  "project_root": "/home/user/myproject",
  "config": {
    "name": "myproject",
    "type": "rust",
    "ai_provider": "opencode"
  },
  "state": {
    "git_branch": "main",
    "last_dag_run": "run-abc-123"
  }
}
```

---

## 交互式 vs 非交互式

### 自动检测

```rust
pub fn is_interactive() -> bool {
    // 1. 检查 stdout 是否是 TTY
    if !atty::is(atty::Stream::Stdout) {
        return false;
    }
    
    // 2. 检查环境变量
    if env::var("CI").is_ok() || env::var("NON_INTERACTIVE").is_ok() {
        return false;
    }
    
    // 3. 检查 --json 或 --batch 标志
    if args.contains("--json") || args.contains("--batch") {
        return false;
    }
    
    true
}
```

### 行为差异

| 场景 | 交互式 | 非交互式 |
|------|--------|----------|
| 输出格式 | 人类友好表格 | JSON |
| 确认提示 | 交互式询问 | 使用默认值或失败 |
| 进度显示 | 进度条 + 动画 | 结构化日志 |
| 错误处理 | 友好提示 | 结构化错误码 |
| 缺失配置 | 引导配置 | 返回错误码 3 |

---

## 实现优先级

### P0（核心）
- [ ] JSON 输出框架
- [ ] 标准化退出码
- [ ] `cis skill invoke` 重构
- [ ] `cis memory query` 重构

### P1（重要）
- [ ] `cis agent` 命令组
- [ ] 流式输出（SSE）
- [ ] Schema 自描述
- [ ] 管道支持

### P2（增强）
- [ ] 会话上下文
- [ ] 批量操作优化
- [ ] 高级错误恢复

---

## 测试策略

```bash
# 自动化测试套件

# 1. JSON 输出验证
cis skill invoke --name test --json | jq -e '.status == "success"'

# 2. 退出码验证
cis system check; echo $?  # 应该是 0 或 3

# 3. 管道测试
cis memory query --text "test" --json | cis skill invoke --name echo --input @-

# 4. 错误处理
cis skill invoke --name nonexistent --json; test $? -eq 1

# 5. 批量操作
cis dag execute --batch <(echo '{"tasks":[]}') --json | jq -e '.data.tasks | length > 0'
```
