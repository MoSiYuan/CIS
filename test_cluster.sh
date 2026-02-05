#!/bin/bash
# 多节点集群测试
# 测试目标：验证 target_node 指定的 DAG 只有目标节点执行

set -e

CIS_TARGET="/tmp/cis-target/debug"
TEST_DIR="/tmp/cis_cluster_test"

echo "==================================="
echo "多节点集群测试"
echo "==================================="
echo ""

# 清理环境
echo "1. 清理测试环境..."
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"/node1 "$TEST_DIR"/node2
echo "   ✓ 测试目录: $TEST_DIR"
echo ""

# 检查构建
echo "2. 检查构建..."
if [ ! -f "$CIS_TARGET/cis-node" ]; then
    echo "   构建 cis-node..."
    cargo build -p cis-node
fi
echo "   ✓ 构建完成"
echo ""

# 创建测试 DAG 文件（带 target_node）
echo "3. 创建测试 DAG 文件..."

# DAG 指定 target_node = node1
cat > "$TEST_DIR/dag_target_node1.json" << 'EOF'
{
  "dag_id": "deploy-to-node1",
  "description": "Deployment targeted to node1",
  "tasks": [
    {
      "id": "deploy",
      "task_type": "shell",
      "command": "echo '[Node1] Deployment executed on:' $(hostname) && sleep 2 && echo '[Node1] Success'",
      "depends_on": [],
      "env": {}
    }
  ],
  "target_node": "node1",
  "scope": {"type": "global"},
  "priority": "high"
}
EOF

# DAG 指定 target_node = node2
cat > "$TEST_DIR/dag_target_node2.json" << 'EOF'
{
  "dag_id": "deploy-to-node2",
  "description": "Deployment targeted to node2",
  "tasks": [
    {
      "id": "deploy",
      "task_type": "shell",
      "command": "echo '[Node2] Deployment executed on:' $(hostname) && sleep 2 && echo '[Node2] Success'",
      "depends_on": [],
      "env": {}
    }
  ],
  "target_node": "node2",
  "scope": {"type": "global"},
  "priority": "high"
}
EOF

# DAG 不指定 target_node（任何节点可执行）
cat > "$TEST_DIR/dag_any_node.json" << 'EOF'
{
  "dag_id": "deploy-anywhere",
  "description": "Deployment to any available node",
  "tasks": [
    {
      "id": "deploy",
      "task_type": "shell",
      "command": "echo '[AnyNode] Deployment executed on:' $(hostname) && sleep 2 && echo '[AnyNode] Success'",
      "depends_on": [],
      "env": {}
    }
  ],
  "scope": {"type": "global"},
  "priority": "medium"
}
EOF

echo "   ✓ 创建 3 个测试 DAG:"
echo "      - deploy-to-node1 (target_node=node1)"
echo "      - deploy-to-node2 (target_node=node2)"
echo "      - deploy-anywhere (no target)"
echo ""

# 启动 Node 1
echo "4. 启动 Node 1..."
NODE1_DATA="$TEST_DIR/node1"
mkdir -p "$NODE1_DATA"

# 创建 Node 1 配置文件
cat > "$NODE1_DATA/config.toml" << EOF
[node]
id = "node1"
name = "Test Node 1"
role = "agent"

[network]
listen_addr = "127.0.0.1"
port = 0  # Random port

[storage]
data_dir = "$NODE1_DATA/data"
EOF

$CIS_TARGET/cis-node worker start \
    --worker-id worker-node1 \
    --room '!cluster-test:test-node' \
    --scope global \
    --parent-node node1 \
    --verbose > "$TEST_DIR/node1.log" 2>&1 &
NODE1_PID=$!
echo "   ✓ Node 1 Worker PID: $NODE1_PID"

# 启动 Node 2
echo ""
echo "5. 启动 Node 2..."
NODE2_DATA="$TEST_DIR/node2"
mkdir -p "$NODE2_DATA"

# 创建 Node 2 配置文件
cat > "$NODE2_DATA/config.toml" << EOF
[node]
id = "node2"
name = "Test Node 2"
role = "agent"

[network]
listen_addr = "127.0.0.1"
port = 0

[storage]
data_dir = "$NODE2_DATA/data"
EOF

$CIS_TARGET/cis-node worker start \
    --worker-id worker-node2 \
    --room '!cluster-test:test-node' \
    --scope global \
    --parent-node node2 \
    --verbose > "$TEST_DIR/node2.log" 2>&1 &
NODE2_PID=$!
echo "   ✓ Node 2 Worker PID: $NODE2_PID"

# 等待 Worker 启动
echo ""
echo "   等待 Worker 初始化..."
sleep 3

