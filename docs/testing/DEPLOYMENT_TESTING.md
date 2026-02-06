# CIS DAG 分布式执行 - 部署测试方案

## 概述

本文档描述 CIS DAG 分布式执行系统的部署测试方案，包括环境搭建、测试用例和验收标准。

## 测试环境

### 1. Docker 本地测试环境

```bash
# 构建镜像
docker build -t cis:latest .

# 启动双节点测试环境
docker-compose up -d

# 查看日志
docker-compose logs -f cis-node1
```

### 2. 物理机/虚拟机测试环境

| 节点 | IP | 端口 | 角色 |
|------|-----|------|------|
| Node 1 | 192.168.1.10 | 7676 | 主节点 |
| Node 2 | 192.168.1.11 | 7676 | 从节点 |

## 测试用例

### TC-01: 单机全流程测试

**目标**: 验证单节点上的完整 DAG 执行流程

**前置条件**:
- cis-node 已启动并运行
- 网络连通

**步骤**:

1. **创建 DAG 定义**
   ```bash
   cat > /tmp/test_dag.toml << 'EOF'
   [skill]
   name = "integration-test"
   type = "dag"
   
   [dag]
   policy = "all_success"
   
   [[dag.tasks]]
   id = "task1"
   skill = "echo"
   deps = []
   
   [[dag.tasks]]
   id = "task2"
   skill = "echo"
   deps = ["task1"]
   EOF
   ```

2. **创建运行**
   ```bash
   curl -X POST http://localhost:7676/api/v1/dag/runs \
     -H "Content-Type: application/json" \
     -d @/tmp/test_dag.toml
   ```

3. **查询状态**
   ```bash
   cis-node dag list
   cis-node dag status <run-id>
   ```

**预期结果**:
- DAG 运行创建成功
- 任务按拓扑顺序执行
- 最终状态为 Completed

### TC-02: 作用域推断测试

**目标**: 验证 4 种作用域推断方式

| 场景 | dag_id | env | 预期 scope |
|------|--------|-----|------------|
| 显式指定 | mydag | - | 使用 dag.scope |
| env 推断 | mydag | PROJECT_ID=proj-a | project:proj-a |
| id 推断 | proj-a-mydag | - | project:proj-a |
| 默认 | mydag | - | global |

**验证命令**:
```bash
cis-node dag definitions --scope project
```

### TC-03: Worker 复用测试

**目标**: 验证相同 scope 的 DAG 复用 Worker

**步骤**:

1. 创建第一个 proj-a 的 DAG 运行
2. 创建第二个 proj-a 的 DAG 运行
3. 检查 Worker 列表

**预期结果**:
- 两个运行使用相同的 worker
- Worker 列表中只有 1 个 proj-a worker

### TC-04: 节点认领过滤测试

**目标**: 验证 target_node 过滤逻辑

**场景 1: 指定目标节点**
```toml
[dag]
target_node = "node2"
```
- Node 1: 应忽略
- Node 2: 应认领

**场景 2: 广播模式**
```toml
[dag]
# target_node 不指定
```
- accept_broadcast=true: 认领
- accept_broadcast=false: 忽略

**验证方法**:
```bash
# 在 Node 2 上检查运行列表
curl http://node2:7676/api/v1/dag/runs | jq '.runs | length'
# 预期: >0
```

### TC-05: 网络分区恢复测试

**目标**: 验证网络分区后的行为

**步骤**:

1. 启动 Node 1 和 Node 2
2. 从 Node 1 发送 DAG 到 Room
3. 断开 Node 2 网络 10 秒
4. 恢复 Node 2 网络
5. 检查 Node 2 是否同步到 DAG

**预期结果**:
- Node 2 恢复后应能获取历史消息
- 或消息持久化确保不丢失

### TC-06: TODO List 提案测试

**目标**: 验证 Room Agent → Worker 的 TODO 提案流程

**步骤**:

