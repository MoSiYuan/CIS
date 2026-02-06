# CIS 双模式架构设计
# Unified Capability Layer, Dual Access Patterns

## 核心原则

```
┌─────────────────────────────────────────────────────────────┐
│                    Capability Layer                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Skill     │  │   Memory    │  │   Context   │         │
│  │   Engine    │  │   Service   │  │   Extractor │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         └─────────────────┼─────────────────┘                │
│                           │                                 │
│              Unified API (Rust Library)                     │
│         ┌─────────────────┴─────────────────┐               │
│         ▼                                   ▼               │
│  ┌──────────────┐                  ┌──────────────┐        │
│  │  Skill Mode  │                  │   MCP Mode   │        │
│  │  (Program)   │                  │  (AI Agent)  │        │
│  │              │                  │              │        │
│  │ • cis-node   │                  │ • Claude CLI │        │
│  │ • API call   │                  │ • OpenCode   │        │
│  │ • Script     │                  │ • Cursor     │        │
│  └──────────────┘                  └──────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

**原则**: 能力实现一份，通过不同适配层暴露给不同用户。

---

## 架构分层

### Layer 1: Core Capability (核心能力)

实现所有业务逻辑，与接入方式无关。

```rust
// crates/cis-capability/src/lib.rs

pub struct SkillEngine {
    registry: SkillRegistry,
    executor: SkillExecutor,
}

pub struct MemoryService {
    db: MemoryDb,
    embedder: Option<Embedder>,
}

pub struct ContextExtractor {
    project_detector: ProjectDetector,
    git_analyzer: GitAnalyzer,
}

// 统一接口
impl SkillEngine {
    pub async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult>;
    pub async fn discover(&self, context: &Context) -> Vec<SkillMatch>;
}

impl MemoryService {
    pub async fn store(&self, entry: MemoryEntry) -> Result<()>;
    pub async fn recall(&self, query: &str, context: &Context) -> Vec<Memory>;
}

impl ContextExtractor {
    pub async fn extract(&self, path: &Path) -> Result<ProjectContext>;
}
```

### Layer 2: Access Adapters (接入适配)

#### 2.1 Skill Adapter (程序化接入)

```rust
// crates/cis-skill-adapter/src/lib.rs

pub struct SkillAdapter {
    capability: Arc<CapabilityLayer>,
}

impl SkillAdapter {
    // 转换为 Skill trait 接口
    pub async fn as_skill_trait(&self, input: SkillInput) -> SkillOutput {
        let request = ExecutionRequest::from(input);
        let result = self.capability.skill.execute(request).await;
        SkillOutput::from(result)
    }
    
    // HTTP API 处理
    pub async fn handle_http(&self, req: HttpRequest) -> HttpResponse {
        // 转换 HTTP 请求为内部调用
    }
    
    // CLI 命令处理
    pub async fn handle_cli(&self, args: CliArgs) -> Result<String> {
        // 转换 CLI 参数为内部调用
    }
}
```

#### 2.2 MCP Adapter (AI Agent 接入)

```rust
// crates/cis-mcp-adapter/src/lib.rs

pub struct McpAdapter {
    capability: Arc<CapabilityLayer>,
}

#[tool(name = "skill_execute")]
impl McpAdapter {
    pub async fn skill_execute(&self, params: SkillExecuteParams) -> CallToolResult {
        let request = ExecutionRequest::from(params);
        let result = self.capability.skill.execute(request).await;
        CallToolResult::from(result)
    }
    
    pub async fn memory_recall(&self, params: RecallParams) -> CallToolResult {
        let memories = self.capability.memory.recall(&params.query, &params.context).await;
        CallToolResult::from(memories)
    }
    
