# CIS v1.1.6 Agent Pool 多 Runtime 架构设计

> **设计日期**: 2026-02-12
> **核心需求**: Agent Pool 支持 Agent 类型（Claude/OpenCode/Kimi）和引擎代码注入
> **目标**: Session 复用、智能任务分配、引擎内核代码范围确定

---

## 问题陈述

### 核心挑战

**挑战 1: Session 复用**
- 不同 Agent 类型可能需要不同的 Session 管理
- 如何在 Agent Pool 中统一管理不同类型的 Session？
- Session 如何在 Agent 间传递和复用？

**挑战 2: 跨 Runtime 任务分配**
- 不同 Agent 类型能力不同
- 如何根据 Agent 能力智能分配任务？
- 如何避免分配到不支持的 Agent？

**挑战 3: 引擎代码注入**
- 需要读取引擎内核代码（unreal 5.7）
- 确定可复用和可注入的代码范围
- 注入机制如何设计？

**挑战 4: GLM 内核 + 多引擎**
- 使用 GLM 为内核
- 同时支持以 Kimi 为内核的 OpenCode
- 引擎代码的统一抽象

---

## 架构设计

### 1. 多 Runtime 支持

```rust
use cis_core::agent::runtime::{RuntimeType, AgentRuntime};

/// Runtime 类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuntimeType {
    /// Claude (Anthropic)
    Claude,

    /// OpenCode (GLM 内核）
    OpenCode,

    /// Kimi (月之暗面）
    Kimi,

    /// Aider
    Aider,

    /// 自定义 Runtime
    Custom(String),
}

/// Agent Runtime trait（统一接口）
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// Runtime 类型
    fn runtime_type(&self) -> RuntimeType;

    /// 创建 Session
    async fn create_session(&self, config: &SessionConfig) -> Result<Box<dyn AgentSession>>;

    /// 执行任务
    async fn execute_task(&self, session: &mut dyn AgentSession, task: TaskSpec) -> Result<TaskResult>;

    /// 流式对话（支持交互式任务）
    async fn chat_stream(&self, session: &mut dyn AgentSession, prompt: &str) -> Result<ChatStream>;

    /// 健康检查
    async fn health_check(&self) -> Result<HealthStatus>;
}

/// Claude Runtime 实现
pub struct ClaudeRuntime {
    api_key: String,
    model: String,
    base_url: String,
}

#[async_trait]
impl AgentRuntime for ClaudeRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Claude
    }

    async fn create_session(&self, config: &SessionConfig) -> Result<Box<dyn AgentSession>> {
        let session = ClaudeSession::new(
            &self.api_key,
            &self.model,
            &self.base_url,
            config,
        ).await?;

        Ok(Box::new(session))
    }

    async fn execute_task(&self, session: &mut dyn AgentSession, task: TaskSpec) -> Result<TaskResult> {
        let claude_session = session.as_any().downcast_ref::<ClaudeSession>()
            .ok_or_else(|| Error::SessionTypeMismatch)?;

        // 调用 Claude API
        let response = claude_session.complete(&task.prompt).await?;

        Ok(TaskResult {
            task_id: task.id,
            output: response.content,
            tokens_used: response.usage.total_tokens,
            status: TaskStatus::Completed,
        })
    }
}

/// OpenCode Runtime 实现（GLM 内核）
pub struct OpenCodeRuntime {
    /// GLM API 配置
    glm_config: GlmConfig,

    /// 支持的引擎
    supported_engines: Vec<EngineType>,
}

#[async_trait]
impl AgentRuntime for OpenCodeRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::OpenCode
    }

    async fn create_session(&self, config: &SessionConfig) -> Result<Box<dyn AgentSession>> {
        let session = OpenCodeSession::new(
            &self.glm_config,
            config,
        ).await?;

        Ok(Box::new(session))
    }

    async fn execute_task(&self, session: &mut dyn AgentSession, task: TaskSpec) -> Result<TaskResult> {
        let opencode_session = session.as_any().downcast_ref::<OpenCodeSession>()
            .ok_or_else(|| Error::SessionTypeMismatch)?;

        // 使用 GLM 内核执行任务
        let response = opencode_session.complete(&task.prompt).await?;

        Ok(TaskResult {
            task_id: task.id,
            output: response.content,
            tokens_used: response.usage.total_tokens,
            status: TaskStatus::Completed,
        })
    }
}

/// Kimi Runtime 实现（月之暗面内核）
pub struct KimiRuntime {
    /// Kimi API 配置
    kimi_config: KimiConfig,
}

#[async_trait]
impl AgentRuntime for KimiRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Kimi
    }

    async fn create_session(&self, config: &SessionConfig) -> Result<Box<dyn AgentSession>> {
        let session = KimiSession::new(
            &self.kimi_config,
            config,
        ).await?;

        Ok(Box::new(session))
    }

    async fn execute_task(&self, session: &mut dyn AgentSession, task: TaskSpec) -> Result<TaskResult> {
        let kimi_session = session.as_any().downcast_ref::<KimiSession>()
            .ok_or_else(|| Error::SessionTypeMismatch)?;

        // 使用 Kimi 内核执行任务
        let response = kimi_session.complete(&task.prompt).await?;

        Ok(TaskResult {
            task_id: task.id,
            output: response.content,
            tokens_used: response.usage.total_tokens,
            status: TaskStatus::Completed,
        })
    }
}
```

---

### 2. Session 复用机制

