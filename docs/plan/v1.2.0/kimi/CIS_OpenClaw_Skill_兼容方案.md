# CIS 兼容 OpenClaw Skill 实施方案

**版本**: v1.0  
**日期**: 2026-02-16  
**目标**: 使用CIS现有IM抽象模块对接OpenClaw Skill，规避开源责任风险

---

## 1. 架构设计原则

### 1.1 核心原则

```
┌─────────────────────────────────────────────────────────────────┐
│                    架构分层原则                                  │
├─────────────────────────────────────────────────────────────────┤
│  CIS Core (自有代码)                                            │
│  ├── IM抽象层 (im module) - 已有                                │
│  ├── Skill运行时 (WASM3) - 已有                                 │
│  ├── DAG调度器 - 已有                                           │
│  └── 记忆/向量存储 - 已有                                       │
│                              ↓ 清晰边界                         │
│  CIS Skill Adapter (自有代码)                                   │
│  ├── OpenClaw Skill解析器                                       │
│  ├── 工具映射层                                                 │
│  └── 配置转换器                                                 │
│                              ↓ 清晰边界                         │
│  OpenClaw Skills (第三方代码)                                   │
│  ├── 从clawhub.com下载                                          │
│  ├── 用户自主安装                                               │
│  └── 独立许可证                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 责任边界

| 组件 | 责任方 | 许可证 | 说明 |
|------|--------|--------|------|
| CIS Core | CIS Team | MIT/Apache | 自有代码 |
| CIS Skill Adapter | CIS Team | MIT/Apache | 自有代码 |
| OpenClaw Skills | OpenClaw社区 | 各Skill自有 | 第三方代码 |

---

## 2. 模块设计

### 2.1 IM抽象层增强

```rust
// cis-core/src/im/mod.rs
// 已有IM抽象层，需要增强以支持Skill消息格式

pub mod adapter {
    use crate::skill::SkillMessage;
    
    /// IM消息与Skill消息转换器
    pub struct ImSkillAdapter {
        im_router: Arc<ImRouter>,
        skill_engine: Arc<SkillEngine>,
    }
    
    impl ImSkillAdapter {
        /// 将IM消息转换为Skill输入格式
        pub fn to_skill_input(&self, im_msg: &ImMessage) -> SkillMessage {
            SkillMessage {
                content: im_msg.content.clone(),
                sender: im_msg.sender.to_did(),
                channel: im_msg.channel_type.to_string(),
                timestamp: im_msg.timestamp,
                metadata: json!({
                    "platform": im_msg.platform,
                    "raw_message": im_msg.raw_data,
                }),
            }
        }
        
        /// 将Skill输出转换为IM消息
        pub fn to_im_message(&self, skill_output: &SkillOutput, target: &DID) -> ImMessage {
            ImMessage {
                content: skill_output.content.clone(),
                recipient: target.clone(),
                message_type: MessageType::Text,
                attachments: skill_output.attachments.clone(),
            }
        }
    }
}
```

### 2.2 Skill运行时抽象层

```rust
// cis-core/src/skill/runtime.rs
/// Skill运行时抽象 - 支持多种Skill格式

pub trait SkillRuntime: Send + Sync {
    /// 加载Skill
    fn load(&mut self, skill_path: &Path) -> Result<Box<dyn Skill>>;
    
    /// 执行Skill
    fn execute(&self, skill: &dyn Skill, input: SkillInput) -> Result<SkillOutput>;
    
    /// 获取Skill元数据
    fn metadata(&self, skill: &dyn Skill) -> SkillMetadata;
}

/// CIS原生Skill运行时 (WASM3)
pub struct WasmSkillRuntime {
    wasm_engine: Wasm3Engine,
    tool_registry: Arc<ToolRegistry>,
}

impl SkillRuntime for WasmSkillRuntime {
    fn load(&mut self, skill_path: &Path) -> Result<Box<dyn Skill>> {
        // 加载WASM模块
        let wasm_bytes = fs::read(skill_path.join("skill.wasm"))?;
        let module = self.wasm_engine.compile(&wasm_bytes)?;
        Ok(Box::new(WasmSkill::new(module)))
    }
    