    pub async fn context_extract(&self, params: ExtractParams) -> CallToolResult {
        let context = self.capability.context.extract(&params.path).await;
        CallToolResult::from(context)
    }
}
```

---

## 项目结构

```
cis/
├── crates/
│   ├── cis-capability/          # 核心能力层
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── skill/           # Skill 引擎
│   │   │   ├── memory/          # 记忆服务
│   │   │   ├── context/         # 上下文提取
│   │   │   └── types.rs         # 共享类型
│   │   └── Cargo.toml
│   │
│   ├── cis-skill-adapter/       # Skill 模式适配
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── http.rs          # HTTP API
│   │   │   ├── cli.rs           # CLI 命令
│   │   │   └── skill_trait.rs   # Skill trait 实现
│   │   └── Cargo.toml
│   │
│   ├── cis-mcp-adapter/         # MCP 模式适配
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── server.rs        # MCP Server
│   │   │   ├── tools.rs         # Tool 定义
│   │   │   └── resources.rs     # Resource 定义
│   │   └── Cargo.toml
│   │
│   └── cis-core/                # 现有核心（逐步迁移）
│
├── bin/
│   ├── cis-node/                # 现有 CLI（使用 skill-adapter）
│   ├── cis-mcp/                 # MCP Server 二进制
│   └── cis-daemon/              # 后台服务（可选）
│
└── Cargo.toml (workspace)
```

---

## 能力清单 (Capability Registry)

所有能力统一注册，两种模式共享。

```rust
// crates/cis-capability/src/registry.rs

pub struct CapabilityRegistry {
    skills: HashMap<String, SkillCapability>,
    memories: HashMap<String, MemoryCapability>,
    contexts: HashMap<String, ContextCapability>,
}

impl CapabilityRegistry {
    pub fn register_skill(&mut self, skill: SkillCapability) {
        self.skills.insert(skill.name.clone(), skill);
    }
    
    // 自动发现所有能力
    pub fn auto_discover() -> Self {
        let mut registry = Self::default();
        
        // 注册内置技能
        registry.register_skill(SkillCapability::new("git-commit", ...));
        registry.register_skill(SkillCapability::new("shell-exec", ...));
        registry.register_skill(SkillCapability::new("file-search", ...));
        
        // 注册记忆能力
        registry.register_memory(MemoryCapability::new("project-prefs", ...));
        registry.register_memory(MemoryCapability::new("exec-history", ...));
        
        // 注册上下文能力
        registry.register_context(ContextCapability::new("project-detect", ...));
        registry.register_context(ContextCapability::new("git-status", ...));
        
        registry
    }
    
    // 生成 MCP Tools 描述
    pub fn to_mcp_tools(&self) -> Vec<Tool> {
        self.skills.values().map(|s| s.to_mcp_tool()).collect()
    }
    
    // 生成 Skill trait 实现
    pub fn to_skill_impls(&self) -> Vec<Box<dyn Skill>> {
        self.skills.values().map(|s| s.to_skill_trait()).collect()
    }
}
```

---

## 使用方式对比

### 方式 1: 作为 CIS Skill（程序化）

```rust
// 在 cis-node 或其他 Rust 代码中使用

use cis_capability::{SkillEngine, MemoryService};
use cis_skill_adapter::SkillTrait;

#[tokio::main]
async fn main() {
    // 初始化能力层
    let capability = CapabilityLayer::new().await;
    
    // 方式 A: 直接调用能力
    let result = capability.skill.execute(ExecutionRequest {
        skill_name: "git-commit".to_string(),
        params: json!({"message": "fix bug"}),
        context: ProjectContext::detect().await,
    }).await;
    
    // 方式 B: 通过 Skill trait（兼容现有代码）
    let skill = GitCommitSkill::new(capability.clone());
    let output = skill.execute(SkillInput {
        message: "fix bug".to_string(),
    }).await;
    
    // 方式 C: HTTP API
    let app = Router::new()
        .route("/skills/:name", post(handle_skill_execute));
}
```

### 方式 2: 作为 MCP Tool（AI Agent）

```json
// claude_desktop_config.json
{
  "mcpServers": {
    "cis": {
      "command": "cis-mcp",
      "args": ["--stdio"]
    }
  }
}
```

```python
# AI Agent 自动发现并使用

# Agent 思考: 用户说"提交代码"，我有 skill_execute 工具

# Agent 调用:
tools.skill_execute({
    "skill_name": "git-commit",
    "params": {
        "message": "commit changes"
    }
})