```rust
/// Agent Session trait（统一接口）
#[async_trait]
pub trait AgentSession: Send + Sync {
    /// Session ID
    fn id(&self) -> &str;

    /// Runtime 类型
    fn runtime_type(&self) -> RuntimeType;

    /// 上下文容量（token 数）
    fn context_capacity(&self) -> usize;

    /// 添加消息到上下文
    async fn add_message(&mut self, role: MessageRole, content: &str) -> Result<()>;

    /// 获取上下文历史
    fn get_history(&self) -> Vec<&Message>;

    /// 清除上下文
    async fn clear_context(&mut self) -> Result<()>;

    /// 序列化 Session（用于跨 Agent 传递）
    fn serialize(&self) -> Result<Vec<u8>>;

    /// 反序列化 Session（从其他 Agent 恢复）
    fn deserialize(data: &[u8]) -> Result<Box<dyn AgentSession>>;
}

/// Session 池（管理不同 Runtime 的 Session）
pub struct SessionPool {
    /// 活跃的 Session（按 Runtime 类型分组）
    sessions: HashMap<RuntimeType, Vec<Box<dyn AgentSession>>>,

    /// Session 复用队列（同一 Runtime 内复用）
    reuse_queues: HashMap<RuntimeType, VecDeque<Box<dyn AgentSession>>>,

    /// 最大 Session 数（每个 Runtime）
    max_sessions_per_runtime: usize,

    /// Session TTL（空闲超时，分钟）
    session_ttl_minutes: u64,
}

impl SessionPool {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            reuse_queues: HashMap::new(),
            max_sessions_per_runtime: 10,
            session_ttl_minutes: 30,
        }
    }

    /// 获取或创建 Session（支持复用）
    pub async fn acquire_session(
        &mut self,
        runtime: &dyn AgentRuntime,
        task: &TaskSpec,
    ) -> Result<Box<dyn AgentSession>> {
        let runtime_type = runtime.runtime_type();

        // 1. 尝试从复用队列获取
        if let Some(session) = self.try_reuse(&runtime_type).await {
            tracing::info!("复用 Session: {} (Runtime: {:?})", session.id(), runtime_type);
            return Ok(session);
        }

        // 2. 创建新 Session
        let config = SessionConfig::from_task(task);
        let session = runtime.create_session(&config).await?;

        tracing::info!("创建新 Session: {} (Runtime: {:?})", session.id(), runtime_type);

        // 3. 添加到活跃 Session
        self.sessions
            .entry(runtime_type.clone())
            .or_insert_with(Vec::new)
            .push(session.clone_box());

        Ok(session)
    }

    /// 尝试复用 Session
    async fn try_reuse(&mut self, runtime_type: &RuntimeType) -> Option<Box<dyn AgentSession>> {
        let queue = self.reuse_queues.get(runtime_type)?;

        // 检查队列头部是否有可用 Session
        while let Some(session) = queue.pop_front() {
            // 检查 Session 是否过期
            if self.is_session_valid(&session).await {
                return Some(session);
            } else {
                // Session 过期，丢弃
                tracing::debug!("Session 已过期: {}", session.id());
                continue;
            }
        }

        None
    }

    /// 归还 Session（放回复用队列）
    pub async fn release_session(&mut self, session: Box<dyn AgentSession>) {
        let runtime_type = session.runtime_type();

        // 从活跃 Session 移除
        if let Some(sessions) = self.sessions.get_mut(&runtime_type) {
            sessions.retain(|s| s.id() != session.id());
        }

        // 添加到复用队列
        self.reuse_queues
            .entry(runtime_type)
            .or_insert_with(VecDeque::new)
            .push_back(session);

        tracing::info!("归还 Session 到复用队列: {} (Runtime: {:?})", session.id(), runtime_type);
    }

    /// 检查 Session 是否有效（未过期）
    async fn is_session_valid(&self, session: &dyn AgentSession) -> bool {
        // 检查 Session 年龄
        let age = session.age();
        let max_age = Duration::from_secs(self.session_ttl_minutes * 60);

        age < max_age
    }
}

/// Session 序列化/反序列化（跨 Runtime 传递）
impl dyn AgentSession {
    fn serialize(&self) -> Result<Vec<u8>> {
        // 使用 bincode 或 MessagePack
        let data = bincode::serialize(&self)?;
        Ok(data)
    }

    fn deserialize(data: &[u8]) -> Result<Box<dyn AgentSession>> {
        // 需要根据 Runtime 类型分发
        // 这里简化，实际需要 Runtime 注册表
        let session: Box<dyn AgentSession> = bincode::deserialize(data)?;
        Ok(session)
    }
}
```

---

### 3. 引擎代码注入

#### 3.1 引擎代码分析