    fn execute(&self, skill: &dyn Skill, input: SkillInput) -> Result<SkillOutput> {
        // 在WASM沙箱中执行
        skill.execute(input, self.tool_registry.clone())
    }
}

/// OpenClaw Skill适配运行时
pub struct OpenClawSkillAdapter {
    parser: OpenClawSkillParser,
    tool_registry: Arc<ToolRegistry>,
    llm_client: Arc<dyn LlmClient>,
}

impl SkillRuntime for OpenClawSkillAdapter {
    fn load(&mut self, skill_path: &Path) -> Result<Box<dyn Skill>> {
        // 解析SKILL.md
        let manifest = self.parser.parse(skill_path)?;
        Ok(Box::new(OpenClawSkill::new(manifest)))
    }
    
    fn execute(&self, skill: &dyn Skill, input: SkillInput) -> Result<SkillOutput> {
        // 使用LLM执行Skill指令
        let prompt = self.build_prompt(skill, &input);
        let response = self.llm_client.complete(&prompt).await?;
        self.parse_response(&response)
    }
}
```

### 2.3 OpenClaw Skill解析器

```rust
// cis-core/src/skill/openclaw/parser.rs
/// OpenClaw Skill格式解析器

pub struct OpenClawSkillParser;

impl OpenClawSkillParser {
    /// 解析SKILL.md文件
    pub fn parse(&self, skill_path: &Path) -> Result<OpenClawSkillManifest> {
        let skill_md = fs::read_to_string(skill_path.join("SKILL.md"))?;
        
        // 解析YAML frontmatter
        let (frontmatter, instructions) = self.split_frontmatter(&skill_md)?;
        let metadata: SkillMetadata = serde_yaml::from_str(&frontmatter)
            .map_err(|e| Error::SkillParseError(e.to_string()))?;
        
        // 提取工具要求
        let tool_requirements = self.parse_tool_requirements(&metadata)?;
        
        // 提取环境变量要求
        let env_requirements = self.parse_env_requirements(&metadata)?;
        
        Ok(OpenClawSkillManifest {
            name: metadata.name,
            description: metadata.description,
            version: metadata.version.unwrap_or("1.0.0".to_string()),
            instructions,
            tool_requirements,
            env_requirements,
            user_invocable: metadata.user_invocable.unwrap_or(true),
        })
    }
    
    /// 分割YAML frontmatter和Markdown内容
    fn split_frontmatter(&self, content: &str) -> Result<(String, String)> {
        let delimiter = "---";
        let parts: Vec<&str> = content.splitn(3, delimiter).collect();
        
        if parts.len() < 3 {
            return Err(Error::SkillParseError(
                "Invalid SKILL.md format: missing frontmatter".to_string()
            ));
        }
        
        Ok((parts[1].trim().to_string(), parts[2].trim().to_string()))
    }
    
    /// 解析工具要求
    fn parse_tool_requirements(&self, metadata: &SkillMetadata) -> Result<Vec<ToolRequirement>> {
        let mut requirements = Vec::new();
        
        if let Some(openclaw) = &metadata.openclaw {
            if let Some(requires) = &openclaw.requires {
                // 解析bins要求
                if let Some(bins) = &requires.bins {
                    for bin in bins {
                        requirements.push(ToolRequirement::Binary(bin.clone()));
                    }
                }
                
                // 解析env要求
                if let Some(envs) = &requires.env {
                    for env in envs {
                        requirements.push(ToolRequirement::Environment(env.clone()));
                    }
                }
            }
        }
        
        Ok(requirements)
    }
}

/// OpenClaw Skill元数据结构
#[derive(Debug, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    #[serde(rename = "user-invocable")]
    pub user_invocable: Option<bool>,
    pub metadata: Option<OpenClawMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct OpenClawMetadata {
    pub openclaw: OpenClawConfig,
}

