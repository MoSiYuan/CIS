# CIS-DAG 架构审视

## 核心问题：是否影响 CIS 原子性？

### CIS 设计哲学
```
┌─────────────────────────────────────────┐
│           CIS 架构分层                   │
├─────────────────────────────────────────┤
│  Layer 3: Skills (可插拔)                │
│           - dag-executor                │
│           - backup-skill                │
│           - deploy-skill                │
├─────────────────────────────────────────┤
│  Layer 2: Core Services (基础服务)       │
│           - 网络 (matrix/websocket)      │
│           - 存储 (sqlite/memory)         │
│           - 身份 (did/auth)              │
│           - 任务队列 (task/types)        │
├─────────────────────────────────────────┤
│  Layer 1: Kernel (最小内核)              │
│           - 节点生命周期                 │
│           - 事件总线                     │
│           - 配置管理                     │
└─────────────────────────────────────────┘
```

### 当前方案分析

#### ✅ 合理之处
1. **DagScope/DagSpec 放在 cis-core**
   - DAG 已成为 GLM API 的一等公民（publish_dag/query_dag/analyze_dag）
   - 类似于 Task/Action 类型，是基础数据结构
   - 多个 skills 可能需要消费 DagSpec

2. **SQLite 扩展在 persistence**
   - dag_specs/dag_runs 是 core 层需要追踪的状态
   - 与 task 存储在同一层，符合现有模式

#### ⚠️ 潜在问题

**问题1：DagSpec 包含太多执行细节？**
```rust
// 当前 DagSpec 包含执行细节
pub struct DagSpec {
    pub target_node: Option<String>,  // 执行相关
    pub scope: DagScope,              // 执行相关
    pub priority: TaskPriority,       // 执行相关
    // ...
}
```

**问题2：DagScope 是否属于"调度策略"？**
- DagScope 决定 worker 隔离策略
- 这属于"如何执行"而非"是什么"

**问题3：GLM API 与 Skill 的边界**
```
当前流程：
GLM API -> 解析 DagSpec -> 存 SQLite -> 发 Matrix Event 
-> dag-executor skill 接收 -> 创建 Worker

问题：GLM API 需要理解 DagSpec 所有字段，但部分字段是执行细节
```

### 替代方案对比

#### 方案A：当前方案（保留）
```
core: DagScope, DagSpec, SQLite schema
skill: worker 管理, 实际执行

glm: 直接操作 DagSpec
```

#### 方案B：分离"定义"与"执行策略"
```rust
// core: 纯 DAG 定义（类似 Dockerfile）
pub struct DagDefinition {
    pub dag_id: String,
    pub tasks: Vec<DagTaskDef>,
    pub schedule: Option<String>,
    // 不含 target_node, scope, priority
}

// skill: 执行策略（类似 docker run 参数）
pub struct DagExecutionPolicy {
    pub target_node: Option<String>,
    pub scope: DagScope,
    pub priority: TaskPriority,
}

// GLM API 接收组合
pub struct PublishDagRequest {
    pub definition: DagDefinition,
    pub policy: DagExecutionPolicy,  // 可选，默认从定义推断
}
```

#### 方案C：GLM 透传，Skill 解析
```rust
// core: GLM 不解析 DAG，直接透传 JSON
glm_api::publish_dag(raw_json: Value) -> Event {
    // 只验证签名和基本格式
    // 将 raw_json 放入 Matrix Event
}

// skill: dag-executor 解析并执行
impl Skill for DagExecutor {
    fn handle(event: Event) {
        let spec: DagSpec = parse(event.content);
        // 创建 worker, 执行...
    }
}
```

### 推荐决策

**保持当前方案（方案A）**，理由：

1. **GLM API 需要理解 DAG**
   - analyze_dag 需要解析依赖图
   - query_dag 需要返回结构化信息
   - 完全透传无法提供这些功能

2. **DagScope 已成为"基础设施"**
   - 类似 TaskPriority，是系统级调度概念
   - 不仅 dag-executor 使用，CLI/API 都需要

3. **执行细节分离的代价**
   - 方案B 增加复杂度，用户体验下降
   - 需要两处配置，容易出错

4. **与现有模式一致**
   - Task 类型也在 core，但执行在 skill
   - DagSpec 类比 Task，遵循相同模式

### 改进建议（可选）

如果后续发现 DagSpec 过于膨胀，可以：

```rust
// 将执行相关字段标记为 #[serde(skip)] 或分离
pub struct DagSpec {
    // 定义部分（必需）
    pub dag_id: String,
    pub tasks: Vec<DagTaskSpec>,
    
    // 执行策略部分（可选，默认推断）
    #[serde(default)]
    pub execution: DagExecutionConfig,
}

#[serde(default)]
pub struct DagExecutionConfig {
    pub target_node: Option<String>,
    pub scope: DagScope,
    pub priority: TaskPriority,
}
```

### 结论

**当前方案不影响 CIS 原子性**：
- ✅ DagScope/DagSpec 是基础数据类型（类似 Task）
- ✅ 执行逻辑完全在 skill，core 只存状态
- ✅ GLM API 作为网关，理解请求格式是合理职责
- ✅ 与现有架构模式一致

**继续执行 Task 1.3：LocalExecutor 改造**