```rust
/// 引擎类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineType {
    /// Unreal Engine 5.7
    Unreal5_7,

    /// Unreal Engine 5.x（其他版本）
    Unreal5,

    /// Unity Engine
    Unity,

    /// Godot Engine
    Godot,

    /// 自定义引擎
    Custom(String),
}

/// 引擎代码范围（确定可复用/注入的部分）
#[derive(Debug, Clone)]
pub struct EngineCodeScope {
    /// 引擎类型
    pub engine_type: EngineType,

    /// 可注入的目录（这些代码可以注入到 Agent 上下文）
    pub injectable_dirs: Vec<PathBuf>,

    /// 只读的目录（只提供给 Agent，不修改）
    pub readonly_dirs: Vec<PathBuf>,

    /// 排除的目录（不提供给 Agent）
    pub exclude_dirs: Vec<PathBuf>,

    /// 文件大小限制（单个文件，MB）
    pub max_file_size_mb: usize,

    /// 总大小限制（所有注入代码，MB）
    pub max_total_size_mb: usize,
}

/// 引擎代码扫描器
pub struct EngineCodeScanner {
    base_dir: PathBuf,
    engine_type: EngineType,
}

impl EngineCodeScanner {
    pub fn new(base_dir: PathBuf, engine_type: EngineType) -> Self {
        Self { base_dir, engine_type }
    }

    /// 扫描引擎代码（确定范围）
    pub fn scan(&self) -> Result<EngineCodeScope> {
        let mut scope = EngineCodeScope {
            engine_type: self.engine_type.clone(),
            injectable_dirs: Vec::new(),
            readonly_dirs: Vec::new(),
            exclude_dirs: vec![
                PathBuf::from("Binaries"),
                PathBuf::from("Intermediate"),
                PathBuf::from("Saved"),
                PathBuf::from(".git"),
                PathBuf::from("node_modules"),
                PathBuf::from("target"),
                PathBuf::from("Cargo.lock"),
            ],
            max_file_size_mb: 1,  // 单个文件 <1MB
            max_total_size_mb: 10, // 总共 <10MB
        };

        // 扫描可注入目录（源代码）
        self.scan_injectable_dirs(&mut scope)?;

        // 扫描只读目录（引擎 API、文档）
        self.scan_readonly_dirs(&mut scope)?;

        Ok(scope)
    }

    /// 扫描可注入目录
    fn scan_injectable_dirs(&self, scope: &mut EngineCodeScope) -> Result<()> {
        let injectable_patterns = vec![
            "Source/",           // C++ 源码
            "Classes/",          // Unreal 类
            "Game/",             // 游戏逻辑
        ];

        for pattern in &injectable_patterns {
            let dir = self.base_dir.join(pattern);
            if dir.exists() {
                scope.injectable_dirs.push(dir);
            }
        }

        Ok(())
    }

    /// 扫描只读目录
    fn scan_readonly_dirs(&self, scope: &mut EngineCodeScope) -> Result<()> {
        let readonly_patterns = vec![
            "Engine/Source/",    // 引擎源码（只读）
            "Engine/Documentation/", // 引擎文档（只读）
            "Config/",            // 配置文件（只读）
        ];

        for pattern in &readonly_patterns {
            let dir = self.base_dir.join(pattern);
            if dir.exists() {
                scope.readonly_dirs.push(dir);
            }
        }

        Ok(())
    }

    /// 统计代码大小
    pub fn calculate_size(&self, scope: &EngineCodeScope) -> Result<CodeSizeStats> {
        let mut stats = CodeSizeStats {
            injectable_size_bytes: 0,
            readonly_size_bytes: 0,
            file_count: 0,
            excluded_size_bytes: 0,
        };

        // 统计可注入代码
        for dir in &scope.injectable_dirs {
            let size = self.calculate_dir_size(dir, &scope.exclude_dirs)?;
            stats.injectable_size_bytes += size;
        }

        // 统计只读代码
        for dir in &scope.readonly_dirs {
            let size = self.calculate_dir_size(dir, &scope.exclude_dirs)?;
            stats.readonly_size_bytes += size;
        }

        // 统计文件数
        stats.file_count = self.count_files(&scope.injectable_dirs, &scope.exclude_dirs)?;

        Ok(stats)
    }

    /// 计算目录大小
    fn calculate_dir_size(&self, dir: &PathBuf, exclude: &[PathBuf]) -> Result<u64> {
        let mut total_size = 0u64;

        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_entry(|e| e.path().is_file())
        {
            let entry = entry?;
            let path = entry.path();

            // 检查是否排除
            if exclude.iter().any(|exc| path.starts_with(exc)) {
                continue;
            }

            // 检查文件大小
            let metadata = entry.metadata()?;
            let size = metadata.len();

            // 检查大小限制
            if size > MAX_FILE_SIZE {
                tracing::warn!("文件过大，跳过: {:?} ({} bytes)", path, size);
                continue;
            }

            total_size += size;
        }

        Ok(total_size)
    }
}

/// 代码大小统计
#[derive(Debug, Clone)]
pub struct CodeSizeStats {
    pub injectable_size_bytes: u64,
    pub readonly_size_bytes: u64,
    pub file_count: usize,
    pub excluded_size_bytes: u64,
}
```

#### 3.2 引擎代码注入