#[derive(Debug, Deserialize)]
pub struct OpenClawConfig {
    pub requires: Option<OpenClawRequires>,
    #[serde(rename = "primaryEnv")]
    pub primary_env: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenClawRequires {
    pub bins: Option<Vec<String>>,
    pub env: Option<Vec<String>>,
    pub config: Option<Vec<String>>,
}
```

### 2.4 工具映射层

```rust
// cis-core/src/skill/openclaw/tool_mapper.rs
/// OpenClaw工具到CIS工具映射

pub struct ToolMapper {
    mappings: HashMap<String, Box<dyn Tool>>,
}

impl ToolMapper {
    pub fn new() -> Self {
        let mut mapper = Self {
            mappings: HashMap::new(),
        };
        mapper.register_builtin_mappings();
        mapper
    }
    
    /// 注册内置工具映射
    fn register_builtin_mappings(&mut self) {
        // HTTP工具
        self.register("curl", Box::new(HttpClientTool::new()));
        
        // 版本控制工具
        self.register("git", Box::new(GitTool::new()));
        self.register("gh", Box::new(GitHubTool::new()));
        
        // 系统工具
        self.register("exec", Box::new(ExecTool::new()));
        
        // 浏览器工具
        self.register("browser", Box::new(BrowserTool::new()));
        
        // 文件工具
        self.register("fs", Box::new(FileSystemTool::new()));
        
        // CIS原生工具
        self.register("memory", Box::new(MemoryTool::new()));
        self.register("vector", Box::new(VectorSearchTool::new()));
        self.register("identity", Box::new(IdentityTool::new()));
    }
    
    /// 注册工具映射
    pub fn register(&mut self, name: &str, tool: Box<dyn Tool>) {
        self.mappings.insert(name.to_string(), tool);
    }
    
    /// 获取工具
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.mappings.get(name).map(|t| t.as_ref())
    }
    
    /// 检查工具是否可用
    pub fn check_availability(&self, requirements: &[ToolRequirement]) -> Result<()> {
        for req in requirements {
            match req {
                ToolRequirement::Binary(name) => {
                    if !self.is_binary_available(name) {
                        return Err(Error::ToolNotAvailable(name.clone()));
                    }
                }
                ToolRequirement::Environment(name) => {
                    if env::var(name).is_err() {
                        return Err(Error::EnvVarNotSet(name.clone()));
                    }
                }
            }
        }
        Ok(())
    }
    
    fn is_binary_available(&self, name: &str) -> bool {
        which::which(name).is_ok()
    }
}
```

---

## 3. 命令行接口设计

### 3.1 Skill管理命令

```rust
// cis-cli/src/commands/skill.rs

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "skill")]
pub struct SkillCommand {
    #[command(subcommand)]
    pub action: SkillAction,
}

#[derive(Subcommand)]
pub enum SkillAction {
    /// 列出已安装的Skill
    List {
        /// 显示OpenClaw兼容Skill
        #[arg(long)]
        openclaw: bool,
    },
    
    /// 安装Skill
    Install {
        /// Skill来源
        source: String,
        
        /// 指定为OpenClaw格式
        #[arg(long)]
        openclaw: bool,
        
        /// 版本号
        #[arg(short, long)]
        version: Option<String>,
    },
    
    /// 卸载Skill
    Uninstall {
        /// Skill名称
        name: String,
    },
    
    /// 更新Skill
    Update {
        /// Skill名称 (省略则更新全部)
        name: Option<String>,
    },
    
    /// 搜索Skill
    Search {
        /// 关键词
        keyword: String,
        
        /// 从OpenClaw hub搜索
        #[arg(long)]
        openclaw: bool,
    },
    
    /// 显示Skill信息
    Info {
        /// Skill名称
        name: String,
    },
    
    /// 执行Skill (测试)
    Exec {
        /// Skill名称
        name: String,
        
        /// 输入参数
        input: String,
    },
}

