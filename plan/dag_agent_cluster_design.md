# CIS DAG Agent Cluster 设计文档

## 概述

基于 CIS 现有架构的 DAG Agent Cluster 设计，实现单机多 Agent 并发执行，支持 CLI attach/detach 到任意 Agent session。

**设计原则：**
- 复用现有代码：`network::agent_session` 的 PTY 转发、`agent::AgentType`、`scheduler::DagRun`
- 最小侵入：作为独立模块 `agent::cluster` 实现，不改动现有 DAG 核心
- 单机优先：暂不实现 Matrix 分布式，专注单节点 Agent 集群

---

## 架构图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        CIS Single-Node DAG Scheduler                         │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     Existing: DagScheduler                          │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │    │
│  │  │    DagRun    │  │   DagRun     │  │        TaskDag           │  │    │
│  │  │  (active)    │  │  (paused)    │  │    (dependency graph)    │  │    │
│  │  └──────┬───────┘  └──────────────┘  └──────────────────────────┘  │    │
│  │         │                                                          │    │
│  │  poll_ready_tasks()                                                │    │
│  └─────────┼──────────────────────────────────────────────────────────┘    │
│            │                                                                 │
│  ┌─────────▼──────────────────────────────────────────────────────────┐    │
│  │                  NEW: AgentClusterExecutor                         │    │
│  │                                                                    │    │
│  │  ┌────────────────────────────────────────────────────────────┐   │    │
│  │  │                 SessionManager (全局单例)                   │   │    │
│  │  │  sessions: HashMap<SessionId, AgentSession>                │   │    │
│  │  │                                                            │   │    │
│  │  │  Session-001 (analyze) ──► PTY ──► Claude Process         │   │    │
│  │  │  Session-002 (update)  ──► PTY ──► Claude Process         │   │    │
│  │  │  Session-003 (test)    ──► PTY ──► Kimi Process           │   │    │
│  │  └────────────────────────────────────────────────────────────┘   │    │
│  │                              ▲                                     │    │
│  │         ┌────────────────────┼────────────────────┐                │    │
│  │         │                    │                    │                │    │
│  │  ┌──────▼──────┐    ┌───────▼───────┐    ┌──────▼──────┐         │    │
│  │  │   attach    │    │    attach     │    │   attach    │         │    │
│  │  │  (CLI user) │    │   (CLI user)  │    │  (CLI user) │         │    │
│  │  └─────────────┘    └───────────────┘    └─────────────┘         │    │
│  │                                                                    │    │
│  │  Features:                                                         │    │
│  │  - max_workers: 并发控制                                           │    │
│  │  - upstream context injection: 上游输出注入下游 prompt             │    │
│  │  - blockage detection: 卡点自动检测                                │    │
│  │  - auto-detach on block: 卡点时自动 detach 等待人工介入            │    │
│  └────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │              Shared: ContextStore (SQLite)                          │    │
│  │  - task_outputs: {run_id}/{task_id} -> output                       │    │
│  │  - checkpoints: DAG 状态持久化                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 可复用代码清单

### 1. PTY 与 Agent 进程管理
**来源：** `cis-core/src/network/agent_session.rs`

```rust
// 可直接复用的组件：
- AgentType (Claude, Kimi, Aider)
- SessionControlMessage (session_start, session_end, resize)
- AgentSession::spawn_agent_in_pty() - 在 PTY 中启动 Agent
- AgentSession::build_agent_command() - 构建 claude/kimi 命令
- PtyForwarder - WebSocket 转发（改造为 Unix Socket 或直接用）
- AgentSession::handle_pty_output() - 输出捕获与处理
```

**复用方式：**
- 提取 `AgentSession` 的核心逻辑到 `agent::cluster::session`
- 移除 WebSocket 依赖，改为本地 Unix Socket 或直接句柄管理
- 保留 PTY 创建和 Agent 命令构建逻辑

### 2. Agent Provider 接口
**来源：** `cis-core/src/agent/mod.rs`

```rust
// 已存在的接口：
- AgentType enum (Claude, Kimi, Aider, Custom)
- AgentRequest / AgentResponse - 请求/响应结构
- AgentContext - 上下文（work_dir, memory_access 等）
- AgentProvider trait - execute(), execute_stream(), available()
```

**复用方式：**
- `AgentClusterExecutor` 直接使用 `AgentType` 区分 Agent 类型
- 通过 `AgentProvider` 接口调用 Claude/Kimi
- 复用 `AgentContext` 传递工作目录和记忆访问配置

### 3. DAG 调度器
**来源：** `cis-core/src/scheduler/mod.rs`