```rust
/// 引擎代码注入器
pub struct EngineCodeInjector {
    scope: EngineCodeScope,
}

impl EngineCodeInjector {
    pub fn new(scope: EngineCodeScope) -> Self {
        Self { scope }
    }

    /// 为 Agent 任务准备引擎代码上下文
    pub async fn prepare_context(
        &self,
        task: &TaskSpec,
    ) -> Result<EngineCodeContext> {
        let mut files = Vec::new();
        let mut total_size = 0u64;

        // 1. 收集可注入文件（有大小限制）
        for dir in &self.scope.injectable_dirs {
            for entry in walkdir::WalkDir::new(dir)
                .into_iter()
                .filter_entry(|e| e.path().is_file())
                {
                    let entry = entry?;
                    let path = entry.path();
                    let metadata = entry.metadata()?;
                    let size = metadata.len();

                    // 检查单文件大小
                    if size > (self.scope.max_file_size_mb * 1024 * 1024) as u64 {
                        continue;
                    }

                    // 检查总大小
                    if total_size + size > (self.scope.max_total_size_mb * 1024 * 1024) as u64 {
                        tracing::warn!("达到总大小限制，停止收集");
                        break;
                    }

                    // 读取文件内容
                    let content = tokio::fs::read(&path).await?;

                    files.push(EngineCodeFile {
                        path: path.clone(),
                        relative_path: path.strip_prefix(&self.scope.base_dir)?.to_path_buf(),
                        content,
                        size,
                    });

                    total_size += size;
                }
        }

        // 2. 收集只读文件（无大小限制，但只读）
        let mut readonly_files = Vec::new();
        for dir in &self.scope.readonly_dirs {
            for entry in walkdir::WalkDir::new(dir)
                .max_depth(2)  // 只读目录限制深度
                .into_iter()
                .filter_entry(|e| e.path().is_file())
                {
                    let entry = entry?;
                    let path = entry.path();

                    // 只读文件无大小限制
                    let content = tokio::fs::read(&path).await?;
                    let metadata = entry.metadata()?;
                    let size = metadata.len();

                    readonly_files.push(EngineCodeFile {
                        path: path.clone(),
                        relative_path: path.strip_prefix(&self.scope.base_dir)?.to_path_buf(),
                        content,
                        size,
                    });
                }
        }

        Ok(EngineCodeContext {
            engine_type: self.scope.engine_type.clone(),
            injectable_files: files,
            readonly_files,
            task: task.clone(),
        })
    }
}

/// 引擎代码上下文（注入到 Agent prompt）
#[derive(Debug, Clone)]
pub struct EngineCodeContext {
    pub engine_type: EngineType,
    pub injectable_files: Vec<EngineCodeFile>,
    pub readonly_files: Vec<EngineCodeFile>,
    pub task: TaskSpec,
}

#[derive(Debug, Clone)]
pub struct EngineCodeFile {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub content: Vec<u8>,
    pub size: u64,
}

/// 使用示例：在 Agent 任务中注入引擎代码
impl AgentTask {
    pub async fn execute_with_engine_code(
        &self,
        runtime: &dyn AgentRuntime,
        engine_context: &EngineCodeContext,
    ) -> Result<TaskResult> {
        // 1. 构建增强的 prompt（包含引擎代码上下文）
        let mut enhanced_prompt = String::new();

        enhanced_prompt.push_str(&format!("引擎类型: {:?}\n\n", engine_context.engine_type));
        enhanced_prompt.push_str("== 可修改的源代码 ==\n\n");

        // 添加可注入文件（摘要）
        for file in &engine_context.injectable_files {
            enhanced_prompt.push_str(&format!(
                "文件: {:?}\n大小: {} bytes\n\n",
                file.relative_path, file.size
            ));

            // 如果文件较小，直接包含内容
            if file.size < 10 * 1024 {  // <10KB
                let content = String::from_utf8_lossy(&file.content);
                enhanced_prompt.push_str(&format!("内容:\n```\n{}\n```\n\n", content));
            } else {
                enhanced_prompt.push_str("(内容太大，仅提供文件路径)\n\n");
            }
        }

        enhanced_prompt.push_str("== 引擎只读代码 ==\n\n");
        enhanced_prompt.push_str("(以下代码仅提供上下文，不应修改)\n\n");

        // 添加只读文件（仅路径）
        for file in &engine_context.readonly_files {
            enhanced_prompt.push_str(&format!(
                "- {:?} ({} bytes)\n",
                file.relative_path, file.size
            ));
        }

        enhanced_prompt.push_str(&format!("== 任务 ==\n\n{}\n", engine_context.task.prompt));

        // 2. 使用增强的 prompt 执行任务
        let session = runtime.create_session(&SessionConfig::default()).await?;
        let response = session.complete(&enhanced_prompt).await?;

        Ok(TaskResult {
            task_id: engine_context.task.id.clone(),
            output: response.content,
            tokens_used: response.usage.total_tokens,
            status: TaskStatus::Completed,
        })
    }
}
```

---

### 4. 智能 Task 分配器

```rust
/// Task 分配器（考虑 Agent 能力和引擎类型）
pub struct TaskAllocator {
    /// 可用的 Agent Teams
    teams: Vec<AgentTeam>,

    /// 任务队列
    task_queue: VecDeque<TaskSpec>,
}

impl TaskAllocator {
    pub fn new(teams: Vec<AgentTeam>) -> Self {
        Self {
            teams,
            task_queue: VecDeque::new(),
        }
    }

    /// 智能分配任务
    pub async fn assign_task(
        &mut self,
        task: TaskSpec,
    ) -> Result<AgentTeam> {
        // 1. 分析任务需求
        let requirements = self.analyze_requirements(&task)?;

        // 2. 筛选满足要求的 Teams
        let capable_teams: Vec<_> = self.teams.iter()
            .filter(|team| self.can_handle(team, &requirements))
            .collect();

        if capable_teams.is_empty() {
            return Err(Error::NoCapableTeam {
                requirements,
                available_teams: self.teams.clone(),
            });
        }

        // 3. 选择最佳 Team
        let best_team = self.select_best_team(&capable_teams, &task)?;

        // 4. 分配任务
        best_team.assign_task(task).await?;

        tracing::info!(
            "任务 {} 分配给 Team {} (Runtime: {:?})",
            task.id, best_team.name, best_team.runtime_type
        );

        Ok(best_team)
    }

    /// 分析任务需求
    fn analyze_requirements(&self, task: &TaskSpec) -> Result<TaskRequirements> {
        let mut requirements = TaskRequirements::default();

        // 1. 从任务类型推断需求
        match &task.task_type {
            TaskType::ModuleRefactoring => {
                // 需要代码读写能力
                requirements.needs_code_read = true;
                requirements.needs_code_write = true;
            }
            TaskType::EngineCodeInjection => {
                // 需要引擎代码注入能力
                requirements.needs_engine_injection = true;
                requirements.engine_type = task.engine_type.clone();
            }
            TaskType::PerformanceOptimization => {
                // 需要性能分析能力
                requirements.needs_profiling = true;
            }
            _ => {}
        }

        // 2. 从上下文推断需求
        if let Some(engine_context) = &task.engine_context {
            requirements.engine_type = Some(engine_context.engine_type.clone());
            requirements.needs_large_context = true;
        }

        Ok(requirements)
    }

    /// 检查 Team 是否能处理任务
    fn can_handle(&self, team: &AgentTeam, requirements: &TaskRequirements) -> bool {
        // 1. 检查能力匹配
        for capability in &requirements.capabilities_needed {
            if !team.capabilities.contains(capability) {
                return false;
            }
        }

        // 2. 检查引擎类型匹配
        if let Some(required_engine) = &requirements.engine_type {
            match required_engine {
                EngineType::Unreal5_7 => {
                    // Unreal 5.7 需要特定的 Agent 类型
                    // 例如：OpenCode 可能更适合处理 C++ 代码
                    if !matches!(team.runtime_type, RuntimeType::OpenCode | RuntimeType::Claude) {
                        return false;
                    }
                }
                _ => {}
            }
        }

        // 3. 检查负载（如果指定）
        if let Some(max_load) = requirements.max_concurrent_tasks {
            if team.current_task_count() >= max_load {
                return false;
            }
        }

        true
    }

    /// 选择最佳 Team（基于负载和能力匹配度）
    fn select_best_team(
        &self,
        teams: &[&AgentTeam],
        task: &TaskSpec,
    ) -> Result<&AgentTeam> {
        // 策略 1: 最低负载
        let min_load_team = teams.iter()
            .min_by_key(|t| t.current_task_count())
            .ok_or(Error::NoAvailableTeam)?;

        // 策略 2: 能力匹配度（如果多个 Team 负载相同）
        // ...

        Ok(min_load_team)
    }
}

/// 任务需求
#[derive(Debug, Clone, Default)]
pub struct TaskRequirements {
    /// 需要的能力
    pub capabilities_needed: Vec<TaskCapability>,

    /// 需要引擎类型
    pub engine_type: Option<EngineType>,

    /// 是否需要代码读取
    pub needs_code_read: bool,

    /// 是否需要代码写入
    pub needs_code_write: bool,

    /// 是否需要引擎代码注入
    pub needs_engine_injection: bool,

    /// 是否需要性能分析
    pub needs_profiling: bool,

    /// 是否需要大上下文
    pub needs_large_context: bool,

    /// 最大并发任务数（负载限制）
    pub max_concurrent_tasks: Option<usize>,
}

/// 任务能力
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskCapability {
    /// 代码审阅
    CodeReview,

    /// 模块重构
    ModuleRefactoring,

    /// 测试编写
    TestWriting,

    /// 文档编写
    Documentation,

    /// 性能优化
    PerformanceOptimization,

    /// 引擎代码注入
    EngineCodeInjection,

    /// 安全加固
    SecurityHardening,
}
```

