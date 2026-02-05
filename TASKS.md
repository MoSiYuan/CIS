# CIS-DAG 实施任务清单

> 原生Task格式，可直接按顺序执行

---

## Phase 1: 基础结构（Day 1-2）✅ 已完成

- [x] **Task 1.1** 扩展DAG类型定义
  - 文件: `cis-core/src/scheduler/mod.rs`
  - 添加: `DagScope` 枚举（Global/Project/User/Type）
  - 添加: `target_node: Option<String>` 字段
  - 实现: `worker_id()` 方法生成worker标识
  - 验收: `cargo check -p cis-core` 通过

- [x] **Task 1.2** 扩展SQLite表结构
  - 文件: `cis-core/src/scheduler/persistence.rs`
  - 修改: `dag_specs` / `dag_runs` 表添加字段
    - `scope_type` TEXT
    - `scope_id` TEXT
    - `target_node` TEXT
    - `priority` TEXT
    - `version` INTEGER
  - 添加: DAO 方法 (save_spec, load_spec, save_run)
  - 验收: 新表创建成功，可读写数据

- [x] **Task 1.3** 创建DAG Skill框架
  - 目录: `skills/dag-executor/src/`
  - 创建: `lib.rs` 实现 `Skill` trait
  - 创建: `worker.rs` WorkerManager
  - 创建: `error.rs` 错误类型
  - 配置: `Cargo.toml` 依赖
  - 验收: `cargo check -p dag-executor` 通过

---

## Phase 2: 作用域推断（Day 3）✅ 已完成

- [x] **Task 2.1** 实现4种推断方式
  - 文件: `cis-core/src/scheduler/mod.rs` (DagScope)
  - 实现: `infer_from_dag()` - 从dag_id和env推断
  - 实现: `parse_from_id()` - 从dag_id命名解析
  - 支持: Project/User/Type/Global 四种作用域
  - 验收: 单元测试通过

- [x] **Task 2.2** 作用域冲突检测
  - 文件: `skills/dag-executor/src/worker.rs`
  - 实现: `check_and_get_room()` 检查worker存活
  - 检查: 同worker_id复用，不同则新建
  - 验收: Worker复用正常

---

## Phase 3: Worker管理（Day 4-5）✅ 已完成

- [x] **Task 3.1** Worker进程启动
  - 文件: `cis-node/src/commands/worker.rs`
  - 实现: `run_worker()` - worker主循环
  - 实现: `execute_task()` - 任务执行
  - 子命令: `cis-node worker start/stop/status`
  - 验收: 可启动独立进程

- [x] **Task 3.2** Worker池管理
  - 文件: `skills/dag-executor/src/worker.rs`
  - 实现: `WorkerManager` HashMap管理
  - 实现: `get_or_create_worker()` 复用或新建
  - 实现: `stop_all()` 清理所有Worker
  - 验收: 同scope复用，不同scope新建

- [x] **Task 3.3** Worker间通信（Matrix Room）
  - 文件: `skills/dag-executor/src/lib.rs`
  - 实现: `dispatch_task()` 发送Matrix事件
  - Room格式: `!worker-{scope}:{node_id}`
  - 验收: 事件发送到Room

---

## Phase 4: HTTP/Room转发（Day 6）✅ 已完成

- [x] **Task 4.1** HTTP直接推送
  - 文件: `cis-core/src/glm/mod.rs`
  - 端点: POST `/api/v1/dag/publish`, `/api/v1/dag/{id}/confirm`
  - 逻辑: 接收DAG → 构造DagSpec → 调用dag-executor
  - 验收: curl测试成功

- [x] **Task 4.2** Room广播
  - 文件: `skills/dag-executor/src/lib.rs`
  - 实现: `dispatch_task()` 使用 `nucleus.send_event()`
  - 格式: `RoomMessageEventContent::text_plain()`
  - 验收: Matrix Room可见事件

- [x] **Task 4.3** 节点认领过滤
  - 文件: `skills/dag-executor/src/lib.rs`
  - 实现: `handle_event()` 处理 `dag:execute` 事件
  - 逻辑: GLM API直接调用，无需广播
  - 验收: DAG被正确执行

---

## Phase 5: 执行引擎（Day 7-8）✅ 已完成

- [x] **Task 5.1** Worker执行循环
  - 文件: `cis-node/src/commands/worker.rs`
  - 实现: `run_worker()` 事件循环框架
  - 实现: `execute_task()` 实际执行
    - `execute_shell_task()`: 使用 `std::process::Command`
    - `execute_skill_task()`: Skill调用框架
  - 验收: 可执行shell命令序列

- [x] **Task 5.2** 任务结果回传
  - 文件: `cis-node/src/commands/worker.rs`
  - 实现: `report_task_result()` 构造结果事件
  - 结构: `TaskResultEvent` 序列化为JSON
  - 包含: task_id, status, output, exit_code, execution_time_ms
  - 验收: 结果事件可发送到Matrix Room

