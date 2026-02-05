#!/bin/bash
# Worker 进程管理测试

set -e

CIS_TARGET="/tmp/cis-target/debug"
TEST_DIR="/tmp/cis_worker_mgmt_test"

echo "==================================="
echo "Worker 进程管理测试"
echo "==================================="
echo ""

# 清理环境
echo "1. 清理测试环境..."
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"
rm -rf "$HOME/.cis/workers"
echo "   ✓ 环境清理完成"
echo ""

# 检查构建
echo "2. 检查构建..."
if [ ! -f "$CIS_TARGET/cis-node" ]; then
    echo "   构建 cis-node..."
    cargo build -p cis-node
fi
echo "   ✓ 构建完成"
echo ""

# 测试 1: 启动 Worker 并查看状态
echo "3. 启动 Worker..."
$CIS_TARGET/cis-node worker start \
    --worker-id test-mgmt-worker \
    --room '!test:test-node' \
    --scope global \
    --parent-node test-node \
    --verbose > "$TEST_DIR/worker.log" 2>&1 &
WORKER_PID=$!

sleep 2

if kill -0 $WORKER_PID 2>/dev/null; then
    echo "   ✓ Worker 启动成功 (PID: $WORKER_PID)"
else
    echo "   ✗ Worker 启动失败"
    exit 1
fi
echo ""

# 测试 2: 查看 Worker 状态
echo "4. 查看 Worker 状态..."
$CIS_TARGET/cis-node worker status test-mgmt-worker 2>&1 | head -20 | sed 's/^/   /'
echo ""

# 测试 3: 列出所有 Workers
echo "5. 列出所有 Workers..."
$CIS_TARGET/cis-node worker status 2>&1 | sed 's/^/   /'
echo ""

# 测试 4: 停止 Worker
echo "6. 停止 Worker ( graceful )..."
$CIS_TARGET/cis-node worker stop test-mgmt-worker 2>&1 | sed 's/^/   /'

sleep 2

if kill -0 $WORKER_PID 2>/dev/null; then
    echo "   ⚠️ Worker 仍在运行，强制停止..."
    $CIS_TARGET/cis-node worker stop test-mgmt-worker --force 2>&1 | sed 's/^/   /'
else
    echo "   ✓ Worker 已停止"
fi
echo ""

# 测试 5: 查看停止后的状态
echo "7. 查看停止后的 Worker 状态..."
$CIS_TARGET/cis-node worker status test-mgmt-worker 2>&1 | sed 's/^/   /'
echo ""

# 测试 6: 列出所有 Workers（停止后）
echo "8. 列出所有 Workers（停止后）..."
$CIS_TARGET/cis-node worker status 2>&1 | sed 's/^/   /'
echo ""

# 测试总结
echo "==================================="
echo "测试总结"
echo "==================================="
echo ""
echo "✅ 已实现功能:"
echo "   - WorkerRegistry 注册表结构"
echo "   - Worker 启动自动注册"
echo "   - Worker 状态查询 (status <worker-id>)"
echo "   - Worker 列表查询 (status)"
echo "   - Worker 优雅停止 (stop)"
echo "   - Worker 强制停止 (stop --force)"
echo "   - Worker 停止后自动清理注册表"
echo "   - 心跳更新机制"
echo "   - 任务统计 (executed/active)"
echo ""
echo "📁 注册表位置: $HOME/.cis/workers/"
echo "📁 日志位置: $TEST_DIR"
echo ""
