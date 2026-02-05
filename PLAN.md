# CIS-DAG 分布式执行实施计划

## 架构确认

5cloud（入口）→ 推断scope → 推送到目标节点 → DAG Skill → 按作用域启动agent-worker → 执行

## Phase 1: 基础结构搭建（1-2天）

### Task 1.1 扩展DAG类型定义
- 文件: `cis-core/src/skill/manifest.rs` 或 `cis-core/src/scheduler/mod.rs`
- 内容: 添加 `DagScope` 枚举（Global/Project/User/Type）
- 字段: dag_id, tasks, scope, target_node, priority
- 方法: `worker_id()`, `infer_from_dag()`, `parse_scope_from_id()`
- 验收: `cargo check` 通过，类型可序列化

### Task 1.2 扩展SQLite表结构
- 文件: `cis-core/src/scheduler/persistence.rs`
- 表: `dag_definitions` 添加字段
  - scope_type TEXT (global/project/user/type)
  - scope_id TEXT (proj-a/user-123/backup)
  - target_node TEXT
  - priority TEXT
- 验收: 数据库迁移脚本可运行，新旧数据兼容

### Task 1.3 创建DAG Skill框架
- 文件: `skills/dag-executor/src/lib.rs`（新建目录）
- 结构: 
  - struct DagSkill
  - impl Skill trait
  - handle_trigger() 方法框架
- 触发器: HTTP (/api/v1/dag/execute), Room事件 (!dag), CLI
- 验收: Skill可被cis-node加载识别

## Phase 2: 作用域推断逻辑（1天）

### Task 2.1 实现4种推断方式
- 文件: `cis-core/src/skill/dag_scope.rs`（新建）
- 功能:
  - 显式指定: 直接使用 `dag.scope`
  - env推断: 扫描 `tasks[*].env.PROJECT_ID`
  - dag_id推断: 正则 `proj-(\w+)-`
  - 默认: Global
- 测试: 单元测试覆盖4种场景
- 验收: `cargo test` 通过

### Task 2.2 作用域冲突检测
- 场景: 同一worker_id，不同target_node
- 处理: 报错或警告，避免调度混乱
- 验收: 异常输入有明确错误信息

## Phase 3: Worker管理（2天）

### Task 3.1 Worker进程启动
- 文件: `skills/dag-executor/src/worker.rs`
- 功能:
  - spawn_worker(worker_id, scope) → Child
  - 文件锁 `/tmp/cis-worker-{id}.lock`
  - 孤儿进程检测（定期检测父PID）
- 命令: `cis worker run --id worker-project-proj-a --scope project:proj-a`
- 验收: 可启动独立进程，文件锁生效

### Task 3.2 Worker池管理
- 文件: `skills/dag-executor/src/pool.rs`
- 结构: HashMap<worker_id, WorkerHandle>
- 功能:
  - get_or_create_worker() → 复用或新建
  - cleanup_dead_workers() → 清理僵尸
  - max_workers限制（LRU淘汰）
- 验收: 同scope的DAG发送到同一worker，不同scope不同worker

### Task 3.3 Worker间通信
- 方式: SQLite队列 或 Unix Socket
- 选择: SQLite简单可靠
- 表: `worker_queue` (worker_id, dag_json, status, created_at)
- 验收: DAG可入队，worker可读取

## Phase 4: 5cloud转发逻辑（1天）

### Task 4.1 HTTP直接推送
- 文件: `cis-core/src/glm/mod.rs`（扩展）
- 端点: POST /api/v1/dag/execute
- 逻辑:
  - 接收DAG
  - 推断scope
  - 如果target_node指定，HTTP POST到目标节点
  - 如果未指定，调用Room广播
- 验收: curl测试，目标节点能接收

### Task 4.2 Room广播
- 文件: `cis-core/src/matrix/events/dag.rs`（新建）
- 事件: `io.cis.dag.execute`
- 内容: { dag_id, tasks, scope, target_node, timestamp }
- 广播: `room.send(event)`
- 验收: Matrix Room可见事件

### Task 4.3 节点认领过滤
- 文件: `skills/dag-executor/src/lib.rs`
- 逻辑:
  - 接收Room事件
  - 检查 `target_node == self.node_id`
  - 匹配 → 执行
  - 不匹配 → 忽略
- 验收: 只有目标节点执行，其他节点忽略

## Phase 5: 执行引擎（2天）