```rust
// 核心结构：
- DagRun - DAG 运行实例（已有 task_commands, source_file）
- TaskDag - 依赖图（nodes, get_dependencies, mark_running/completed/failed）
- DagNodeStatus - Pending, Ready, Running, Completed, Failed, Blocked
- DagRunStatus - Running, Paused, Completed, Failed
```

**复用方式：**
- `AgentClusterExecutor` 操作现有 `DagRun`，不改动结构
- 添加 `Blocked` 状态到 `DagNodeStatus`（或复用 `Arbitrated`）
- 复用 `mark_running/completed/failed` 状态转换

### 4. 本地执行器
**来源：** `cis-core/src/scheduler/local_executor.rs`

```rust
// 可参考的实现：
- LocalExecutor::spawn_worker() - 进程启动
- WorkerInfo - 进程句柄管理
- ensure_worker() / check_workers() - 生命周期管理
```

**复用方式：**
- 参考 `WorkerInfo` 设计 `AgentWorker` 结构
- 复用进程启动和监控逻辑
- 扩展为支持多个并发 Worker

### 5. 项目会话管理
**来源：** `cis-core/src/project/session.rs`

```rust
// 可参考：
- ProjectSession - 项目上下文管理
- load_local_skills() - 加载本地技能
- AgentManager - Agent 管理
```

**复用方式：**
- 每个 DAG Run 关联一个 `ProjectSession` 上下文
- 复用 `AgentManager` 管理多个 Agent Provider

---

## 核心数据结构

### 1. SessionId
```rust
/// Agent Session 唯一标识
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SessionId {
    pub dag_run_id: String,
    pub task_id: String,
}

impl SessionId {
    pub fn new(run_id: &str, task_id: &str) -> Self {
        Self {
            dag_run_id: run_id.to_string(),
            task_id: task_id.to_string(),
        }
    }
    
    /// 短格式用于 CLI 显示: "a1b2c3d4:analyze"
    pub fn short(&self) -> String {
        format!("{}:{}", 
            &self.dag_run_id[..8.min(self.dag_run_id.len())], 
            self.task_id
        )
    }
}
```

### 2. AgentSession
```rust
/// Agent Session 状态
#[derive(Debug, Clone)]
pub enum SessionState {
    /// 刚创建，正在启动 PTY
    Spawning,
    /// 运行中，无人 attach
    RunningDetached { since: DateTime<Utc> },
    /// 有人 attach 中
    Attached { user: String, since: DateTime<Utc> },
    /// 检测到卡点，等待人工介入
    Blocked { reason: String, since: DateTime<Utc> },
    /// 任务完成
    Completed { output: String, exit_code: i32 },
    /// 任务失败
    Failed { error: String },
}

/// Agent Session（复用 network::agent_session 核心逻辑）
pub struct AgentSession {
    pub id: SessionId,
    pub state: SessionState,
    pub agent_type: AgentType,
    
    // PTY 相关（复用 portable_pty）
    pub pty_master: Box<dyn MasterPty>,
    pub pty_slave: Box<dyn ChildPty>,
    pub agent_process: tokio::process::Child,
    
    // 输出捕获
    pub output_buffer: Arc<RwLock<OutputBuffer>>,
    pub max_buffer_lines: usize,
    
    // 上下文
    pub work_dir: PathBuf,
    pub prompt: String,
    pub upstream_context: String,  // 上游任务输出
    
    // 元数据
    pub started_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

/// 输出缓冲区（带行数限制）
pub struct OutputBuffer {
    pub lines: Vec<String>,
    pub total_bytes: usize,
}
```