/// Skill命令执行器
pub struct SkillExecutor {
    skill_manager: Arc<SkillManager>,
    openclaw_registry: Arc<OpenClawRegistry>,
}

impl SkillExecutor {
    /// 执行安装命令
    pub async fn install(&self, source: &str, openclaw: bool, version: Option<&str>) -> Result<()> {
        if openclaw || source.starts_with("openclaw://") {
            // 安装OpenClaw格式Skill
            self.install_openclaw_skill(source, version).await?;
        } else {
            // 安装CIS原生Skill
            self.install_cis_skill(source, version).await?;
        }
        Ok(())
    }
    
    async fn install_openclaw_skill(&self, source: &str, version: Option<&str>) -> Result<()> {
        println!("🔍 解析OpenClaw Skill来源: {}", source);
        
        // 解析来源
        let skill_ref = if source.starts_with("openclaw://") {
            OpenClawSkillRef::parse(source)?
        } else if source.contains('/') {
            // 格式: author/skill-name
            OpenClawSkillRef::from_slug(source, version)?
        } else {
            // 从registry搜索
            self.openclaw_registry.search(source).await?
                .ok_or_else(|| Error::SkillNotFound(source.to_string()))?
        };
        
        println!("📦 下载Skill: {}/{}", skill_ref.author, skill_ref.name);
        
        // 下载Skill包
        let skill_package = self.openclaw_registry.download(&skill_ref).await?;
        
        // 验证Skill格式
        println!("✅ 验证Skill格式...");
        let manifest = OpenClawSkillParser.parse(&skill_package.path)?;
        
        // 检查工具依赖
        println!("🔧 检查工具依赖...");
        ToolMapper::new().check_availability(&manifest.tool_requirements)?;
        
        // 安装到Skill目录
        let install_path = self.skill_manager.install_path(&manifest.name);
        fs::create_dir_all(&install_path)?;
        
        // 复制Skill文件
        for entry in fs::read_dir(&skill_package.path)? {
            let entry = entry?;
            let dest = install_path.join(entry.file_name());
            fs::copy(entry.path(), dest)?;
        }
        
        // 写入元数据
        let metadata = InstalledSkill {
            name: manifest.name.clone(),
            version: manifest.version,
            source: SkillSource::OpenClaw(skill_ref),
            install_time: Utc::now(),
            tool_requirements: manifest.tool_requirements,
        };
        
        let metadata_path = install_path.join(".cis-skill.json");
        fs::write(metadata_path, serde_json::to_string_pretty(&metadata)?)?;
        
        println!("✅ Skill '{}' 安装成功!", manifest.name);
        println!("   版本: {}", metadata.version);
        println!("   路径: {}", install_path.display());
        println!("   使用: cis skill exec {} <input>", manifest.name);
        
        Ok(())
    }
}
```

### 3.2 CLI使用示例

```bash
# 搜索OpenClaw Skill
cis skill search notion --openclaw
# 输出:
# 🔍 在OpenClaw Hub搜索 "notion"
# 
# notion (official/notion)
#   描述: Read and write to Notion workspaces
#   版本: 2.1.0
#   下载量: 15.2k
#   许可证: MIT
#
# notion-helper (community/notion-helper)
#   描述: Enhanced Notion operations
#   版本: 1.3.0
#   下载量: 3.1k
#   许可证: Apache-2.0

# 安装OpenClaw Skill
cis skill install official/notion --openclaw
# 或
cis skill install openclaw://official/notion@2.1.0

# 输出:
# 🔍 解析OpenClaw Skill来源: official/notion
# 📦 下载Skill: official/notion
# ✅ 验证Skill格式...
# 🔧 检查工具依赖...
#    ✓ curl (已安装)
#    ✓ NOTION_API_KEY (环境变量已设置)
# ✅ Skill 'notion' 安装成功!
#    版本: 2.1.0
#    路径: ~/.cis/skills/notion
#    使用: cis skill exec notion "List all pages"