1. Worker 启动 DAG 运行
2. Room Agent 发送 TODO 提案
3. Worker 审核并合并
4. 验证 TODO List 更新

**Matrix 事件**:
```json
{
  "type": "io.cis.dag.todo_proposal",
  "content": {
    "run_id": "run-xxx",
    "proposal": {
      "source": "RoomAgent",
      "changes": { "added": [...] }
    }
  }
}
```

### TC-07: 并发压力测试

**目标**: 验证高并发下的稳定性

**参数**:
- DAG 数量: 100
- 任务数/DAG: 5-10
- 并发执行: 10

**验证指标**:
- 成功率 > 99%
- 平均执行时间 < 30s
- 无死锁/数据竞争

```bash
# 使用脚本批量提交
for i in $(seq 1 100); do
  curl -X POST http://localhost:7676/api/v1/dag/runs \
    -d @test_dag_$i.toml &
done
wait
```

### TC-08: 故障恢复测试

**目标**: 验证 Worker 故障后的恢复

**场景 1: Worker 进程崩溃**
1. 启动 DAG 运行
2. kill -9 <worker_pid>
3. 检查是否重新启动 Worker
4. 验证 DAG 状态一致性

**场景 2: 节点重启**
1. 运行中的 DAG
2. 重启 cis-node
3. 检查 DAG 恢复
4. 继续执行

## 自动化测试

### 使用集成测试脚本

```bash
# 完整测试
./scripts/integration-test.sh

# 仅测试 Node 1
./scripts/integration-test.sh --node1-only

# 等待服务就绪
./scripts/integration-test.sh --wait
```

### CI/CD 集成

```yaml
# .github/workflows/integration-test.yml
name: Integration Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Build Docker image
        run: docker build -t cis:test .
      
      - name: Start services
        run: docker-compose -f docker-compose.test.yml up -d
      
      - name: Run tests
        run: ./scripts/integration-test.sh
      
      - name: Cleanup
        run: docker-compose down -v
```

## 验收标准

### 功能验收

- [ ] TC-01: 单机全流程通过
- [ ] TC-02: 作用域推断正确
- [ ] TC-03: Worker 复用正常
- [ ] TC-04: 节点认领过滤正确
- [ ] TC-05: 网络分区可恢复
- [ ] TC-06: TODO 提案流程正常
- [ ] TC-07: 并发压力通过
- [ ] TC-08: 故障恢复正常

### 性能验收

- [ ] DAG 提交延迟 < 100ms
- [ ] Worker 启动时间 < 3s
- [ ] 状态查询响应 < 50ms
- [ ] 支持 100+ 并发 DAG

### 稳定性验收

- [ ] 7x24 小时运行无内存泄漏
- [ ] 网络抖动不影响核心功能
- [ ] 数据库连接自动恢复

## 问题排查

### 常见问题

**问题 1: 节点无法加入 Matrix Room**
```bash
# 检查网络连通性
curl http://node1:7676/health

# 检查 Room 配置
cis-node config get matrix.room_id
```

**问题 2: Worker 未复用**
```bash
# 检查 scope 是否一致
cis-node dag definitions --scope project

# 检查 Worker 列表
cis-node dag worker list
```

**问题 3: DAG 状态不一致**
```bash
# 检查数据库
cis-node doctor --check-db

# 强制同步
cis-node dag sync --run-id <id>
```

### 日志收集

```bash
# 收集所有节点日志
docker-compose logs --tail=1000 > test_logs.txt

# 收集特定运行日志
cis-node dag logs <run-id> > run_logs.txt
```

## 附录

### 测试数据生成

```bash
# 生成随机 DAG
./scripts/generate-test-dag.sh --tasks 10 --depth 3

# 批量创建运行
./scripts/bulk-submit.sh --count 100
```

### 监控指标

```bash
# 查看实时指标
curl http://localhost:7676/metrics

# 查看 Worker 状态
curl http://localhost:7676/api/v1/workers
```