### Task 5.1 Worker执行循环
- 文件: `skills/dag-executor/src/worker_main.rs`
- 流程:
  - loop: 查询SQLite队列
  - 取出DAG
  - 拓扑排序
  - 逐个执行任务
  - 更新状态
- 任务执行: shell命令或skill调用
- 验收: 可执行简单shell任务

### Task 5.2 任务状态机
- 状态: Pending → Running → Completed/Failed
- 乐观锁: version字段
- 更新: `UPDATE dag_runs SET status=?, version=version+1 WHERE id=? AND version=?`
- 验收: 并发更新测试通过

### Task 5.3 失败重试
- 配置: max_retries, retry_delay
- 逻辑: 失败时根据retry策略重新入队
- 验收: 失败任务自动重试指定次数

## Phase 6: CLI查询（1天）

### Task 6.1 dag list命令
- 文件: `cis-node/src/commands/dag.rs`
- 功能: 列出dag_definitions表
- 过滤: --status, --scope, --node
- 输出: 表格格式（dag_id, scope, status, owner_node, created）
- 验收: 可直接读SQLite显示

### Task 6.2 dag status命令
- 功能: `cis dag status <dag-id>`
- 内容: 显示DAG详情 + 最近的run记录 + 当前任务
- 验收: 信息完整可读

### Task 6.3 dag logs命令
- 功能: `cis dag logs <dag-id> [--run <run-id>]`
- 内容: 显示task_executions表的output/error
- 验收: 可查看执行日志

### Task 6.4 dag worker命令
- 功能: 
  - `cis dag worker list` → 显示运行中的workers
  - `cis dag worker status <worker-id>` → worker详情
- 验收: 可监控worker状态

## Phase 7: 集成测试（1天）

### Task 7.1 单机全流程测试
- 步骤:
  1. `cis glm start` 启动API
  2. curl推送DAG（无target_node，scope=global）
  3. 验证worker启动
  4. 验证任务执行
  5. `cis dag status` 查询
- 验收: 端到端流程打通

### Task 7.2 作用域隔离测试
- 步骤:
  1. 推送proj-a的DAG
  2. 推送proj-b的DAG
  3. 验证启动2个worker
  4. 验证各自执行不干扰
- 验收: 不同scope的DAG并行执行

### Task 7.3 模拟集群测试
- 环境: 2个cis-node实例（不同端口）
- 步骤:
  1. node-1启动，指定target_node=node-1
  2. node-2启动，不指定target_node
  3. 从node-2推送DAG到Room
  4. 验证只有node-1执行
- 验收: 分布式认领逻辑正确

## 依赖与风险

### 外部依赖
- Matrix Room（集群模式必需）
- SQLite（已存在）
- tokio进程管理

### 技术风险
| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| Worker进程僵尸 | 中 | 资源泄漏 | 父PID检测 + 租约超时 |
| SQLite并发锁 | 低 | 性能瓶颈 | 连接池 + 短事务 |
| 作用域推断错误 | 中 | DAG分到错误worker | 显式指定优先级最高 |
| 网络分区 | 低 | 任务丢失 | 消息持久化 + 重试 |

## 验收标准

### 功能验收
- [ ] 5cloud HTTP推送DAG成功
- [ ] Room广播DAG被目标节点认领
- [ ] 相同scope的DAG复用worker
- [ ] 不同scope的DAG启动不同worker
- [ ] CLI可查询DAG状态、日志、worker
- [ ] 任务失败可重试

### 性能验收
- [ ] 单机100个DAG/分钟处理能力
- [ ] Worker启动时间 < 3秒
- [ ] 查询响应时间 < 100ms

## 时间线

| Phase | 任务数 | 预估时间 | 累计 |
|-------|--------|----------|------|
| 1 基础结构 | 3 | 1-2天 | 2天 |
| 2 作用域推断 | 2 | 1天 | 3天 |
| 3 Worker管理 | 3 | 2天 | 5天 |
| 4 5cloud转发 | 3 | 1天 | 6天 |
| 5 执行引擎 | 3 | 2天 | 8天 |
| 6 CLI查询 | 4 | 1天 | 9天 |
| 7 集成测试 | 3 | 1天 | 10天 |

**总计: 10天（保守估计，实际可能7-8天）**

## 下一步行动

立即开始: Task 1.1（扩展DAG类型定义）

检查点: 
- Day 3: Phase 1-2完成，作用域推断可用
- Day 6: Phase 3-4完成，5cloud推送可用
- Day 9: Phase 5-6完成，CLI查询可用
- Day 10: 全量测试通过
