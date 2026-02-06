这是 **"单机版 Kimi Agent 集群"** —— 把分布式编排压缩到单进程 DAG 调度器，类似本地 Airflow，但执行单元是 LLM Agent。

## 架构：单进程 DAG 调度器

```
┌─────────────────────────────────────────┐
│  CIS Single-Node Scheduler (Rust/Tokio) │
│  ┌──────────────────────────────────┐   │
│  │ DAG Engine (拓扑排序 + 状态机)     │   │
│  │  • 解析依赖图                     │   │
│  │  • 维护 TaskQueue (Ready状态)     │   │
│  │  • 处理失败重试/回滚               │   │
│  └──────────────┬───────────────────┘   │
│                 │                       │
│  ┌──────────────▼───────────────────┐   │
│  │ Worker Pool (并发控制)            │   │
│  │  ┌─────────┐ ┌─────────┐         │   │
│  │  │Agent-001│ │Agent-002│ ...     │   │
│  │  │(进程1)  │ │(进程2)  │         │   │
│  │  │Port:8001│ │Port:8002│         │   │
│  │  └────┬────┘ └────┬────┘         │   │
│  │       │           │               │   │
│  │  Unix Socket Control Interface   │   │
│  └───────┼───────────┼───────────────┘   │
│          │           │                   │
│  ┌───────▼───────────▼───────────────┐   │
│  │ Shared Context Store (SQLite/FS)  │   │
│  │  • 上游输出 → 下游输入             │   │
│  │  • Checkpoint 持久化               │   │
│  └───────────────────────────────────┘   │
└──────────────────────────────────────────┘
```

## 核心实现

```rust
// src/dag/scheduler.rs
use std::collections::{HashMap, VecDeque};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};
use petgraph::graph::{DiGraph, NodeIndex};

pub struct LocalAgentCluster {
    // DAG 定义
    graph: DiGraph<DagNode, ()>,
    // 任务状态
    states: RwLock<HashMap<NodeIndex, TaskState>>,
    // 进程池 pid -> AgentHandle
    workers: RwLock<HashMap<String, AgentHandle>>,
    // 并发限制
    max_workers: usize,
    // 上下文存储
    context_store: ContextStore,
}

#[derive(Clone)]
pub struct DagNode {
    pub id: String,
    pub prompt: String,
    pub agent_type: AgentType, // Claude | Kimi
    pub deps: Vec<String>,
    pub workspace: PathBuf,
    pub timeout: Duration,
}

pub struct AgentHandle {
    pub id: String,
    pub process: Child,
    pub socket_path: PathBuf,
    pub state: Arc<RwLock<AgentState>>,
    // 监控句柄
    pub monitor: JoinHandle<()>,
}

impl LocalAgentCluster {
    pub async fn execute(&self, dag: Vec<DagNode>) -> Result<ExecutionReport> {
        // 1. 构建图
        let mut graph = DiGraph::new();
        let mut id_to_idx = HashMap::new();
        
        for node in &dag {
            let idx = graph.add_node(node.clone());
            id_to_idx.insert(node.id.clone(), idx);
        }
        
        // 添加边（依赖关系）
        for node in &dag {
            let target = id_to_idx[&node.id];
            for dep in &node.deps {
                let source = id_to_idx[dep];
                graph.add_edge(source, target, ());
            }
        }
        
        // 2. 拓扑排序执行
        let mut ready_queue: VecDeque<NodeIndex> = graph
            .node_indices()
            .filter(|n| graph.neighbors_directed(*n, petgraph::Incoming).count() == 0)
            .collect();
            
        let mut running = FuturesUnordered::new();
        
        while !ready_queue.is_empty() || !running.is_empty() {
            // 启动 Ready 任务（受并发限制）
            while running.len() < self.max_workers && !ready_queue.is_empty() {
                let idx = ready_queue.pop_front().unwrap();
                let node = graph[idx].clone();
                
                // 准备上下文（注入上游输出）
                let context = self.prepare_context(&node, &graph).await;
                let prompt = format!("{}\n\n[上游上下文]\n{}", node.prompt, context);
                
                // 启动 Agent 进程
                let handle = self.spawn_agent(node, prompt).await?;
                let id = handle.id.clone();
                self.workers.write().await.insert(id.clone(), handle);
                
                running.push(self.monitor_task(id, idx));
            }
            
            // 等待任一任务完成
            if let Some((result, idx)) = running.next().await {
                match result {
                    Ok(output) => {
                        self.save_output(idx, &output).await;
                        // 检查下游任务是否就绪
                        for neighbor in graph.neighbors_directed(idx, petgraph::Outgoing) {
                            if self.all_deps_completed(neighbor, &graph).await {
                                ready_queue.push_back(neighbor);
                            }
                        }
                    }
                    Err(e) => {
                        // 卡点/失败处理
                        if e.is_blockage() {
                            self.pause_downstream(idx, &graph).await;
                            self.alert_human(&graph[idx], &e).await;
                            // 阻塞等待人工介入（通过 Unix Socket）
                            self.wait_for_intervention(&graph[idx]).await?;
                            // 恢复后重新加入队列
                            ready_queue.push_front(idx);
                        } else {
                            // 真失败，DAG 中止或走失败分支
                            return Err(e);
                        }
                    }
                }
            }
        }
        
        Ok(ExecutionReport { ... })
    }

    async fn spawn_agent(&self, node: DagNode, prompt: String) -> Result<AgentHandle> {
        let id = format!("{}-{}", node.id, uuid::Uuid::new_v4());
        let socket_path = std::env::temp_dir().join(format!("cis-{}.sock", id));
        let workspace = node.workspace.clone();
        
        // 创建隔离工作区
        fs::create_dir_all(&workspace).await?;
        
        // 启动 Agent Daemon（之前的 Daemon 设计）
        let child = Command::new("claude") // 或 kimi-code
            .arg("--dangerously-skip-permissions")
            .arg("--headless-socket") // 假设参数：监听 Unix Socket 控制
            .arg(&socket_path)
            .current_dir(&workspace)
            .env("CIS_AGENT_ID", &id)
            .spawn()?;
            
        // 等待 Socket 就绪
        tokio::time::timeout(Duration::from_secs(10), async {
            while !socket_path.exists() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await?;
        
        // 启动监控（之前实现的 Monitor）
        let state = Arc::new(RwLock::new(AgentState::Running));
        let monitor = tokio::spawn(
            AgentMonitor::new(id.clone(), socket_path.clone(), state.clone()).run()
        );
        
        // 发送初始任务
        self.send_to_socket(&socket_path, &prompt).await?;
        
        Ok(AgentHandle {
            id,
            process: child,
            socket_path,
            state,
            monitor,
        })
    }

    async fn monitor_task(&self, agent_id: String, idx: NodeIndex) -> Result<String, Error> {
        let handle = self.workers.read().await.get(&agent_id).cloned();
        
        loop {
            // 检查状态
            let state = handle.as_ref().unwrap().state.read().await.clone();
            match state {
                AgentState::Completed(output) => {
                    return Ok(output);
                }
                AgentState::Blocked(reason) => {
                    return Err(Error::Blockage(reason));
                }
                AgentState::Failed(e) => {
                    return Err(Error::ExecutionFailed(e));
                }
                AgentState::Running => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn wait_for_intervention(&self, node: &DagNode) -> Result<()> {
        println!("🚧 Agent {} 卡住，等待人工介入...", node.id);
        println!("执行: cis attach {}", node.id);
        
        // 阻塞直到 Monitor 报告状态变为 Recovered
        loop {
            if let Some(handle) = self.workers.read().await.get(&node.id) {
                let state = handle.state.read().await;
                if matches!(*state, AgentState::Recovered | AgentState::Running) {
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
```