---

### 5. Agent Pool 架构更新

```rust
/// Agent Pool（支持多 Runtime 和 Session 复用）
pub struct AgentPool {
    /// 可用的 Teams（不同 Runtime）
    teams: Vec<AgentTeam>,

    /// Session 池（跨 Runtime 复用）
    session_pool: SessionPool,

    /// 任务分配器
    allocator: TaskAllocator,

    /// 事件总线（跨 Team 通信）
    event_bus: Arc<EventBus>,

    /// 引擎代码扫描器（按引擎类型）
    engine_scanners: HashMap<EngineType, EngineCodeScanner>,
}

impl AgentPool {
    pub fn new() -> Self {
        Self {
            teams: Vec::new(),
            session_pool: SessionPool::new(),
            allocator: TaskAllocator::new(),
            event_bus: Arc::new(EventBus::new(1000)),
            engine_scanners: HashMap::new(),
        }
    }

    /// 添加 Team（支持不同 Runtime）
    pub fn add_team(&mut self, team: AgentTeam) {
        tracing::info!("添加 Team: {} (Runtime: {:?})", team.name, team.runtime_type);
        self.teams.push(team);
    }

    /// 注册引擎代码扫描器
    pub fn register_engine_scanner(&mut self, engine_type: EngineType, base_dir: PathBuf) {
        let scanner = EngineCodeScanner::new(base_dir, engine_type);
        self.engine_scanners.insert(engine_type, scanner);
    }

    /// 分配任务（智能匹配）
    pub async fn assign_task(&mut self, task: TaskSpec) -> Result<AssignmentResult> {
        // 1. 如果任务需要引擎代码，先扫描
        let engine_context = if task.task_type == TaskType::EngineCodeInjection {
            if let Some(engine_type) = &task.engine_type {
                let scanner = self.engine_scanners.get(engine_type)
                    .ok_or_else(|| Error::EngineNotRegistered(engine_type.clone()))?;

                // 扫描引擎代码
                let scope = scanner.scan()?;

                // 注入引擎代码到任务上下文
                let injector = EngineCodeInjector::new(scope);
                Some(injector.prepare_context(&task).await?)
            } else {
                None
            }
        } else {
            None
        };

        // 2. 更新任务上下文
        let mut enhanced_task = task.clone();
        enhanced_task.engine_context = engine_context;

        // 3. 分配到合适的 Team
        let team = self.allocator.assign_task(enhanced_task).await?;

        // 4. 获取或创建 Session（支持复用）
        let session = self.session_pool.acquire_session(
            &team.runtime,
            &enhanced_task,
        ).await?;

        // 5. 执行任务
        let runtime = team.get_runtime();
        let result = if let Some(engine_context) = &enhanced_task.engine_context {
            // 使用引擎代码上下文执行
            Self::execute_with_engine_code(runtime, session, engine_context).await?
        } else {
            // 普通执行
            runtime.execute_task(session, enhanced_task).await?
        };

        // 6. 归还 Session（复用）
        self.session_pool.release_session(session).await;

        Ok(AssignmentResult {
            task_id: task.id,
            team_id: team.id,
            session_id: session.id(),
            result,
        })
    }
}

/// 分配结果
#[derive(Debug, Clone)]
pub struct AssignmentResult {
    pub task_id: String,
    pub team_id: String,
    pub session_id: String,
    pub result: TaskResult,
}
```

---

## 典型使用场景

### 场景 1: GLM 内核 + 多引擎