# CIS-MCP 内部:
# 1. 调用 capability.skill.execute()
# 2. 自动检测项目上下文
# 3. 执行 git commit
# 4. 返回结果

# Agent 获得: "已在 /project 执行 git commit -m 'commit changes'"
```

---

## 关键设计决策

### 1. 能力实现放哪？

❌ 错误：在 adapter 里实现业务逻辑
```rust
// 不要这样！
impl McpAdapter {
    async fn skill_execute(&self, params: Params) {
        // 直接写死执行逻辑
        std::process::Command::new("git").arg("commit")...
    }
}
```

✅ 正确：在 capability 层实现，adapter 仅转发
```rust
// capability 层实现一次
impl SkillEngine {
    pub async fn execute(&self, req: ExecutionRequest) {
        // 完整的执行逻辑：验证、上下文、执行、记录
    }
}

// 两个 adapter 都调用同一个方法
impl McpAdapter {
    async fn skill_execute(&self, params: Params) {
        self.capability.skill.execute(params.into()).await
    }
}
```

### 2. 参数转换

```rust
// 统一请求类型
pub struct ExecutionRequest {
    pub skill_name: String,
    pub params: serde_json::Value,
    pub context: ProjectContext,
    pub caller: CallerType,  // Skill | MCP | HTTP | CLI
}

// Skill 模式转换
impl From<SkillInput> for ExecutionRequest {
    fn from(input: SkillInput) -> Self {
        Self {
            skill_name: input.skill_name,
            params: serde_json::to_value(input.params).unwrap(),
            context: ProjectContext::detect_blocking(),
            caller: CallerType::Skill,
        }
    }
}

// MCP 模式转换
impl From<McpParams> for ExecutionRequest {
    fn from(params: McpParams) -> Self {
        Self {
            skill_name: params.skill_name,
            params: params.params,
            context: params.context.unwrap_or_else(|| ProjectContext::detect_blocking()),
            caller: CallerType::Mcp,
        }
    }
}
```

### 3. 结果返回

```rust
// 统一结果类型
pub struct ExecutionResult {
    pub success: bool,
    pub output: String,
    pub exit_code: Option<i32>,
    pub metadata: ExecutionMetadata,
}

// 转换为 Skill 输出
impl From<ExecutionResult> for SkillOutput {
    fn from(result: ExecutionResult) -> Self {
        SkillOutput {
            success: result.success,
            content: result.output,
        }
    }
}

// 转换为 MCP 结果
impl From<ExecutionResult> for CallToolResult {
    fn from(result: ExecutionResult) -> Self {
        CallToolResult {
            content: vec![Content::text(result.output)],
            is_error: !result.success,
        }
    }
}
```

---

## 实施步骤

### Phase 1: 提取 Capability Layer（3天）

1. 创建 `cis-capability` crate
2. 从现有代码提取 Skill 逻辑
3. 从现有代码提取 Memory 逻辑
4. 从现有代码提取 Context 逻辑
5. 统一类型定义

### Phase 2: Skill Adapter（1天）

1. 创建 `cis-skill-adapter`
2. 实现 Skill trait 包装
3. 实现 HTTP API 转发
4. 更新 `cis-node` 使用 adapter

### Phase 3: MCP Adapter（2天）

1. 创建 `cis-mcp-adapter`
2. 实现 MCP Server 框架
3. 实现 Tool 注册和路由
4. 实现 3 个核心 Tools

### Phase 4: 集成测试（1天）

1. 双模式功能测试
2. 数据一致性验证
3. 性能基准测试

---

## 总结

**核心价值**:
- 能力实现一次，多处使用
- 不重复造轮子
- 两种场景都获得完整能力

**关键成功因素**:
1. Capability Layer 必须完整、不依赖接入方式
2. Adapter 只做转换，不做业务逻辑
3. 类型系统保证两边接口一致

**最终形态**:
```
用户/Agent
    ├── CLI → cis-node → skill-adapter → capability
    ├── HTTP → cis-api → skill-adapter → capability
    ├── MCP → cis-mcp → mcp-adapter → capability
    └── Code → direct call → capability
```
