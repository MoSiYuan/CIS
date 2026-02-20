# TASK 7.4: DAG 任务编排

> **Phase**: 7 - 多 Agent 架构 (P3 可选)
> **状态**: 🔄 进行中 (占位符已创建)
> **负责人**: TBD
> **周期**: Week 15

---

## 任务概述

实现复杂的 DAG（有向无环图）任务编排，支持 Agent 协作。

## 工作内容

### 1. DAG 核心结构

```rust
// crates/cis-scheduler/src/dag.rs
#[cfg(feature = "dag")]
pub struct DagScheduler {
    graph: TaskGraph,
    executor: Arc<dyn Executor>,
    results: HashMap<TaskId, TaskResult>,
}

#[cfg(feature = "dag")]
pub struct TaskGraph {
    nodes: HashMap<TaskId, TaskNode>,
    edges: Vec<(TaskId, TaskId)>,  // from -> to
}

#[cfg(feature = "dag")]
impl TaskGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }
    
    pub fn add_task(&mut self, task: DagTask) -> Result<&mut Self, DagError> {
        let id = task.id.clone();
        self.nodes.insert(id, TaskNode::new(task));
        Ok(self)
    }
    
    pub fn add_dependency(&mut self, from: TaskId, to: TaskId) -> Result<&mut Self, DagError> {
        // 检查循环依赖
        if self.would_create_cycle(&from, &to) {
            return Err(DagError::CyclicDependency);
        }
        self.edges.push((from, to));
        Ok(self)
    }
    
    pub fn topological_sort(&self) -> Result<Vec<TaskId>, DagError> {
        // Kahn 算法
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        let mut adj_list: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
        
        // 初始化
        for id in self.nodes.keys() {
            in_degree.insert(id.clone(), 0);
            adj_list.insert(id.clone(), Vec::new());
        }
        
        // 构建邻接表和入度
        for (from, to) in &self.edges {
            adj_list.get_mut(from).unwrap().push(to.clone());
            *in_degree.get_mut(to).unwrap() += 1;
        }
        
        // 拓扑排序
        let mut queue: VecDeque<TaskId> = in_degree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| id.clone())
            .collect();
        
        let mut result = Vec::new();
        while let Some(id) = queue.pop_front() {
            result.push(id.clone());
            for neighbor in &adj_list[&id] {
                let degree = in_degree.get_mut(neighbor).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(neighbor.clone());
                }
            }
        }
        
        if result.len() != self.nodes.len() {
            return Err(DagError::CyclicDependency);
        }
        
        Ok(result)
    }
}
```

### 2. DAG 任务定义

```rust
// crates/cis-scheduler/src/dag.rs
#[cfg(feature = "dag")]
#[derive(Debug, Clone)]
pub struct DagTask {
    pub id: TaskId,
    pub agent: String,           // Agent 名称或类型
    pub level: TaskLevel,        // 决策层级
    pub input_mapping: HashMap<String, String>,  // 输入映射
    pub output_mapping: HashMap<String, String>, // 输出映射
}

#[cfg(feature = "dag")]
impl DagTask {
    pub fn builder(id: impl Into<TaskId>) -> DagTaskBuilder {
        DagTaskBuilder::new(id)
    }
}

#[cfg(feature = "dag")]
pub struct DagTaskBuilder {
    id: TaskId,
    agent: Option<String>,
    level: TaskLevel,
    input_mapping: HashMap<String, String>,
    output_mapping: HashMap<String, String>,
}

#[cfg(feature = "dag")]
impl DagTaskBuilder {
    pub fn agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }
    
    pub fn mechanical(mut self, retry: u32) -> Self {
        self.level = TaskLevel::Mechanical { retry };
        self
    }
    
    pub fn arbitrated(mut self, stakeholders: Vec<String>) -> Self {
        self.level = TaskLevel::Arbitrated { stakeholders };
        self
    }
    
    pub fn build(self) -> Result<DagTask, DagError> {
        Ok(DagTask {
            id: self.id,
            agent: self.agent.ok_or(DagError::MissingAgent)?,
            level: self.level,
            input_mapping: self.input_mapping,
            output_mapping: self.output_mapping,
        })
    }
}
```

### 3. DAG 执行引擎

```rust
// crates/cis-scheduler/src/dag_executor.rs
#[cfg(feature = "dag")]
pub struct DagExecutor {
    agent_pool: Arc<AgentPool>,
}

#[cfg(feature = "dag")]
impl DagExecutor {
    pub async fn execute(&self, dag: &TaskGraph) -> Result<DagResult, ExecutionError> {
        let order = dag.topological_sort()?;
        let mut results: HashMap<TaskId, TaskResult> = HashMap::new();
        
        for task_id in order {
            let node = dag.get_node(&task_id).unwrap();
            
            // 等待依赖完成
            self.wait_for_dependencies(&node.dependencies, &results).await?;
            
            // 获取 Agent
            let mut agent = self.agent_pool
                .acquire(AgentType::from(&node.task.agent))
                .await?;
            
            // 准备输入（映射依赖的输出）
            let input = self.prepare_input(&node.task, &results)?;
            
            // 执行任务
            let result = match node.task.level {
                TaskLevel::Mechanical { retry } => {
                    self.execute_with_retry(&mut *agent, &input, retry).await
                }
                TaskLevel::Arbitrated { ref stakeholders } => {
                    self.execute_with_arbitration(&mut *agent, &input, stakeholders).await
                }
                _ => agent.turn(&input).await.map_err(Into::into),
            };
            
            results.insert(task_id, result?);
            
            // 归还 Agent
            self.agent_pool.release(agent).await;
        }
        
        Ok(DagResult { results })
    }
    
    async fn execute_with_retry(
        &self,
        agent: &mut dyn Agent,
        input: &str,
        max_retry: u32,
    ) -> Result<TaskResult, ExecutionError> {
        for attempt in 0..=max_retry {
            match agent.turn(input).await {
                Ok(output) => return Ok(TaskResult::Success(output)),
                Err(e) if attempt < max_retry => {
                    warn!("Attempt {} failed: {}, retrying...", attempt + 1, e);
                    sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
                Err(e) => return Err(ExecutionError::AgentError(e)),
            }
        }
        unreachable!()
    }
}
```

### 4. 使用示例

```rust
// 创建一个复杂的工作流
let mut dag = TaskGraph::new();

dag.add_task(
    DagTask::builder("analyze_code")
        .agent("debugger")
        .mechanical(3)
        .build()?
)?
.add_task(
    DagTask::builder("fix_code")
        .agent("coder")
        .arbitrated(vec!["senior_dev".into()])
        .build()?
)?
.add_task(
    DagTask::builder("update_docs")
        .agent("doc")
        .mechanical(2)
        .build()?
)?
.add_dependency("analyze_code", "fix_code")?
.add_dependency("fix_code", "update_docs")?;

// 执行 DAG
let executor = DagExecutor::new(agent_pool);
let result = executor.execute(&dag).await?;
```

## 验收标准

- [ ] DAG 结构支持复杂依赖
- [ ] 拓扑排序正确
- [ ] 循环依赖检测
- [ ] 多级决策支持
- [ ] Agent 池集成
- [ ] 性能可接受（1000节点 < 1分钟）

## 依赖

- Task 7.2 (Receptionist)
- Task 7.3 (Worker Agents)
- Task 2.3 (Scheduler)

## 阻塞

- Task 7.5 (P2P 跨设备)

---