```bash
# 1. 创建 Agent Pool
pool_id=$(cis agent pool create --name "glm-pool")

# 2. 添加 OpenCode Team（GLM 内核）
cis agent pool add-team $pool_id \
    --name "OpenCode-GLM" \
    --runtime "opencode" \
    --capabilities "ModuleRefactoring,EngineCodeInjection"

# 3. 添加 Claude Team（作为补充）
cis agent pool add-team $pool_id \
    --name "Claude-Assist" \
    --runtime "claude" \
    --capabilities "CodeReview,Documentation"

# 4. 注册引擎代码扫描器
cis agent pool register-engine $pool_id \
    --engine "unreal5.7" \
    --path "/path/to/unreal5.7/project"

# 5. 创建任务（引擎代码注入）
cat > task-engine-injection.json << 'EOF'
{
  "id": "task-001",
  "name": "Unreal 5.7 C++ 代码重构",
  "type": "EngineCodeInjection",
  "engine_type": "Unreal5_7",
  "priority": "p1",
  "prompt": "重构 PlayerController.cpp，优化性能瓶颈"
}
EOF

# 6. 分配任务（自动选择合适的 Team）
cis agent pool assign-task $pool_id --task task-engine-injection.json

# Agent Pool 会：
# - 识别任务需要引擎代码注入
# - 扫描 Unreal 5.7 代码
# - 选择支持 C++ 的 Team（OpenCode）
# - 注入引擎代码上下文到 prompt
# - 执行任务
```

### 场景 2: Session 复用

```bash
# Agent A 执行任务 1（使用 Session S1）
task_id=$(cis agent pool execute --pool-id $pool_id --task task-001)

# Agent B 执行任务 2（可以复用 Session S1）
# 如果两个任务属于同一 Runtime，且 S1 未过期
# Agent Pool 会自动复用 Session，节省初始化时间

# 查看复用统计
cis agent pool session-stats $pool_id
# 输出:
# Session 复用率: 75%
# 平均 Session 生命周期: 25 分钟
# 节省的初始化次数: 150 次
```

### 场景 3: 跨 Runtime 协作

```bash
# 任务需要代码审阅（Claude）和性能优化（OpenCode）

# 1. 创建主任务
cat > task-composite.json << 'EOF'
{
  "id": "task-composite",
  "name": "代码审阅和优化",
  "type": "CompositeTask",
  "subtasks": [
    {"id": "review", "type": "CodeReview", "runtime": "claude"},
    {"id": "optimize", "type": "PerformanceOptimization", "runtime": "opencode"}
  ],
  "dependencies": []
}
EOF

# 2. 分配任务
cis agent pool assign-task $pool_id --task task-composite.json

# Agent Pool 会：
# - 分配 review 到 Claude Team
# - 分配 optimize 到 OpenCode Team
# - 并行执行两个子任务
# - 通过 EventBus 协调
```

---

## 更新的任务定义

### 新增任务类型

```toml
[[task]]
id = "V-4-Engine"
name = "引擎代码注入支持"
type = "EngineCodeInjection"
priority = "p1"
effort_person_days = 8

prompt = """
# 任务：引擎代码注入支持

## 目标
实现引擎代码扫描、注入、范围管理。

## 参考文档
- AGENT_POOL_MULTI_RUNTIME_DESIGN.md

## 具体步骤
1. 实现 EngineCodeScanner
   - 扫描引擎代码目录
   - 确定可注入范围
   - 确定只读范围
   - 计算代码大小
2. 实现 EngineCodeInjector
   - 为 Agent 任务准备引擎代码上下文
   - 控制文件大小限制
   - 生成增强的 prompt
3. 集成到 TaskAllocator
   - 识别引擎代码注入任务
   - 自动调用 Scanner 和 Injector
4. 支持多种引擎
   - Unreal 5.7
   - Unreal 5.x
   - Unity
   - Godot

## 验收标准
- [ ] 引擎代码扫描准确
- [ ] 文件大小限制生效
- [ ] Agent prompt 正确包含引擎代码
- [ ] 测试覆盖率 >80%

## 依赖
- V-1: 需要事件总线

## 上下文
- 引擎示例项目（如果可用）
- unreal 5.7 目录结构
"""

[[task.dependencies]]
task_id = "V-1"

[[task.capabilities]]
capability = "ModuleRefactoring"
capability = "EngineCodeInjection"
```

---

## 可替换的抽象接口设计

### 核心原则：依赖倒置

```
高层模块（Agent Pool）
    ↓ 依赖
抽象接口（trait）
    ↑ 实现
低层模块（具体 Agent）
```

**关键点**：
- Agent Pool 只依赖 trait，不依赖具体实现
- Agent 可以随时替换，不影响 Pool
- 通过注册机制添加新 Agent 类型

### 1. Agent 抽象接口（可替换）

