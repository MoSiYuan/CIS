# TASK 7.2: Receptionist Agent 实现

> **Phase**: 7 - 多 Agent 架构 (P3 可选)
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: 497a3d4
> **负责人**: TBD
> **周期**: Week 14

---

## 任务概述

实现 Receptionist Agent，作为 IM 网关和任务分类/路由中心。

## 工作内容

### 1. Receptionist Agent 设计

```rust
// crates/cis-core/src/agent/receptionist.rs
#[cfg(feature = "receptionist")]
pub struct ReceptionistAgent {
    name: String,
    im_adapter: Box<dyn IMAdapter>,
    classifier: Box<dyn QueryClassifier>,
    agent_pool: Arc<AgentPool>,
    memory: Arc<dyn Memory>,
    decision_engine: DecisionEngine,
}

#[cfg(feature = "receptionist")]
impl ReceptionistAgent {
    pub fn builder() -> ReceptionistAgentBuilder {
        ReceptionistAgentBuilder::new()
    }
    
    /// 处理 IM 消息入口
    pub async fn handle_message(&mut self, msg: IMMessage) -> Result<(), ReceptionistError> {
        // 1. 提取上下文
        let context = self.extract_context(&msg).await?;
        
        // 2. 分类任务
        let task_type = self.classifier.classify(&msg.content).await?;
        
        // 3. 确定决策层级
        let level = self.determine_decision_level(&task_type, &context);
        
        // 4. 路由到合适的 Agent
        match level {
            TaskLevel::Mechanical { retry } => {
                self.route_mechanical(task_type, msg, retry).await
            }
            TaskLevel::Recommended => {
                self.route_recommended(task_type, msg).await
            }
            TaskLevel::Confirmed { timeout } => {
                self.route_confirmed(task_type, msg, timeout).await
            }
            TaskLevel::Arbitrated { stakeholders } => {
                self.route_arbitrated(task_type, msg, stakeholders).await
            }
        }
    }
    
    async fn route_mechanical(
        &self,
        task_type: TaskType,
        msg: IMMessage,
        retry: u32,
    ) -> Result<(), ReceptionistError> {
        // 获取 Worker Agent
        let mut agent = self.agent_pool
            .acquire(AgentType::from(&task_type))
            .await?;
        
        // 执行并自动重试
        let mut last_error = None;
        for attempt in 0..=retry {
            match agent.turn(&msg.content).await {
                Ok(response) => {
                    // 发送回复
                    self.im_adapter.send(&msg.sender, &response).await?;
                    self.agent_pool.release(agent).await;
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < retry {
                        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                    }
                }
            }
        }
        
        Err(ReceptionistError::ExecutionFailed(last_error.unwrap()))
    }
}
```

### 2. 实现 Query Classifier

```rust
// crates/cis-core/src/agent/classifier.rs
#[cfg(feature = "receptionist")]
pub trait QueryClassifier: Send + Sync {
    async fn classify(&self, query: &str) -> Result<TaskType, ClassificationError>;
    async fn classify_with_confidence(&self, query: &str) -> Result<(TaskType, f32), ClassificationError>;
}

#[cfg(feature = "receptionist")]
pub struct LLMQueryClassifier {
    model: Box<dyn LLM>,
    categories: Vec<TaskCategory>,
}

#[cfg(feature = "receptionist")]
impl QueryClassifier for LLMQueryClassifier {
    async fn classify(&self, query: &str) -> Result<TaskType, ClassificationError> {
        let prompt = format!(
            "Classify the following query into one of: {:?}\n\nQuery: {}",
            self.categories, query
        );
        
        let response = self.model.complete(&prompt).await?;
        TaskType::from_str(&response.trim())
            .map_err(|_| ClassificationError::UnknownCategory(response))
    }
}
```

### 3. 实现四层级决策引擎

```rust
// crates/cis-core/src/agent/decision.rs
#[derive(Debug, Clone)]
pub enum TaskLevel {
    /// 机械层: 自动化执行，失败自动重试
    Mechanical { retry: u32 },
    /// 推荐层: 给出建议，等待用户确认
    Recommended,
    /// 确认层: 需要超时确认
    Confirmed { timeout: Duration },
    /// 仲裁层: 需要多方确认
    Arbitrated { stakeholders: Vec<String> },
}

pub struct DecisionEngine {
    rules: Vec<DecisionRule>,
}

impl DecisionEngine {
    pub fn evaluate(&self, task_type: &TaskType, context: &Context) -> TaskLevel {
        for rule in &self.rules {
            if rule.matches(task_type, context) {
                return rule.level.clone();
            }
        }
        // 默认推荐层
        TaskLevel::Recommended
    }
}
```

## 验收标准

- [ ] Receptionist Agent 可处理 IM 消息
- [ ] Query Classifier 准确分类任务
- [ ] 四层级决策机制工作正常
- [ ] 可路由到不同 Worker Agent
- [ ] 集成测试通过

## 依赖

- Task 7.1 (Agent trait)

## 阻塞

- Task 7.4 (DAG 编排)

---