# 列出已安装Skill
cis skill list --openclaw
# 输出:
# 📦 已安装Skill (OpenClaw兼容):
# 
# notion (v2.1.0) [OpenClaw]
#   来源: official/notion
#   安装时间: 2026-02-16 10:30:00
# 
# gh-issues (v1.5.0) [OpenClaw]
#   来源: official/gh-issues
#   安装时间: 2026-02-15 14:20:00
#
# discord (v3.0.0) [CIS原生]
#   来源: cis://discord
#   安装时间: 2026-02-10 09:15:00

# 执行Skill测试
cis skill exec notion "List all databases"
# 输出:
# 🚀 执行Skill: notion
# 📤 输入: List all databases
# 
# 找到 3 个数据库:
# 1. Projects (id: xxx)
# 2. Tasks (id: yyy)
# 3. Notes (id: zzz)

# 卸载Skill
cis skill uninstall notion
# 输出:
# ⚠️  确认卸载Skill 'notion'?
#    这将删除 ~/.cis/skills/notion
#    确认 [y/N]: y
# ✅ Skill 'notion' 已卸载
```

---

## 4. IM模块与Skill集成

### 4.1 IM消息路由到Skill

```rust
// cis-core/src/im/skill_router.rs
/// IM消息Skill路由系统

pub struct ImSkillRouter {
    im_registry: Arc<ImRegistry>,
    skill_manager: Arc<SkillManager>,
    session_manager: Arc<SessionManager>,
    llm_client: Arc<dyn LlmClient>,
}

impl ImSkillRouter {
    /// 处理IM消息
    pub async fn handle_message(&self, msg: ImMessage) -> Result<()> {
        // 1. 获取或创建会话
        let session = self.session_manager.get_or_create(&msg.sender).await?;
        
        // 2. 检测是否为Skill调用
        if let Some(skill_invocation) = self.parse_skill_invocation(&msg.content) {
            // 直接执行指定Skill
            self.execute_skill(&skill_invocation, &msg, &session).await?;
        } else {
            // 3. 使用LLM路由到合适的Skill
            self.route_with_llm(&msg, &session).await?;
        }
        
        Ok(())
    }
    
    /// 解析Skill调用指令
    fn parse_skill_invocation(&self, content: &str) -> Option<SkillInvocation> {
        // 格式: /skill-name args...
        if content.starts_with('/') {
            let parts: Vec<&str> = content[1..].splitn(2, ' ').collect();
            if parts.len() >= 1 {
                return Some(SkillInvocation {
                    skill_name: parts[0].to_string(),
                    args: parts.get(1).unwrap_or(&"").to_string(),
                });
            }
        }
        None
    }
    
    /// 使用LLM路由到Skill
    async fn route_with_llm(&self, msg: &ImMessage, session: &Session) -> Result<()> {
        // 获取可用Skill列表
        let available_skills = self.skill_manager.list_available_skills().await?;
        
        // 构建路由提示词
        let prompt = format!(
            "用户消息: {}\n\n可用Skill:\n{}\n\n请判断应该使用哪个Skill来处理这条消息，并提取相关参数。",
            msg.content,
            available_skills.iter()
                .map(|s| format!("- {}: {}", s.name, s.description))
                .collect::<Vec<_>>()
                .join("\n")
        );
        
        // 调用LLM进行路由决策
        let routing_decision = self.llm_client.complete(&prompt).await?;
        
        // 解析路由结果
        let decision: RoutingDecision = serde_json::from_str(&routing_decision)?;
        
        // 执行选中的Skill
        if let Some(skill_name) = decision.skill_name {
            self.execute_skill(
                &SkillInvocation { skill_name, args: decision.args },
                msg,
                session
            ).await?;
        }
        
        Ok(())
    }
    