```rust
/// Agent 执行器抽象（可替换接口）
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// 执行任务
    async fn execute(&self, task: TaskContext) -> Result<TaskOutput>;

    /// 流式对话（支持交互式任务）
    async fn chat_stream(&self, context: &ConversationContext) -> Result<ChatStream>;

    /// 健康检查
    async fn health_check(&self) -> Result<HealthStatus>;

    /// 能力查询
    fn capabilities(&self) -> Vec<Capability>;
}

/// Agent 描述符（注册表项）
pub struct AgentDescriptor {
    /// Agent 类型标识
    pub agent_type: String,

    /// 显示名称
    pub display_name: String,

    /// 提供的能力
    pub capabilities: Vec<Capability>,

    /// 创建函数（工厂模式）
    pub factory: fn() -> Result<Box<dyn AgentExecutor>>,

    /// 配置 schema
    pub config_schema: ConfigSchema,
}

/// Agent 注册表（全局单例）
pub struct AgentRegistry {
    agents: HashMap<String, AgentDescriptor>,
}

impl AgentRegistry {
    pub fn instance() -> &'static Self {
        use std::sync::OnceLock;
        static REGISTRY: OnceLock<AgentRegistry> = OnceLock::new();
        REGISTRY.get().unwrap_or_else(|| {
            let mut registry = AgentRegistry::new();
            registry.register_defaults();
            registry
        })
    }

    fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// 注册默认 Agents
    fn register_defaults(&mut self) {
        // Claude Agent
        self.register(AgentDescriptor {
            agent_type: "claude".to_string(),
            display_name: "Claude (Anthropic)".to_string(),
            capabilities: vec![
                Capability::CodeReview,
                Capability::ModuleRefactoring,
                Capability::Documentation,
            ],
            factory: || Ok(Box::new(ClaudeExecutor::new()?)),
            config_schema: ConfigSchema {
                required_fields: vec![
                    ("api_key".to_string(), FieldType::SecretString),
                    ("model".to_string(), FieldType::String),
                    ("base_url".to_string(), FieldType::Url),
                ],
            },
        });

        // OpenCode Agent (GLM 内核)
        self.register(AgentDescriptor {
            agent_type: "opencode".to_string(),
            display_name: "OpenCode (GLM)".to_string(),
            capabilities: vec![
                Capability::ModuleRefactoring,
                Capability::EngineCodeInjection,
                Capability::PerformanceOptimization,
            ],
            factory: || Ok(Box::new(OpenCodeExecutor::new()?)),
            config_schema: ConfigSchema {
                required_fields: vec![
                    ("glm_api_key".to_string(), FieldType::SecretString),
                    ("endpoint".to_string(), FieldType::Url),
                    ("model".to_string(), FieldType::String),
                ],
            },
        });

        // Kimi Agent
        self.register(AgentDescriptor {
            agent_type: "kimi".to_string(),
            display_name: "Kimi (月之暗面)".to_string(),
            capabilities: vec![
                Capability::ModuleRefactoring,
                Capability::EngineCodeInjection,
            ],
            factory: || Ok(Box::new(KimiExecutor::new()?)),
            config_schema: ConfigSchema {
                required_fields: vec![
                    ("api_key".to_string(), FieldType::SecretString),
                    ("model".to_string(), FieldType::String),
                ],
            },
        });
    }

    /// 注册自定义 Agent
    pub fn register(&mut self, descriptor: AgentDescriptor) {
        self.agents.insert(descriptor.agent_type.clone(), descriptor);
    }

    /// 获取 Agent（创建实例）
    pub fn get_agent(&self, agent_type: &str) -> Result<Box<dyn AgentExecutor>> {
        let descriptor = self.agents.get(agent_type)
            .ok_or_else(|| Error::AgentNotFound(agent_type.to_string()))?;

        // 调用工厂函数创建实例
        (descriptor.factory)()
    }

    /// 查询能力匹配的 Agents
    pub fn find_by_capability(&self, capability: Capability) -> Vec<&AgentDescriptor> {
        self.agents.values()
            .filter(|agent| agent.capabilities.contains(&capability))
            .collect()
    }
}

/// 能力定义
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    CodeReview,
    ModuleRefactoring,
    EngineCodeInjection,
    TestWriting,
    Documentation,
    PerformanceOptimization,
}

/// 配置字段类型
#[derive(Debug, Clone)]
pub enum FieldType {
    String,
    SecretString,  // 敏感信息，不显示
    Url,
    Number,
    Boolean,
}
```

### 2. 任务上下文（与 Agent 解耦）

```rust
/// 任务上下文（Agent 不可知）
#[derive(Debug, Clone)]
pub struct TaskContext {
    /// 任务 ID
    pub id: String,

    /// 任务名称
    pub name: String,

    /// 任务类型
    pub task_type: TaskType,

    /// 优先级
    pub priority: TaskPriority,

    /// Prompt 模板（可能包含占位符）
    pub prompt_template: String,

    /// 上下文变量（用于填充 prompt）
    pub context_vars: HashMap<String, String>,

    /// 引擎代码上下文（如果需要）
    pub engine_context: Option<EngineCodeContext>,

    /// 超时设置
    pub timeout_secs: Option<u64>,

    /// 依赖任务
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskType {
    ModuleRefactoring,
    EngineCodeInjection,
    PerformanceOptimization,
    CodeReview,
    TestWriting,
    Documentation,
}

impl TaskContext {
    /// 渲染 prompt（填充变量）
    pub fn render_prompt(&self) -> Result<String> {
        let mut prompt = self.prompt_template.clone();

        // 替换变量占位符
        for (key, value) in &self.context_vars {
            let placeholder = format!("{{{}}}", key);
            prompt = prompt.replace(&placeholder, value);
        }

        // 添加引擎代码上下文（如果有）
        if let Some(engine_ctx) = &self.engine_context {
            prompt.push_str("\n\n== 引擎代码上下文 ==\n");
            prompt.push_str(&format!("引擎类型: {:?}\n", engine_ctx.engine_type));
            prompt.push_str("可修改源代码:\n");
            for file in &engine_ctx.injectable_files {
                prompt.push_str(&format!("  - {}\n", file.relative_path));
            }
            prompt.push_str("只读参考:\n");
            for file in &engine_ctx.readonly_files {
                prompt.push_str(&format!("  - {}\n", file.relative_path));
            }
        }

        Ok(prompt)
    }
}
```

### 3. Agent 执行适配器（解耦层）

