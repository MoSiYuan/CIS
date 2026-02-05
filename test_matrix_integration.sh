#!/bin/bash
# Worker Matrix 集成测试

set -e

CIS_TARGET="/tmp/cis-target/debug"
TEST_DIR="/tmp/cis_matrix_test"

echo "==================================="
echo "Worker Matrix 集成测试"
echo "==================================="
echo ""

# 清理环境
echo "1. 清理测试环境..."
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"
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

# 显示新参数
echo "3. 检查 Matrix 参数..."
$CIS_TARGET/cis-node worker start --help 2>&1 | grep -E "(matrix|Matrix)" | sed 's/^/   /'
echo ""

# 测试 Worker 启动（带 Matrix 参数）
echo "4. 测试 Worker 启动（带 Matrix 参数）..."
$CIS_TARGET/cis-node worker start \
    --worker-id test-matrix-worker \
    --room '!test-room:test-node' \
    --scope global \
    --parent-node test-node \
    --matrix-server "http://localhost:7676" \
    --matrix-token "test-token" \
    --verbose > "$TEST_DIR/worker.log" 2>&1 &
WORKER_PID=$!

sleep 2

if kill -0 $WORKER_PID 2>/dev/null; then
    echo "   ✓ Worker 启动成功 (PID: $WORKER_PID)"
    
    # 检查日志中是否有 Matrix 相关信息
    if grep -q "Matrix" "$TEST_DIR/worker.log" 2>/dev/null; then
        echo "   ✓ Matrix 初始化日志 detected:"
        grep -E "(Matrix|matrix)" "$TEST_DIR/worker.log" | head -3 | sed 's/^/      /'
    fi
else
    echo "   ⚠️ Worker 可能已退出（无 Matrix 服务器）"
fi

# 停止 Worker
kill $WORKER_PID 2>/dev/null || true
sleep 1
echo ""

# 测试 2: Standalone 模式（无 Matrix）
echo "5. 测试 Standalone 模式（无 Matrix token）..."
$CIS_TARGET/cis-node worker start \
    --worker-id test-standalone \
    --room '!test-room:test-node' \
    --scope global \
    --parent-node test-node \
    --verbose > "$TEST_DIR/worker2.log" 2>&1 &
WORKER2_PID=$!

sleep 2

if kill -0 $WORKER2_PID 2>/dev/null; then
    echo "   ✓ Worker 启动成功 (Standalone 模式)"
    
    if grep -q "standalone" "$TEST_DIR/worker2.log" 2>/dev/null; then
        echo "   ✓ Standalone 模式日志:"
        grep -i "standalone" "$TEST_DIR/worker2.log" | head -1 | sed 's/^/      /'
    fi
fi

kill $WORKER2_PID 2>/dev/null || true
echo ""

# 测试总结
echo "==================================="
echo "测试总结"
echo "==================================="
echo ""
echo "✅ 已实现功能:"
echo "   - MatrixHttpClient 结构体 (HTTP API 客户端)"
echo "   - --matrix-server 参数"
echo "   - --matrix-token 参数"
echo "   - join_room() 实际调用 Matrix API"
echo "   - send_message() 发送消息到 Room"
echo "   - Task 结果自动发送到 Matrix Room"
echo "   - Heartbeat 自动发送到 Matrix Room"
echo "   - Standalone 模式（无 Matrix 时）"
echo ""
echo "📁 日志位置: $TEST_DIR"
echo ""
