# CIS 架构 V2 - 双向集成基础设施

## 核心概念

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS 基础设施层                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │   Memory    │  │   Skills    │  │   Tasks     │          │
│  │   Store     │  │   Registry  │  │  Scheduler  │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         └─────────────────┼─────────────────┘                │
│                           │                                  │
│                    ┌──────┴──────┐                          │
│                    │  CIS Core   │                          │
│                    │   Engine    │                          │
│                    └──────┬──────┘                          │
└───────────────────────────┼─────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
              ▼                           ▼
    ┌──────────────────┐      ┌──────────────────┐
    │   CIS → Agent    │      │   Agent → CIS    │
    │   (调用 LLM)     │      │   (调用基础设施)  │
    └──────────────────┘      └──────────────────┘
```

## 双向调用机制

### 1. CIS → Agent (CIS 调用 LLM)

CIS 通过 Agent Provider 接口调用外部 LLM Agent：

```rust
// cis-core/src/agent/mod.rs
#[async_trait]
pub trait AgentProvider: Send + Sync {
    /// Agent 名称
    fn name(&self) -> &str;
    
    /// 执行指令
    async fn execute(&self, req: AgentRequest) -> AgentResponse;
    
    /// 流式执行
    async fn execute_stream(&self, req: AgentRequest, tx: mpsc::Sender<String>);
}

pub struct AgentRequest {
    pub prompt: String,
    pub context: AgentContext,
    pub skills: Vec<String>, // 允许使用的 Skill
}

pub struct AgentContext {
    pub work_dir: Option<PathBuf>,
    pub memory_access: Vec<String>, // 允许访问的记忆前缀
    pub project_config: Option<ProjectConfig>,
}
```

### 2. Agent → CIS (Agent 调用 CIS)

Claude 等外部 Agent 通过 CLI/API 调用 CIS：

```bash
# Claude 调用 CIS Skill
$ cis skill call memory-get --key "project/context"
$ cis skill call task-create --title "Implement feature X"
$ cis skill call ai-summary --text "..."

# Claude 获取 CIS 上下文
$ cis context export --format json
$ cis memory query --prefix "project/"
```

## 项目级集成

### 项目配置文件 `.cis/project.toml`

```toml
[project]
name = "my-project"
id = "uuid"

[ai]
# 项目级 AI 引导配置
guide = """
You are working on a Rust project with CIS integration.
Available skills: memory-search, task-manage, code-review
"""
provider = "claude"  # 或 "kimi", "aider", 等

[[skills]]
name = "custom-linter"
path = "./skills/custom-linter"
auto_load = true

[memory]
# 项目级记忆命名空间
namespace = "project/my-project"
shared_keys = ["conventions", "architecture"]
```

### 启动时双向绑定

```rust
// cis-core/src/project/mod.rs
pub struct ProjectSession {
    project: ProjectConfig,
    agent: Box<dyn AgentProvider>,
    memory: Arc<MemoryStore>,
}

impl ProjectSession {
    /// 启动双向集成会话
    pub async fn start(&self) -> Result<()> {
        // 1. CIS → Agent: 发送项目上下文
        let context = self.build_agent_context();
        self.agent.init(context).await?;
        
        // 2. Agent → CIS: 注册 Agent 能力
        self.register_agent_skills().await?;
        
        Ok(())
    }
    
    fn build_agent_context(&self) -> AgentContext {
        AgentContext {
            work_dir: Some(self.project.root_dir.clone()),
            memory_access: vec![
                format!("project/{}/", self.project.id),
                "shared/".to_string(),
            ],
            project_config: Some(self.project.clone()),
        }
    }
}
```

## Agent Skill 适配器

让 Claude 等 Agent 作为 CIS 的 Skill：

```rust
// skills/agent-bridge/src/lib.rs
pub struct AgentBridgeSkill {
    agent: Box<dyn AgentProvider>,
}

impl Skill for AgentBridgeSkill {
    fn name(&self) -> &str { "agent-bridge" }
    
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::AgentCall { prompt, callback } => {
                // 调用外部 Agent
                let response = self.agent.execute(AgentRequest {
                    prompt,
                    context: self.build_context(ctx),
                    skills: self.available_skills(ctx),
                }).await?;
                
                // 回调返回结果
                callback.send(response).await?;
            }
            _ => {}
        }
        Ok(())
    }
}
```

## 网状调用结构

```
┌─────────────────────────────────────────────────────────────┐
│                        Agent (Claude)                        │
│                         ▲         │                          │
│                         │         ▼                          │
│  ┌──────────────────────┴─────────┐                         │
│  │      Agent Bridge Skill        │                         │
│  │   (将 Agent 包装为 CIS Skill)   │                         │
│  └──────────┬─────────────────────┘                         │
│             │                                                │
│             ▼                                                │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                  CIS Core Engine                     │    │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐ │    │
│  │  │ Memory  │  │  Task   │  │  Push   │  │ Custom  │ │    │
│  │  │ Skill   │  │ Skill   │  │ Skill   │  │ Skills  │ │    │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘ │    │
│  │       └─────────────┴─────────────┴─────────────┘    │    │
│  │                         │                            │    │
│  │                    ┌────┴────┐                       │    │
│  │                    │  Router │ ◄── 项目本地 Skill     │    │
│  │                    └────┬────┘                       │    │
│  └─────────────────────────┼────────────────────────────┘    │
│                            │                                 │
│                            ▼                                 │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Project Local Skills                    │    │
│  │  (./.cis/skills/ 或项目内的 skills/ 目录)             │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## CLI 接口

### CIS 调用 Agent

```bash
# 通过 CIS 调用 Claude
$ cis agent claude --prompt "Review this code" --file src/main.rs

# 调用特定 Agent 并传递 CIS 上下文
$ cis agent kimi --prompt "Summarize memory" --memory-prefix "project/"

# 使用项目配置调用
$ cis agent --project . --prompt "What tasks are pending?"
```

### Agent 调用 CIS

```bash
# Claude 可以调用的 CIS 命令
$ cis memory get <key>
$ cis memory set <key> <value>
$ cis memory search <query>

$ cis task list
$ cis task create --title "..." --description "..."
$ cis task complete <id>

$ cis skill list
$ cis skill call <name> --method <method> --args <args>

$ cis context get  # 获取当前项目上下文
$ cis context export  # 导出完整上下文给 Agent
```

## 实现优先级

1. **Agent Provider 接口** - 抽象层
2. **CIS CLI 增强** - Agent 可调用的命令
3. **项目配置** - `.cis/project.toml`
4. **Agent Bridge Skill** - 双向绑定
5. **网状路由** - 本地 Skill 加载