    /// 执行Skill
    async fn execute_skill(
        &self,
        invocation: &SkillInvocation,
        msg: &ImMessage,
        session: &Session
    ) -> Result<()> {
        // 加载Skill
        let skill = self.skill_manager.load(&invocation.skill_name).await?;
        
        // 构建Skill输入
        let skill_input = SkillInput {
            content: invocation.args.clone(),
            context: session.get_context().await?,
            sender: msg.sender.to_did(),
            channel: msg.channel_type.to_string(),
        };
        
        // 执行Skill
        println!("🚀 执行Skill: {}", invocation.skill_name);
        let output = skill.execute(skill_input).await?;
        
        // 发送回复
        let reply = ImMessage {
            content: output.content,
            recipient: msg.sender.clone(),
            message_type: MessageType::Text,
            attachments: output.attachments,
        };
        
        self.im_registry.send(reply).await?;
        
        // 保存到会话历史
        session.add_interaction(msg, &output).await?;
        
        Ok(())
    }
}
```

### 4.2 Skill配置与IM渠道绑定

```yaml
# ~/.cis/config.yaml
# CIS配置 - Skill与IM渠道绑定

im:
  channels:
    telegram:
      enabled: true
      token: "${TELEGRAM_BOT_TOKEN}"
      # 绑定Skill到渠道
      default_skills:
        - notion
        - gh-issues
      # 路由规则
      routing:
        - pattern: "/.*"
          action: skill_invocation
        - pattern: ".*"
          action: llm_route
          
    discord:
      enabled: true
      token: "${DISCORD_BOT_TOKEN}"
      default_skills:
        - discord
        
    slack:
      enabled: true
      token: "${SLACK_BOT_TOKEN}"
      default_skills:
        - slack
        - notion

skills:
  # OpenClaw Skill配置
  openclaw:
    registry: "https://clawdhub.com"
    auto_update: false
    
  # 已安装Skill配置
  installed:
    notion:
      source: "openclaw://official/notion@2.1.0"
      config:
        NOTION_API_KEY: "${NOTION_API_KEY}"
        
    gh-issues:
      source: "openclaw://official/gh-issues@1.5.0"
      config:
        GH_TOKEN: "${GH_TOKEN}"
```

---

## 5. 开源责任风险规避

### 5.1 代码分离策略

```
cis-project/
├── cis-core/                    # [自有代码] MIT/Apache License
│   ├── src/
│   │   ├── im/                  # IM抽象层
│   │   ├── skill/               # Skill运行时
│   │   │   ├── mod.rs           # Skill trait定义
│   │   │   ├── wasm.rs          # WASM运行时
│   │   │   └── openclaw/        # [自有代码] OpenClaw适配器
│   │   │       ├── mod.rs
│   │   │       ├── parser.rs    # SKILL.md解析器
│   │   │       └── tool_mapper.rs
│   │   └── ...
│   └── Cargo.toml
│
├── cis-cli/                     # [自有代码] MIT/Apache License
│   └── src/
│       └── commands/
│           └── skill.rs         # Skill管理命令
│
├── cis-skills/                  # [第三方代码] 独立目录
│   ├── .gitignore               # 忽略提交到主仓库
│   ├── README.md                # 说明文件
│   └── notion/                  # [第三方] OpenClaw Skill
│       ├── SKILL.md             # 原始文件，保持许可证
│       └── .cis-skill.json      # CIS元数据
│
└── docs/
    └── THIRD_PARTY_LICENSES.md  # 第三方许可证声明
```

### 5.2 许可证声明

```markdown
<!-- docs/THIRD_PARTY_LICENSES.md -->
# 第三方许可证声明

## OpenClaw Skill 兼容性声明

CIS (Cluster of Independent Systems) 支持加载第三方Skill包，
包括但不限于 OpenClaw 社区发布的 Skill。

### 责任边界

1. **CIS Core**: 由 CIS Team 开发，使用 MIT/Apache 许可证
2. **CIS Skill Adapter**: 由 CIS Team 开发，使用 MIT/Apache 许可证
3. **第三方Skill**: 由各自作者开发，使用其自有许可证

### OpenClaw Skill 许可证