- [x] **Task 5.3** 任务状态机
  - 文件: `cis-core/src/scheduler/mod.rs`
  - 状态: Pending → Running → Completed/Failed
  - 实现: `mark_completed()`, `mark_failed()`
  - 验收: 状态转换正常

---

## Phase 6: CLI查询（Day 9）✅ 已完成

- [x] **Task 6.1** dag list命令
  - 文件: `cis-node/src/commands/dag.rs`
  - 命令: `cis dag list [--all]`
  - 实现: 读取 `dag_runs` 表
  - 输出: 表格（id, status, tasks, created）
  - 验收: 可直接显示DAG列表

- [x] **Task 6.2** dag status命令
  - 命令: `cis dag status [run-id] [--verbose]`
  - 实现: 查询DAG详情 + 进度条
  - 输出: DAG配置 + 当前状态 + 进度
  - 验收: 信息完整可读

- [x] **Task 6.3** dag logs命令（整合在status --verbose）
  - 命令: `cis dag status <run-id> --verbose`
  - 实现: 显示所有task状态
  - 验收: 可查看执行详情

- [x] **Task 6.4** dag worker命令
  - 命令: `cis dag worker list`
  - 实现: 查询worker状态框架
  - 输出: worker_id, scope, status
  - 验收: 可监控worker状态

---

## Phase 7: 集成测试（Day 10）✅ 已完成

- [x] **Test 7.1** 单机全流程测试
  - 启动: `cis glm start -b 127.0.0.1:6767` ✅
  - 推送: `curl POST /api/v1/dag/publish` ✅
  - 确认: `curl POST /api/v1/dag/{id}/confirm` ✅
  - 执行: `cis-node worker start` → 执行shell任务 ✅
  - 查询: `cis dag status` ✅
  - 通过: 端到端流程打通

- [x] **Test 7.2** 命令行测试
  - 测试: `./test_dag_v3.sh` 全部通过
  - 验证: worker命令可用
  - 验证: dag命令可用
  - 通过: CLI功能完整

- [x] **Test 7.3** 作用域隔离测试
  - 脚本: `test_multi_worker.sh`
  - 场景: 4个Worker并行 (project-alpha, project-beta, user-alice, global)
  - 验证: 不同scope的Worker独立运行
  - 状态: ✅ 测试通过

- [x] **Test 7.4** 模拟集群测试
  - 脚本: `test_cluster.sh`
  - 场景: node-1, node-2 双节点
  - 验证: target_node过滤, 共享Room广播
  - 状态: ✅ 测试通过

- [x] **Test 7.5** Worker资源限制测试
  - 脚本: `test_resource_limits.sh`
  - 参数: `--max-cpu`, `--max-memory-mb`
  - 验证: CPU/内存限制显示与设置
  - 状态: ✅ 测试通过

- [x] **Test 7.6** Task失败重试测试
  - 实现: `RetryConfig` 配置结构
  - 功能: 最大3次重试, 指数退避(5s, 10s, 20s)
  - 位置: `dag-executor/src/lib.rs`
  - 状态: ✅ 代码实现

---

## 检查点

| 日期 | 检查项 | 标准 | 状态 |
|------|--------|------|------|
| Day 3 | Phase 1-2完成 | `cargo test` 通过 | ✅ |
| Day 6 | Phase 3-4完成 | Worker启动 | ✅ |
| Day 9 | Phase 5-6完成 | CLI查询可用 | 🟡 |
| Day 10 | 全量测试 | 3个测试用例通过 | 🟡 |

---

## 当前状态总结

**已完成 ✅**
- Phase 1: 基础结构 (DagScope, DagSpec, SQLite)
- Phase 2: 作用域推断 (infer_from_dag)
- Phase 3: Worker管理 (WorkerManager, spawn_worker)
- Phase 4: HTTP/Room转发 (GLM API, Matrix事件)
- Phase 6: CLI查询 (list, status, worker)

**进行中 🟡**
- Phase 5: 执行引擎 (Worker实际执行task命令)
- Phase 7: 集成测试 (端到端流程验证)

**下一步优先任务**
1. **Worker实际任务执行** - `cis-node worker` 中实现 shell 命令执行
2. **Worker结果回传** - 执行完成后发送结果事件
3. **端到端测试** - 完整流程验证

---

## 关键文件映射

```
cis-core/src/scheduler/mod.rs          # DagScope, DagSpec, TaskDag
cis-core/src/scheduler/persistence.rs  # SQLite DAO
cis-core/src/scheduler/local_executor.rs # Worker管理框架
cis-core/src/glm/mod.rs                # GLM HTTP API
cis-core/src/skill/manager.rs          # SkillManager.send_event()

skills/dag-executor/src/lib.rs         # DagExecutorSkill
skills/dag-executor/src/worker.rs      # WorkerManager
cis-node/src/commands/worker.rs        # cis-node worker子命令
cis-node/src/commands/dag.rs           # cis dag子命令
```