```rust
/// Agent 执行适配器（连接 TaskContext 和 AgentExecutor）
pub struct AgentAdapter {
    registry: Arc<AgentRegistry>,
    session_pool: Arc<SessionPool>,
}

impl AgentAdapter {
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self {
            registry,
            session_pool: Arc::new(SessionPool::new()),
        }
    }

    /// 执行任务（自动选择 Agent）
    pub async fn execute(&self, task: TaskContext) -> Result<TaskOutput> {
        // 1. 渲染 prompt
        let prompt = task.render_prompt()?;

        // 2. 查找合适的 Agents
        let required_capability = Self::task_to_capability(&task.task_type)?;
        let available_agents = self.registry.find_by_capability(required_capability);

        if available_agents.is_empty() {
            return Err(Error::NoCapableAgent {
                capability: required_capability,
            });
        }

        // 3. 选择最佳 Agent（负载均衡）
        let agent = self.select_best_agent(available_agents)?;

        // 4. 创建或获取 Session
        let descriptor = self.registry.agents.get(&agent.agent_type)
            .ok_or_else(|| Error::AgentNotFound(agent.agent_type.clone()))?;
        let runtime = Self::create_runtime(descriptor)?;

        let mut session = self.session_pool.acquire_session(&runtime, &task).await?;

        // 5. 执行任务
        let result = match task.task_type {
            TaskType::EngineCodeInjection => {
                // 需要引擎代码上下文
                if let Some(engine_ctx) = &task.engine_context {
                    Self::execute_with_engine_code(&runtime, &mut session, &prompt, engine_ctx).await?
                } else {
                    runtime.execute_task(&mut *session, TaskSpec::from(&task)).await?
                }
            }
            _ => {
                // 普通任务执行
                runtime.execute_task(&mut *session, TaskSpec::from(&task)).await?
            }
        };

        // 6. 归还 Session（复用）
        self.session_pool.release_session(session).await;

        Ok(TaskOutput {
            task_id: task.id,
            agent_type: agent.agent_type,
            result,
            session_used: session.id(),
        })
    }

    /// 任务类型转能力
    fn task_to_capability(task_type: &TaskType) -> Result<Capability> {
        match task_type {
            TaskType::ModuleRefactoring => Ok(Capability::ModuleRefactoring),
            TaskType::EngineCodeInjection => Ok(Capability::EngineCodeInjection),
            TaskType::PerformanceOptimization => Ok(Capability::PerformanceOptimization),
            TaskType::CodeReview => Ok(Capability::CodeReview),
            TaskType::TestWriting => Ok(Capability::TestWriting),
            TaskType::Documentation => Ok(Capability::Documentation),
        }
    }

    /// 选择最佳 Agent（简单负载均衡）
    fn select_best_agent(&self, agents: Vec<&AgentDescriptor>) -> Result<&AgentDescriptor> {
        // 简单实现：随机选择（后续可以升级为更复杂的负载均衡）
        agents.first()
            .ok_or_else(|| Error::NoAvailableAgent)
    }

    /// 创建 Runtime（辅助函数）
    fn create_runtime(descriptor: &AgentDescriptor) -> Result<Box<dyn AgentRuntime>> {
        (descriptor.factory)()
    }
}

/// 任务输出
#[derive(Debug, Clone)]
pub struct TaskOutput {
    pub task_id: String,
    pub agent_type: String,
    pub result: AgentResult,
    pub session_used: String,
}
```

### 4. 使用示例（可替换）

```rust
/// 使用示例 1：默认注册
let adapter = AgentAdapter::new(Arc::new(AgentRegistry::instance()));

// 执行任务
let task = TaskContext {
    id: "task-001".to_string(),
    name: "重构 scheduler".to_string(),
    task_type: TaskType::ModuleRefactoring,
    prompt_template: "重构以下模块...".to_string(),
    context_vars: HashMap::new(),
    engine_context: None,
    timeout_secs: Some(3600),
    dependencies: vec![],
};

let output = adapter.execute(task).await?;
println!("结果: {:?}", output.result);
```

```rust
/// 使用示例 2：注册自定义 Agent
// 1. 定义自定义 Agent
struct MyCustomAgent {
    config: MyConfig,
}

#[async_trait]
impl AgentExecutor for MyCustomAgent {
    async fn execute(&self, task: TaskContext) -> Result<TaskOutput> {
        // 自定义实现
        Ok(TaskOutput::new())
    }
}

// 2. 注册到全局 Registry
AgentRegistry::instance().register(AgentDescriptor {
    agent_type: "my-custom".to_string(),
    display_name: "My Custom Agent".to_string(),
    capabilities: vec![Capability::CodeReview],
    factory: || Ok(Box::new(MyCustomAgent::new()?)),
    config_schema: ConfigSchema::new(),
});

// 3. 立即可用，无需修改 Agent Pool 代码
```

---

## 总结

### 核心改进

1. **Agent 与任务解耦**
   - AgentExecutor trait：可替换的抽象接口
   - AgentDescriptor：Agent 注册表（工厂模式）
   - AgentAdapter：连接层（处理选择和执行）

2. **依赖倒置**
   - 高层模块（Agent Pool）依赖抽象接口
   - 低层模块（具体 Agent）实现接口
   - 可随时替换 Agent 实现

3. **可扩展性**
   - 通过注册机制添加新 Agent
   - 无需修改核心代码
   - 支持运行时动态加载 Agent

4. **Session 复用**
   - SessionPool 统一管理
   - 同一 Runtime 内复用 Session
   - 序列化/反序列化跨 Agent 传递

2. **Session 复用**
   - SessionPool 管理 Session 生命周期
   - 同一 Runtime 内复用 Session
   - 序列化/反序列化跨 Agent 传递
   - TTL 自动清理过期 Session

3. **引擎代码注入**
   - EngineCodeScanner 扫描确定范围
   - EngineCodeInjector 准备上下文
   - 文件大小限制（单文件1MB，总共10MB）
   - 只读目录和可注入目录分离

4. **智能任务分配**
   - 基于任务需求匹配 Team
   - 考虑 Agent 能力
   - 考虑负载均衡
   - 自动选择最佳 Team

---

**文档版本**: 1.0
**设计完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**审核状态**: 待审核