### 3. SessionManager
```rust
/// 全局 Session 管理器
pub struct SessionManager {
    /// 所有活跃 Session
    sessions: Arc<RwLock<HashMap<SessionId, AgentSession>>>,
    
    /// Session ID 到 PTY Master 的映射（用于 attach）
    pty_masters: Arc<RwLock<HashMap<SessionId, Box<dyn MasterPty>>>>,
    
    /// 配置
    config: SessionManagerConfig,
}

#[derive(Debug, Clone)]
pub struct SessionManagerConfig {
    pub socket_dir: PathBuf,           // Unix Socket 目录
    pub max_buffer_lines: usize,       // 输出缓冲区行数限制
    pub blockage_keywords: Vec<String>, // 卡点检测关键词
    pub default_timeout_secs: u64,
}

impl SessionManager {
    /// 获取全局单例
    pub fn global() -> &'static Self {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<SessionManager> = OnceLock::new();
        INSTANCE.get_or_init(|| Self::new(Default::default()))
    }
    
    /// 创建新 Session（启动 Agent PTY）
    pub async fn create_session(
        &self,
        dag_run_id: &str,
        task_id: &str,
        agent_type: AgentType,
        prompt: &str,
        work_dir: &Path,
        upstream_context: &str,
    ) -> Result<SessionId>;
    
    /// 列出所有 Sessions
    pub async fn list_sessions(&self) -> Vec<SessionSummary>;
    
    /// 列出特定 DAG Run 的 Sessions
    pub async fn list_sessions_by_dag(&self, dag_run_id: &str) -> Vec<SessionSummary>;
    
    /// Attach 到 Session（返回 PTY Master 用于交互）
    pub async fn attach_session(
        &self,
        session_id: &SessionId,
        user: &str,
    ) -> Result<AttachHandle>;
    
    /// Detach（在 AttachHandle drop 时自动调用）
    pub async fn detach_session(&self, session_id: &SessionId) -> Result<()>;
    
    /// 标记卡点（由监控任务调用）
    pub async fn mark_blocked(
        &self, 
        session_id: &SessionId, 
        reason: &str
    ) -> Result<()>;
    
    /// 标记完成
    pub async fn mark_completed(
        &self,
        session_id: &SessionId,
        output: &str,
        exit_code: i32,
    ) -> Result<()>;
    
    /// 获取 Session 输出（用于非 attach 时查看）
    pub async fn get_output(&self, session_id: &SessionId) -> Result<String>;
}

pub struct SessionSummary {
    pub id: String,           // short format
    pub task_id: String,
    pub dag_run_id: String,
    pub agent_type: AgentType,
    pub state: String,
    pub runtime: Duration,
    pub output_preview: String,
}
```

### 4. AgentClusterExecutor
```rust
/// Agent 集群执行器（整合到 DAG 执行流程）
pub struct AgentClusterExecutor {
    session_manager: &'static SessionManager,
    max_workers: usize,
}

impl AgentClusterExecutor {
    pub fn new(max_workers: usize) -> Self {
        Self {
            session_manager: SessionManager::global(),
            max_workers,
        }
    }
    
    /// 执行单个 DAG Run（主入口）
    pub async fn execute_run(&self, run: &mut DagRun) -> Result<ExecutionReport> {
        loop {
            // 1. 检查当前运行中的 Worker 数
            let active_count = self.count_active_sessions(&run.run_id).await;
            let available_slots = self.max_workers.saturating_sub(active_count);
            
            // 2. 获取 Ready 任务
            let ready_tasks = self.get_ready_tasks(run);
            
            // 3. 启动新的 Agent Sessions（受并发限制）
            for (task_id, command) in ready_tasks.iter().take(available_slots) {
                // 构建完整 prompt（注入上游上下文）
                let upstream = self.prepare_upstream_context(run, task_id).await;
                let full_prompt = self.build_task_prompt(command, &upstream);
                
                // 标记 Running
                run.dag.mark_running(task_id.clone())?;
                
                // 创建 Session（启动 Agent PTY）
                let session_id = self.session_manager.create_session(
                    &run.run_id,
                    task_id,
                    AgentType::Claude,  // 从 task 配置读取
                    &full_prompt,
                    &self.get_work_dir(run, task_id),
                    &upstream,
                ).await?;
                
                // 启动后台监控任务
                self.spawn_monitor_task(session_id.clone(), run.run_id.clone());
            }
            
            // 4. 检查所有 Sessions 状态
            self.poll_sessions(run).await?;
            
            // 5. 检查是否完成
            if self.is_run_complete(run) {
                break;
            }
            
            // 6. 等待一小段时间再轮询
            sleep(Duration::from_millis(500)).await;
        }
        
        Ok(self.build_report(run))
    }
    
    /// 监控单个 Session（后台任务）
    async fn monitor_session(&self, session_id: SessionId, run_id: String) {
        loop {
            match self.session_manager.get_state(&session_id).await {
                Ok(SessionState::Completed { output, .. }) => {
                    // 更新 DAG 状态
                    self.update_task_completed(&run_id, &session_id.task_id, output).await;
                    break;
                }
                Ok(SessionState::Failed { error }) => {
                    self.update_task_failed(&run_id, &session_id.task_id, error).await;
                    break;
                }
                Ok(SessionState::Blocked { reason, .. }) => {
                    // 卡点：暂停下游，等待人工介入
                    self.pause_downstream(&run_id, &session_id.task_id).await;
                    // 等待状态恢复
                    self.wait_for_recovery(&session_id).await;
                }
                Ok(SessionState::Attached { .. }) => {
                    // 有人 attach，等待恢复运行
                    sleep(Duration::from_secs(1)).await;
                }
                _ => {
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
    
    /// 准备上游上下文（注入下游 prompt）
    async fn prepare_upstream_context(&self, run: &DagRun, task_id: &str) -> String {
        let mut context = String::new();
        
        // 从 ContextStore 加载所有依赖的输出
        if let Some(deps) = run.dag.get_dependencies(task_id) {
            for dep_id in deps {
                if let Ok(output) = ContextStore::load(&run.run_id, &dep_id).await {
                    context.push_str(&format!("\n## Output from {}:\n{}\n", dep_id, output));
                }
            }
        }
        
        context
    }
}
```