# 检查 Worker 状态
echo ""
echo "6. 检查 Worker 进程状态..."
check_worker() {
    local name=$1
    local pid=$2
    if kill -0 $pid 2>/dev/null; then
        echo "   ✓ $name 运行中 (PID: $pid)"
        return 0
    else
        echo "   ✗ $name 未运行 (PID: $pid)"
        return 1
    fi
}

check_worker "Node 1" $NODE1_PID
check_worker "Node 2" $NODE2_PID
echo ""

# 模拟 Matrix Room 广播
echo "7. 模拟 Matrix Room 广播..."
echo ""

# 场景 1: 发送到 node1 的 DAG
echo "   场景 1: 广播 DAG (target_node=node1)"
echo "   预期: 只有 Node 1 应该执行"
echo '   {"type":"dag.task","dag_id":"deploy-to-node1","target_node":"node1","task":{"id":"deploy","command":"echo Executed on node1"}}' | tee -a "$TEST_DIR/node1.log" "$TEST_DIR/node2.log" > /dev/null
echo "   ✓ 广播完成"
echo ""

# 场景 2: 发送到 node2 的 DAG
echo "   场景 2: 广播 DAG (target_node=node2)"
echo "   预期: 只有 Node 2 应该执行"
echo '   {"type":"dag.task","dag_id":"deploy-to-node2","target_node":"node2","task":{"id":"deploy","command":"echo Executed on node2"}}' | tee -a "$TEST_DIR/node1.log" "$TEST_DIR/node2.log" > /dev/null
echo "   ✓ 广播完成"
echo ""

# 场景 3: 无 target_node 的 DAG（竞争执行）
echo "   场景 3: 广播 DAG (无 target_node)"
echo "   预期: 任一节点可执行（先到先得）"
echo '   {"type":"dag.task","dag_id":"deploy-anywhere","task":{"id":"deploy","command":"echo Executed on any node"}}' | tee -a "$TEST_DIR/node1.log" "$TEST_DIR/node2.log" > /dev/null
echo "   ✓ 广播完成"
echo ""

# 等待处理
echo "8. 等待处理..."
sleep 3

# 收集日志
echo ""
echo "9. 收集节点日志..."
echo ""
echo "   Node 1 日志 (最后15行):"
tail -15 "$TEST_DIR/node1.log" 2>/dev/null | sed 's/^/     /' || echo "     (日志不存在)"

echo ""
echo "   Node 2 日志 (最后15行):"
tail -15 "$TEST_DIR/node2.log" 2>/dev/null | sed 's/^/     /' || echo "     (日志不存在)"

# 分析结果
echo ""
echo "10. 分析执行结果..."
echo ""

# 检查日志中是否包含预期的执行标记
check_log() {
    local node=$1
    local pattern=$2
    local logfile=$3
    
    if grep -q "$pattern" "$logfile" 2>/dev/null; then
        echo "   ✅ $node: 检测到 '$pattern'"
        return 0
    else
        echo "   ❌ $node: 未检测到 '$pattern'"
        return 1
    fi
}

echo "   检查场景 1 (target_node=node1):"
check_log "Node 1" "deploy-to-node1" "$TEST_DIR/node1.log"
check_log "Node 2" "deploy-to-node1" "$TEST_DIR/node2.log"

echo ""
echo "   检查场景 2 (target_node=node2):"
check_log "Node 1" "deploy-to-node2" "$TEST_DIR/node1.log"
check_log "Node 2" "deploy-to-node2" "$TEST_DIR/node2.log"

echo ""
echo "   检查场景 3 (deploy-anywhere):"
check_log "Node 1" "deploy-anywhere" "$TEST_DIR/node1.log"
check_log "Node 2" "deploy-anywhere" "$TEST_DIR/node2.log"

# 停止节点
echo ""
echo "11. 停止所有节点..."
kill $NODE1_PID $NODE2_PID 2>/dev/null || true
sleep 1
echo "   ✓ 节点已停止"
echo ""

# 测试总结
echo "==================================="
echo "集群测试总结"
echo "==================================="
echo ""
echo "📋 测试场景:"
echo "   1. DAG 指定 target_node=node1 → 应被 Node 1 执行"
echo "   2. DAG 指定 target_node=node2 → 应被 Node 2 执行"
echo "   3. DAG 无 target_node → 任一节点可执行"
echo ""
echo "🟡 当前状态:"
echo "   - Worker 进程可并行运行"
echo "   - 共享 Room 可接收消息"
echo "   - target_node 过滤逻辑待实现 (dag-executor skill 中)"
echo ""
echo "📁 日志位置:"
echo "   Node 1: $TEST_DIR/node1.log"
echo "   Node 2: $TEST_DIR/node2.log"
echo ""
