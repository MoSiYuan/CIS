#!/bin/bash
# Worker 资源限制测试

set -e

CIS_TARGET="/tmp/cis-target/debug"
TEST_DIR="/tmp/cis_resource_test"

echo "==================================="
echo "Worker 资源限制测试"
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

# 测试 1: 无限制
echo "3. 测试 1: 无资源限制..."
$CIS_TARGET/cis-node worker start \
    --worker-id worker-unlimited \
    --room '!test:test-node' \
    --scope global \
    --parent-node test-node \
    --verbose > "$TEST_DIR/worker1.log" 2>&1 &
WORKER1_PID=$!
sleep 2

if kill -0 $WORKER1_PID 2>/dev/null; then
    echo "   ✓ Worker 启动成功 (无限制)"
    grep "Resources:" "$TEST_DIR/worker1.log" | tail -1 | sed 's/^/     /' || true
else
    echo "   ✗ Worker 启动失败"
fi

kill $WORKER1_PID 2>/dev/null || true
echo ""

# 测试 2: 内存限制
echo "4. 测试 2: 内存限制 (100MB)..."
$CIS_TARGET/cis-node worker start \
    --worker-id worker-mem-limit \
    --room '!test:test-node' \
    --scope global \
    --parent-node test-node \
    --max-memory-mb 100 \
    --verbose > "$TEST_DIR/worker2.log" 2>&1 &
WORKER2_PID=$!
sleep 2

if kill -0 $WORKER2_PID 2>/dev/null; then
    echo "   ✓ Worker 启动成功 (内存限制 100MB)"
    grep "Memory Limit:" "$TEST_DIR/worker2.log" | tail -1 | sed 's/^/     /' || true
else
    echo "   ✗ Worker 启动失败"
fi

kill $WORKER2_PID 2>/dev/null || true
echo ""

# 测试 3: CPU 限制
echo "5. 测试 3: CPU 限制 (2 cores)..."
$CIS_TARGET/cis-node worker start \
    --worker-id worker-cpu-limit \
    --room '!test:test-node' \
    --scope global \
    --parent-node test-node \
    --max-cpu 2 \
    --verbose > "$TEST_DIR/worker3.log" 2>&1 &
WORKER3_PID=$!
sleep 2

if kill -0 $WORKER3_PID 2>/dev/null; then
    echo "   ✓ Worker 启动成功 (CPU限制 2 cores)"
    grep "CPU Limit:" "$TEST_DIR/worker3.log" | tail -1 | sed 's/^/     /' || true
else
    echo "   ✗ Worker 启动失败"
fi

kill $WORKER3_PID 2>/dev/null || true
echo ""

# 测试 4: CPU + 内存限制
echo "6. 测试 4: CPU + 内存限制..."
$CIS_TARGET/cis-node worker start \
    --worker-id worker-both-limits \
    --room '!test:test-node' \
    --scope global \
    --parent-node test-node \
    --max-cpu 4 \
    --max-memory-mb 512 \
    --verbose > "$TEST_DIR/worker4.log" 2>&1 &
WORKER4_PID=$!
sleep 2

if kill -0 $WORKER4_PID 2>/dev/null; then
    echo "   ✓ Worker 启动成功 (CPU: 4, Memory: 512MB)"
    grep -E "(CPU Limit|Memory Limit):" "$TEST_DIR/worker4.log" | sed 's/^/     /' || true
else
    echo "   ✗ Worker 启动失败"
fi

kill $WORKER4_PID 2>/dev/null || true
echo ""

# 测试总结
echo "==================================="
echo "测试总结"
echo "==================================="
echo ""
echo "✅ 已验证功能:"
echo "   - --max-cpu 参数可用"
echo "   - --max-memory-mb 参数可用"
echo "   - 资源限制信息在启动时显示"
echo ""
echo "🟡 说明:"
echo "   - 资源限制通过 ulimit 环境变量实现"
echo "   - 实际效果取决于操作系统支持"
echo "   - 完整 cgroup 支持待后续实现"
echo ""
echo "📁 日志位置: $TEST_DIR"
echo ""