### 5. AttachHandle
```rust
/// Attach 句柄（类似 ssh session）
pub struct AttachHandle {
    session_id: SessionId,
    pty_master: Box<dyn MasterPty>,
    
    // 转发任务句柄
    stdin_task: JoinHandle<()>,
    stdout_task: JoinHandle<()>,
}

impl AttachHandle {
    /// 进入交互模式（阻塞直到 detach）
    pub async fn interact(&mut self) -> Result<()> {
        // 等待转发任务完成（用户 Ctrl+D 或 session 结束）
        tokio::select! {
            _ = &mut self.stdin_task => {},
            _ = &mut self.stdout_task => {},
        }
        Ok(())
    }
    
    /// 调整终端大小
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.pty_master.resize(PtySize { cols, rows, .. })
    }
}

impl Drop for AttachHandle {
    fn drop(&mut self) {
        // 自动 detach
        tokio::spawn(async move {
            SessionManager::global()
                .detach_session(&self.session_id)
                .await
        });
    }
}
```

---

## 卡点检测机制

### 1. 基于输出的卡点检测
```rust
impl AgentSession {
    /// 监控输出，检测卡点
    async fn monitor_output(&self) {
        let buffer = self.output_buffer.clone();
        let blockage_keywords = vec![
            "?", "confirm", "yes/no", "y/n",
            "enter to continue", "press any key",
            "authentication required", "password:",
            "merge conflict", "rebase conflict",
        ];
        
        loop {
            let lines = buffer.read().await.lines.clone();
            
            // 检查最近几行
            for line in lines.iter().rev().take(5) {
                if blockage_keywords.iter().any(|kw| line.to_lowercase().contains(kw)) {
                    // 触发卡点
                    self.session_manager.mark_blocked(
                        &self.id,
                        &format!("Detected blockage: {}", line)
                    ).await;
                    return;
                }
            }
            
            sleep(Duration::from_millis(200)).await;
        }
    }
}
```

### 2. 卡点恢复
```rust
impl SessionManager {
    /// 用户 attach 处理完卡点后恢复
    pub async fn mark_recovered(&self, session_id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            match &session.state {
                SessionState::Blocked { .. } | SessionState::Attached { .. } => {
                    session.state = SessionState::RunningDetached { 
                        since: Utc::now() 
                    };
                    // 发送恢复信号到 Agent（如需要）
                    self.send_input(session_id, "\n").await?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
```

---

## CLI 接口

### 1. Sessions 列表
```bash
$ cis dag sessions
SESSION ID          TASK        DAG RUN     AGENT    STATUS       RUNTIME    PREVIEW
────────────────────────────────────────────────────────────────────────────────────────
a1b2c3d4:analyze    analyze     a1b2c3d4    claude   Blocked      5m32s      Merge conflict...
a1b2c3d4:update     update      a1b2c3d4    claude   Running      2m10s      Updating Cargo.toml...
a1b2c3d4:test       test        a1b2c3d4    kimi     Waiting      -          (blocked by update)

$ cis dag sessions --dag a1b2c3d4 --all
...
```

### 2. Attach
```bash
# 通过 session ID attach（短 ID）
$ cis dag attach a1b2c3d4:analyze
[Attaching to session a1b2c3d4:analyze (task: analyze)...]
[Agent output...]
[Interactive Claude session - Ctrl+B D to detach, Ctrl+B K to kill]

# 通过 dag run + task attach
$ cis dag attach --run a1b2c3d4 --task analyze

# 强制 attach（踢掉当前用户）
$ cis dag attach --force a1b2c3d4:analyze

# 只读模式 attach（不抢占）
$ cis dag attach --readonly a1b2c3d4:analyze
```

### 3. 其他命令
```bash
# 查看输出（非交互）
$ cis dag logs a1b2c3d4:analyze
$ cis dag logs a1b2c3d4:analyze --tail 50
$ cis dag logs a1b2c3d4:analyze --follow

# 发送输入（非 attach 时）
$ cis dag input a1b2c3d4:analyze "yes"

# 强制标记恢复（卡点解除）
$ cis dag unblock a1b2c3d4:analyze

# 杀死 Session
$ cis dag kill a1b2c3d4:analyze

# 批量操作
$ cis dag kill-all --dag a1b2c3d4
```