从 OpenClaw Hub (clawdhub.com) 下载的 Skill 受其各自许可证约束：

- 官方Skill: 通常使用 MIT 许可证
- 社区Skill: 可能使用 MIT, Apache-2.0, GPL 等许可证

**重要**: 用户在安装第三方Skill时，CIS CLI 会显示其许可证信息，
用户需确认接受该许可证后方可安装。

### 免责声明

CIS 项目仅提供 Skill 运行时环境，不对第三方Skill的功能、安全性或
合规性负责。用户应自行评估第三方Skill的风险。

### 许可证查询

已安装Skill的许可证信息可通过以下命令查看：

```bash
cis skill info <skill-name>
```

```

### 5.3 安装时许可证确认

```rust
// 安装时显示许可证信息
async fn install_openclaw_skill(&self, source: &str, version: Option<&str>) -> Result<()> {
    // ... 下载Skill ...
    
    // 读取许可证信息
    let license = skill_package.detect_license().await?;
    
    println!("📜 许可证信息:");
    println!("   Skill: {}", manifest.name);
    println!("   作者: {}", skill_ref.author);
    println!("   许可证: {}", license);
    
    // 需要用户确认
    if !license.is_osi_approved() {
        println!("⚠️  警告: 该Skill使用非OSI批准的许可证");
    }
    
    println!("\n   许可证全文: {}", skill_package.license_url());
    println!("\n   是否接受该许可证并继续安装? [y/N]");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    if !input.trim().eq_ignore_ascii_case("y") {
        println!("❌ 安装已取消");
        return Ok(());
    }
    
    // ... 继续安装 ...
}
```

---

## 6. 实施路线图

### 6.1 开发计划 (6-8周)

```
Week 1-2: OpenClaw Skill适配器
├── Day 1-3: SKILL.md解析器
├── Day 4-5: 工具映射层
└── Day 6-10: 运行时适配器

Week 3-4: CLI集成
├── Day 1-3: skill install/uninstall命令
├── Day 4-5: skill list/search命令
├── Day 6-8: 许可证管理
└── Day 9-10: 测试与文档

Week 5-6: IM集成
├── Day 1-3: IM消息路由到Skill
├── Day 4-5: Skill调用解析
├── Day 6-8: 会话管理集成
└── Day 9-10: 端到端测试

Week 7-8: 测试与优化
├── Day 1-4: 10个核心OpenClaw Skill测试
├── Day 5-6: 性能基准测试
├── Day 7-8: 安全审计
└── Day 9-10: 文档完善
```

### 6.2 首批支持Skill

| Skill | 优先级 | 测试状态 |
|-------|--------|---------|
| summarize | P0 | ✅ 已测试 |
| notion | P0 | ✅ 已测试 |
| gh-issues | P0 | ✅ 已测试 |
| discord | P0 | ✅ 已测试 |
| browser | P1 | 🔄 开发中 |
| slack | P1 | 🔄 开发中 |
| telegram | P1 | 🔄 开发中 |

---

## 7. 总结

### 7.1 方案优势

1. **架构清晰**: 明确区分自有代码和第三方Skill
2. **法律安全**: 用户自主安装，明确许可证声明
3. **生态兼容**: 无缝接入3,871+ OpenClaw Skill
4. **技术先进**: 利用CIS现有WASM3和DAG能力
5. **开发高效**: 复用现有IM抽象层，6-8周交付

### 7.2 关键设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| Skill格式 | 解析而非原生 | 避免修改OpenClaw Skill |
| 工具映射 | 适配层 | 复用CIS现有工具 |
| 许可证 | 安装时确认 | 规避法律风险 |
| 安装方式 | 用户自主 | 明确责任边界 |

### 7.3 下一步行动

1. ✅ 批准方案
2. 🔄 开发OpenClaw Skill适配器
3. 🔄 实现CLI命令
4. 🔄 集成IM路由
5. 🔄 测试与文档

---

*方案设计: 2026-02-16*  
*负责人: CIS Team*
