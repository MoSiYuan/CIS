# CIS 单机多节点模拟指南

在没有网络环境或多台机器的情况下，如何在单机上模拟多节点任务分发？

## 问题背景

真实的跨平台编译场景需要:
- 电脑A (Mac) - 协调器
- 电脑B (Linux + CUDA) - CUDA编译  
- 电脑C (Mac Studio) - Metal编译

但单机环境下:
- 端口冲突 (7676/8080)
- 无法真正分配任务到不同机器
- Matrix Room 需要网络

## 解决方案

### 方案1: Embedded 模式 (推荐用于开发测试)

使用 `dag execute` 命令在单进程内模拟执行:

```bash
# 1. 创建 DAG 运行
cargo run -p cis-node -- dag run examples/cross_compile_pipeline.toml

# 2. 使用 embedded 模式执行 (不依赖 Matrix)
cargo run -p cis-node -- dag execute --use-agent --max-workers 4
```

**特点**:
- ✅ 无需网络
- ✅ 单进程内模拟
- ✅ 支持 Agent 执行
- ⚠️ 不真正分发到多节点

### 方案2: 多数据目录模拟 (推荐用于集成测试)

使用不同的 `CIS_DATA_DIR` 环境变量启动多个实例:

```bash
# 终端1: 节点A (协调器)
export CIS_DATA_DIR=/tmp/cis-node-a
export CIS_NODE_ID=node-a
cargo run -p cis-node -- glm start &

# 终端2: 节点B (CUDA Worker)  
export CIS_DATA_DIR=/tmp/cis-node-b
export CIS_NODE_ID=node-b
cargo run -p cis-node -- worker run \
    --worker-id cuda-worker \
    --room "!test:localhost" \
    --parent-node node-b

# 终端3: 节点C (Metal Worker)
export CIS_DATA_DIR=/tmp/cis-node-c
export CIS_NODE_ID=node-c
cargo run -p cis-node -- worker run \
    --worker-id metal-worker \
    --room "!test:localhost" \
    --parent-node node-c
```

**特点**:
- ✅ 真正的多进程隔离
- ✅ 独立的数据存储
- ✅ 可测试 Worker 通信
- ⚠️ 仍需要 Matrix Room

### 方案3: Mock 模式 (推荐用于单元测试)

修改 DAG 文件，使用模拟技能:

```toml
[[dag.tasks]]
id = "cuda-compile"
name = "CUDA模块编译"
skill = "mock-compile"  # 模拟编译
level = { type = "mechanical" }
deps = ["check-branch"]
target_node = "node-b"

[[dag.tasks]]
id = "metal-compile"
name = "Metal模块编译"  
skill = "mock-compile"
level = { type = "mechanical" }
deps = ["check-branch"]
target_node = "node-c"
```

创建模拟技能:

```bash
# 创建 mock 技能配置
mkdir -p ~/.cis/skills/mock-compile
cat > ~/.cis/skills/mock-compile/skill.toml << 'EOF'
[skill]
name = "mock-compile"
version = "1.0.0"
type = "builtin"
description = "模拟编译任务"

[executor]
type = "shell"
command = "echo"
args = ["Mock compile for", "{{target_node}}"]
EOF
```

**特点**:
- ✅ 无需外部依赖
- ✅ 快速验证流程
- ⚠️ 不执行真实编译

### 方案4: Docker Compose (推荐用于完整测试)

使用 Docker 容器隔离:

```bash
# 启动多节点集群
docker compose up -d

# 查看日志
docker compose logs -f

# 在容器内执行 DAG
docker compose exec cis-node1 cis-node dag run \
    /app/config/cross_compile_pipeline.toml
```

**特点**:
- ✅ 完全隔离的环境
- ✅ 真实的网络通信
- ✅ 可测试完整的任务分发
- ⚠️ 需要 Docker

## 实际演示

### 单机模拟多节点执行

```bash
# 1. 准备环境
./scripts/multi-node-simulator.sh run

# 2. 创建测试 DAG
cat > /tmp/test-dag.toml << 'EOF'
[skill]
name = "single-node-test"
type = "dag"

[dag]
policy = "all_success"

[[dag.tasks]]
id = "prepare"
name = "准备"
skill = "echo"
level = { type = "mechanical" }

[[dag.tasks]]
id = "compile-a"
name = "编译A"
skill = "echo"
level = { type = "mechanical" }
deps = ["prepare"]
target_node = "node-a"

[[dag.tasks]]
id = "compile-b"
name = "编译B"
skill = "echo"
level = { type = "mechanical" }
deps = ["prepare"]
target_node = "node-b"

[[dag.tasks]]
id = "merge"
name = "合并"
skill = "echo"
level = { type = "mechanical" }
deps = ["compile-a", "compile-b"]
EOF

# 3. 运行测试
cargo run -p cis-node -- dag run /tmp/test-dag.toml
cargo run -p cis-node -- dag execute --use-agent
```

### 验证节点过滤

检查任务是否正确分配到不同节点:

```bash
# 查看 DAG 状态
cargo run -p cis-node -- dag status

# 预期输出:
# - prepare:     completed (无 target_node, 任意节点执行)
# - compile-a:   completed (target_node: node-a)
# - compile-b:   completed (target_node: node-b)  
# - merge:       completed (等待前两个完成)
```

## 节点选择机制

### 单机模拟时的行为

1. **无 target_node**: 任务可在任意节点执行
2. **有 target_node**: 调度器会尝试匹配
   - 如果本地节点ID匹配，本地执行
   - 如果不匹配，尝试转发或排队等待

### 强制本地执行

```bash
# 设置当前节点ID
export CIS_NODE_ID=node-b

# 此时只有 target_node=node-b 的任务会执行
# 其他任务会等待或跳过
```

## 推荐工作流

### 开发阶段
```bash
# 使用 embedded 模式快速迭代
cargo run -p cis-node -- dag run my-dag.toml
cargo run -p cis-node -- dag execute --use-agent
```

### 集成测试阶段
```bash
# 使用多数据目录测试
tmux new-session -d -s cis-test

tmux split-window -h
tmux send-keys "CIS_DATA_DIR=/tmp/node-a CIS_NODE_ID=node-a cargo run -p cis-node -- glm start" C-m

tmux split-window -v  
tmux send-keys "CIS_DATA_DIR=/tmp/node-b CIS_NODE_ID=node-b cargo run -p cis-node -- worker run --worker-id w1" C-m

tmux attach
```

### 生产验证阶段
```bash
# 使用 Docker Compose
docker compose up -d
docker compose logs -f cis-node1
```

## 常见问题

### Q: 单机模拟时任务都跑到一个节点怎么办？
A: 这是预期行为。单机模式下所有任务都在同一进程执行。要测试真正的分发，需要使用 Docker 或多台机器。

### Q: 如何验证 target_node 过滤逻辑？
A: 查看日志输出，搜索 `"target_node":` 和任务分配记录。

### Q: 可以绕过 target_node 强制本地执行吗？
A: 可以，使用 `dag execute` 的 embedded 模式会忽略 target_node 限制。

## 总结

| 方案 | 复杂度 | 真实度 | 适用场景 |
|------|--------|--------|----------|
| Embedded | 低 | 低 | 开发迭代 |
| 多数据目录 | 中 | 中 | 集成测试 |
| Mock 模式 | 低 | 低 | 单元测试 |
| Docker | 高 | 高 | 完整验证 |

单机环境下推荐组合使用：**Embedded 模式** 用于快速开发，**Docker Compose** 用于最终验证。