---

## 文件结构

```
cis-core/src/agent/cluster/
├── mod.rs              # 模块导出
├── manager.rs          # SessionManager
├── session.rs          # AgentSession, SessionState
├── attach.rs           # AttachHandle, PTY 转发
├── executor.rs         # AgentClusterExecutor
├── context.rs          # ContextStore, 上游注入
├── monitor.rs          # 卡点检测, 输出监控
└── config.rs           # SessionManagerConfig

cis-node/src/commands/dag/
├── mod.rs              # 现有 dag 命令
├── sessions.rs         # cis dag sessions
├── attach.rs           # cis dag attach
├── logs.rs             # cis dag logs
└── kill.rs             # cis dag kill
```

---

## 与现有代码的整合点

### 1. 启动时初始化
```rust
// cis-node/src/main.rs
fn main() {
    // ...
    
    // 初始化 SessionManager（全局单例）
    let _ = SessionManager::global();
    
    // ...
}
```

### 2. DAG 执行器选择
```rust
// cis-node/src/commands/dag.rs
pub async fn execute_run(run_id: Option<&str>, use_agent: bool) -> Result<()> {
    if use_agent {
        // 使用 Agent Cluster
        let executor = AgentClusterExecutor::new(4); // max 4 workers
        executor.execute_run(&mut run).await?;
    } else {
        // 使用现有 shell 执行
        // ...
    }
}
```

### 3. TOML 扩展
```toml
[dag]
policy = "all_success"
max_workers = 4              # 并发限制
auto_attach_on_block = true  # 卡点时是否自动弹出 attach

[[dag.tasks]]
id = "analyze"
name = "Analyze Dependencies"
agent = "claude"             # claude | kimi | aider | shell
prompt = "分析 Cargo.toml 依赖"
work_dir = "/tmp/cis/dag-001/analyze"
deps = []
level = { type = "mechanical", retry = 3 }

[[dag.tasks]]
id = "update"
name = "Update Code"
agent = "claude"
prompt = "根据分析结果更新代码"
# 自动注入 analyze 的输出到 prompt
deps = ["analyze"]
work_dir = "/tmp/cis/dag-001/update"
```

---

## 实现路线图

### Phase 1: 基础 Session 管理
- [ ] 提取 `network::agent_session` 到 `agent::cluster`
- [ ] 实现 `SessionManager` 和 `AgentSession`
- [ ] 实现 `create_session` 和 `list_sessions`
- [ ] CLI: `cis dag sessions`

### Phase 2: Attach/Detach
- [ ] PTY Master/Slave 管理
- [ ] 实现 `AttachHandle` 和 PTY 转发
- [ ] CLI: `cis dag attach`
- [ ] 信号处理（Ctrl+B D detach, Ctrl+B K kill）

### Phase 3: DAG 集成
- [ ] 实现 `AgentClusterExecutor`
- [ ] 并发控制（max_workers）
- [ ] 状态轮询和 DAG 更新
- [ ] CLI: `cis dag up` (后台运行)

### Phase 4: 上下文传递
- [ ] 实现 `ContextStore`
- [ ] 上游输出注入下游 prompt
- [ ] Markdown 格式化

### Phase 5: 卡点处理
- [ ] 卡点检测（关键词匹配）
- [ ] 自动 detach + 通知
- [ ] `cis dag unblock` 恢复
- [ ] 邮件/Webhook 通知（可选）

### Phase 6: 优化
- [ ] 性能优化（减少轮询）
- [ ] Web UI 查看 sessions
- [ ] Matrix Room 同步（联邦记忆）

---

## 附录：复用代码详细说明

### A. PTY 创建（来自 agent_session.rs）
```rust
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

fn create_pty(cols: u16, rows: u16) -> Result<(Box<dyn MasterPty>, Box<dyn ChildPty>)> {
    let pty_system = NativePtySystem::default();
    let pair = pty_system.openpty(PtySize {
        cols,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    Ok((pair.master, pair.slave))
}
```

### B. Agent 命令构建（来自 agent_session.rs）
```rust
fn build_agent_command(&self, agent_type: AgentType) -> Result<CommandBuilder> {
    match agent_type {
        AgentType::Claude => {
            let mut cmd = CommandBuilder::new("claude");
            cmd.arg("--dangerously-skip-permissions");
            Ok(cmd)
        }
        AgentType::Kimi => {
            let mut cmd = CommandBuilder::new("kimi");
            cmd.arg("--dangerously-skip-permissions");
            Ok(cmd)
        }
        // ...
    }
}
```