## 上下文传递（Pipe 机制）

```rust
// 上游 Agent 的输出自动注入下游
impl ContextStore {
    pub async fn prepare(&self, node: &DagNode, graph: &DiGraph<DagNode, ()>) -> String {
        let mut context = String::new();
        
        // 收集所有直接依赖的输出
        for dep_idx in graph.neighbors_directed(graph[node.id], petgraph::Incoming) {
            let dep_node = &graph[dep_idx];
            if let Ok(output) = self.load(&dep_node.id).await {
                context.push_str(&format!("\n## 来自 {} 的输出:\n{}\n", dep_node.id, output));
            }
        }
        
        context
    }
    
    // 保存时自动向 Matrix Room 广播（联邦记忆）
    pub async fn save(&self, node_id: &str, output: &str) {
        fs::write(self.dir.join(format!("{}.md", node_id)), output).await.ok();
        
        // 同步到 CIS 联邦
        self.matrix.send_to_room(&format!("checkpoint:{}", node_id), output).await.ok();
    }
}
```

## DAG 定义示例（YAML）

```yaml
# dag.yaml
agents:
  - id: analyze-deps
    type: claude
    prompt: "分析 Cargo.toml 依赖，找出可升级项"
    workspace: /tmp/cis/dag-001/analyze
    
  - id: update-code  
    type: claude
    prompt: "根据依赖分析结果，执行 cargo update 并修复 API 变更"
    deps: [analyze-deps]
    workspace: /tmp/cis/dag-001/update
    
  - id: test-build
    type: claude  
    prompt: "运行 cargo test 和 cargo clippy"
    deps: [update-code]
    workspace: /tmp/cis/dag-001/test
    timeout: 300

  - id: notify
    type: shell  # 也可以是本地脚本
    prompt: "echo 'DAG 执行完成'"
    deps: [test-build]
```

## CLI 交互（docker-compose 风格）

```bash
# 启动 DAG（后台执行）
cis dag up -f dag.yaml --name refactor-project

# 查看实时状态（类似 docker ps）
cis dag ps
# ID              AGENT          STATUS     OUTPUT
# refactor-001    analyze-deps   Running    (2/10 files)
# refactor-002    update-code    Waiting    (blocked by analyze-deps)

# 查看日志流
cis dag logs -f refactor-001

# 介入卡住的 Agent（自动暂停下游）
cis dag attach refactor-project analyze-deps
# [进入交互式 Claude，处理完后 Ctrl+\ 退出]
# 下游 update-code 自动继续

# 手动触发重试（如果失败）
cis dag retry refactor-project update-code

# 销毁所有进程
cis dag down refactor-project
```

## 与之前方案的连续性

1. **复用 Daemon**：每个 Agent 仍是之前的 Unix Socket Daemon，只是由 Scheduler 统一 spawn
2. **复用 Monitor**：卡点检测逻辑不变，但状态变更会触发 DAG 调度（暂停下游）
3. **简化网络**：单机用 `tokio::sync` 代替 Matrix Room 做状态同步，但保留 Matrix 事件上报（联邦记忆）

**关键点：**
- **工作区隔离**：每个 Agent 独立目录，避免文件冲突
- **并发控制**：`max_workers` 防止同时开 10 个 Claude 把内存吃光
- **自动注入**：上游输出自动格式化为 Markdown 注入下游 Prompt，无需手动 copy-paste

这样你得到了一个 **"本地版 Kimi Agent 集群"**：单机并发执行、DAG 依赖管理、卡点自动暂停+人工介入，且能随时 `attach` 进去救场。