### C. 输出转发（来自 agent_session.rs）
```rust
async fn forward_pty_to_websocket(
    mut master: Box<dyn MasterPty>,
    ws_tx: mpsc::Sender<Message>,
) -> Result<()> {
    let mut reader = master.try_clone_reader()?;
    let mut buf = [0u8; 1024];
    
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                let data = buf[..n].to_vec();
                ws_tx.send(Message::Binary(data)).await?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
```

这些都可以直接复用或稍作改造用于本地 Unix Socket 或 direct PTY attach。


---

## CLI / GUI / API 复用设计

本设计天然支持 **CLI / GUI / API** 三种管理形式，核心是 **`SessionManager` 作为统一的状态中心**。

### 三层架构图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Presentation Layer                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────────────┐  │
│  │     CLI      │  │  GUI (Web)   │  │           API Server             │  │
│  │  (cis dag)   │  │  (React/Vue) │  │        (REST + WebSocket)        │  │
│  └──────┬───────┘  └──────┬───────┘  └──────────────┬───────────────────┘  │
│         │                 │                         │                      │
│         │  stdin/stdout   │   HTTP/WebSocket        │   HTTP/REST          │
│         │                 │                         │                      │
└─────────┼─────────────────┼─────────────────────────┼──────────────────────┘
          │                 │                         │
          └─────────────────┴─────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────────────────┐
│                        SessionManager Layer                                  │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                  SessionManager (全局单例)                          │   │
│   │                                                                     │   │
│   │   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                │   │
│   │   │  Session-1  │  │  Session-2  │  │  Session-N  │                │   │
│   │   │  (Claude)   │  │   (Kimi)    │  │   (Shell)   │                │   │
│   │   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                │   │
│   │          │                │                │                        │   │
│   │          └────────────────┴────────────────┘                        │   │
│   │                           │                                         │   │
│   │   Methods:                                                         │   │
│   │   - list_sessions() → Vec<SessionSummary>                          │   │
│   │   - create_session(...) → SessionId                                │   │
│   │   - attach_session(id) → AttachHandle                              │   │
│   │   - get_output(id) → String                                        │   │
│   │   - kill_session(id)                                               │   │
│   │   - mark_blocked/unblock                                           │   │
│   │                                                                     │   │
│   │   Events (tokio::sync::broadcast):                                 │   │
│   │   - SessionCreated(SessionSummary)                                 │   │
│   │   - OutputUpdated(SessionId, String)                               │   │
│   │   - StateChanged(SessionId, SessionState)                          │   │
│   │   - SessionCompleted(SessionId, Result)                            │   │
│   │                                                                     │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                         Execution Layer                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │AgentSession 1│  │AgentSession 2│  │  PTY Master  │  │ ContextStore     │ │
│  │  + Claude    │  │   + Kimi     │  │  (for attach)│  │  (SQLite)        │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 各层适配方案

#### 1. CLI（已实现）

**复用方式：直接调用 `SessionManager`**

```rust
// cis-node/src/commands/dag/sessions.rs
pub async fn list_sessions(dag_filter: Option<&str>) -> Result<()> {
    let manager = SessionManager::global();
    let sessions = manager.list_sessions().await;
    
    // 格式化为表格输出
    for s in sessions {
        println!("{:<20} {:<10} {:<10}", s.id, s.state, s.runtime);
    }
}

// cis-node/src/commands/dag/attach.rs
pub async fn attach_session(session_id: &str) -> Result<()> {
    let manager = SessionManager::global();
    let id = parse_session_id(session_id)?;
    
    // 创建 AttachHandle，进入交互模式
    let mut handle = manager.attach_session(&id, "cli-user").await?;
    
    // 设置终端 raw mode
    enable_raw_mode()?;
    handle.interact().await?;
    disable_raw_mode()?;
}
```

**特点：**
- 直接 PTY attach，零拷贝转发
- SIGWINCH 信号处理终端 resize
- Ctrl+B D 优雅 detach

---

#### 2. GUI (Web)

**复用方式：HTTP API + WebSocket 事件流**

```rust
// cis-gui/src/server/api/sessions.rs
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

// REST API
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/:id", get(get_session))
        .route("/api/sessions/:id/kill", post(kill_session))
        .route("/api/sessions/:id/unblock", post(unblock_session))
        .route("/ws/sessions/:id", get(attach_websocket))
}

async fn list_sessions(State(state): State<AppState>) -> Json<Vec<SessionSummary>> {
    let manager = SessionManager::global();
    Json(manager.list_sessions().await)
}

// WebSocket attach（替代 PTY）
async fn attach_websocket(
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_attach(socket, id))
}

async fn handle_ws_attach(mut socket: WebSocket, session_id: String) {
    let manager = SessionManager::global();
    let id = parse_session_id(&session_id).unwrap();
    
    // 订阅事件
    let mut events = manager.subscribe_events(&id);
    
    // 启动转发任务
    tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            let msg = match event {
                SessionEvent::OutputUpdated(_, data) => {
                    Message::Text(json!({ "type": "output", "data": data }).to_string())
                }
                SessionEvent::StateChanged(_, state) => {
                    Message::Text(json!({ "type": "state", "state": state }).to_string())
                }
                // ...
            };
            socket.send(msg).await.ok();
        }
    });
}
```

**前端组件示例：**

```typescript
// GUI: SessionList.tsx
function SessionList() {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  
  useEffect(() => {
    // 初始加载
    fetch('/api/sessions').then(r => r.json()).then(setSessions);
    
    // WebSocket 实时更新
    const ws = new WebSocket('/ws/events');
    ws.onmessage = (e) => {
      const event = JSON.parse(e.data);
      if (event.type === 'SessionCreated') {
        setSessions(prev => [...prev, event.data]);
      }
    };
  }, []);
  
  return (
    <div>
      {sessions.map(s => (
        <SessionCard 
          key={s.id} 
          session={s}
          onAttach={() => openTerminal(s.id)}  // 打开 xterm.js
        />
      ))}
    </div>
  );
}

// GUI: Terminal.tsx (使用 xterm.js)
function Terminal({ sessionId }) {
  const terminal = useRef(new Terminal());
  
  useEffect(() => {
    const ws = new WebSocket(`/ws/sessions/${sessionId}`);
    const fitAddon = new FitAddon();
    terminal.current.loadAddon(fitAddon);
    
    // WebSocket -> Terminal
    ws.onmessage = (e) => {
      const msg = JSON.parse(e.data);
      if (msg.type === 'output') {
        terminal.current.write(msg.data);
      }
    };
    
    // Terminal -> WebSocket (用户输入)
    terminal.current.onData(data => {
      ws.send(JSON.stringify({ type: 'input', data }));
    });
    
    // 窗口 resize
    window.addEventListener('resize', () => {
      fitAddon.fit();
      ws.send(JSON.stringify({ 
        type: 'resize', 
        cols: terminal.current.cols,
        rows: terminal.current.rows 
      }));
    });
  }, [sessionId]);
  
  return <div ref={ref} />;
}
```

**GUI 专属功能：**
- 可视化 DAG 图（依赖关系 + 实时状态）
- 多窗口分屏（同时 attach 多个 sessions）
- 输出搜索/过滤
- 一键 unblock（按钮代替命令）

---

#### 3. API (HTTP/REST)

**复用方式：包装 `SessionManager` 为 REST API**

```rust
// cis-node/src/server/api/mod.rs 或独立 crate
pub struct ApiServer {
    session_manager: &'static SessionManager,
}

impl ApiServer {
    // 同步接口（返回当前状态）
    pub async fn create_dag_run(&self, req: CreateDagRequest) -> Result<DagRunInfo> {
        // 创建 DagRun，启动 AgentClusterExecutor
    }
    
    pub async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.session_manager.list_sessions().await
    }
    
    pub async fn get_session_output(&self, id: &SessionId) -> Result<String> {
        self.session_manager.get_output(id).await
    }
    
    // 异步接口（Webhook 或轮询）
    pub async fn attach_session_http(&self, id: &SessionId) -> Result<AttachToken> {
        // 创建 attach token，有效期 5 分钟
        // 客户端用 token 连接 WebSocket
    }
}
```

**OpenAPI 规范示例：**

```yaml
# api.yaml
paths:
  /api/v1/sessions:
    get:
      summary: List all agent sessions
      responses:
        200:
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/SessionSummary'
                  
  /api/v1/sessions/{id}/attach:
    post:
      summary: Attach to a session (get WebSocket URL)
      responses:
        200:
          content:
            application/json:
              schema:
                type: object
                properties:
                  ws_url: string
                  token: string
                  
  /ws/v1/sessions/{id}:
    get:
      summary: WebSocket for real-time I/O
      parameters:
        - name: token
          in: query
          required: true
```

**API 客户端示例：**

```python
# Python SDK
import cis_sdk

client = cis_sdk.Client("http://localhost:8080")

# 创建 DAG Run
run = client.dag.create("dag.toml", name="refactor")

# 获取 sessions
sessions = client.sessions.list(dag_run=run.id)
for s in sessions:
    print(f"{s.task_id}: {s.state}")

# WebSocket attach（实时 I/O）
with client.sessions.attach(sessions[0].id) as ws:
    ws.send("yes\n")  # 回复卡点
    for msg in ws:
        print(msg.output)
```

---

### 共享能力矩阵

| 功能 | CLI | GUI | API | 实现位置 |
|------|-----|-----|-----|----------|
| **List sessions** | ✅ `cis dag sessions` | ✅ 表格视图 | ✅ `GET /api/sessions` | `SessionManager::list_sessions()` |
| **Attach** | ✅ PTY raw mode | ✅ xterm.js | ✅ WebSocket | `SessionManager::attach_session()` |
| **View output** | ✅ `cis dag logs` | ✅ 终端面板 | ✅ `GET /api/sessions/{id}/output` | `SessionManager::get_output()` |
| **Kill session** | ✅ `cis dag kill` | ✅ ❌ 按钮 | ✅ `POST /api/sessions/{id}/kill` | `SessionManager::kill_session()` |
| **Unblock** | ✅ `cis dag unblock` | ✅ 一键恢复 | ✅ `POST /api/sessions/{id}/unblock` | `SessionManager::mark_recovered()` |
| **Create run** | ✅ `cis dag run` | ✅ 上传 TOML | ✅ `POST /api/dag-runs` | `AgentClusterExecutor::execute_run()` |
| **Events** | ✅ 轮询/阻塞 | ✅ WebSocket | ✅ WebSocket | `tokio::sync::broadcast` |

---

### 关键复用设计

#### 1. 事件总线（跨层通知）

```rust
// SessionManager 内部使用广播通道
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, AgentSession>>>,
    event_tx: broadcast::Sender<SessionEvent>,  // 所有层订阅
}

pub enum SessionEvent {
    Created(SessionId, SessionSummary),
    OutputUpdated(SessionId, String),
    StateChanged(SessionId, SessionState),
    Completed(SessionId, Result<String, String>),
}

// CLI: 静默忽略事件（轮询状态）
// GUI: WebSocket 转发事件
// API: Webhook 推送或 SSE
```

#### 2. 统一的 Attach 抽象

```rust
// 底层统一：AttachHandle
pub struct AttachHandle {
    session_id: SessionId,
    input_tx: mpsc::Sender<String>,      // 发送输入
    output_rx: mpsc::Receiver<String>,   // 接收输出
    state_rx: watch::Receiver<SessionState>,
}

// CLI: 映射到 stdin/stdout + termios
// GUI: 映射到 WebSocket + xterm.js
// API: 映射到 WebSocket + JSON
```

#### 3. 状态存储共享

```rust
// ContextStore: CLI/GUI/API 共享数据
pub struct ContextStore {
    db: SqlitePool,  // 持久化
    cache: Arc<DashMap<String, String>>,  // 内存缓存
}

// 所有层通过 ContextStore 读取：
// - 上游任务输出
// - Checkpoint 状态
// - 日志历史
```

---

### 目录结构（支持三层）

```
cis-core/src/agent/cluster/
├── lib.rs           # 核心导出
├── manager.rs       # SessionManager (无界面依赖)
├── session.rs       # AgentSession
├── attach.rs        # AttachHandle (抽象 I/O)
├── events.rs        # SessionEvent 广播
├── executor.rs      # AgentClusterExecutor
└── store.rs         # ContextStore

cis-node/src/
├── commands/dag/    # CLI 命令
│   ├── sessions.rs
│   ├── attach.rs
│   └── logs.rs
└── server/api/      # HTTP API
    ├── mod.rs
    ├── sessions.rs  # REST 路由
    └── ws.rs        # WebSocket 处理

cis-gui/src/
├── api.ts           # 前端 API 客户端
├── components/
│   ├── SessionList.tsx
│   ├── Terminal.tsx  # xterm.js 封装
│   └── DagGraph.tsx  # DAG 可视化
└── hooks/
    └── useSession.ts # WebSocket 管理
```

### 依赖关系

```
CLI  -> SessionManager (直接)
GUI  -> HTTP API -> SessionManager
API  -> SessionManager (包装)
```

---

## 总结

**核心结论：** 该设计完全支持 CLI / GUI / API 三种形式，核心逻辑在 `SessionManager`，上层仅需做协议适配（PTY/WebSocket/HTTP）。

**开发优先级建议：**
1. **Phase 1:** 先实现 CLI 版本（直接调用 SessionManager）
2. **Phase 2:** 添加事件总线和 WebSocket 支持
3. **Phase 3:** 包装 HTTP API 和 GUI

所有三层共享同一套 Session 状态和执行逻辑，零代码重复。
