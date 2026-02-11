#!/bin/bash
# CIS 网络测试脚本
# 在 Docker 环境中运行网络相关测试

set -e

cd "$(dirname "$0")"

echo "======================================"
echo "CIS Network Test Runner"
echo "======================================"

# 检查 Docker 是否运行
if ! docker info > /dev/null 2>&1; then
    echo "Error: Docker is not running"
    exit 1
fi

# 构建镜像
echo ""
echo "[1/4] Building Docker images..."
docker-compose -f docker-compose.test-network.yml build

# 启动 3 节点组网
echo ""
echo "[2/4] Starting 3-node CIS network..."
docker-compose -f docker-compose.test-network.yml up -d node1 node2 node3

# 等待节点启动
echo ""
echo "[3/4] Waiting for nodes to start (10s)..."
sleep 10

# 显示节点状态
echo ""
echo "Node status:"
docker-compose -f docker-compose.test-network.yml ps

# 运行网络测试
echo ""
echo "[4/4] Running network tests..."
docker-compose -f docker-compose.test-network.yml --profile test run --rm test-runner

# 测试完成
echo ""
echo "======================================"
echo "Network tests completed!"
echo "======================================"
echo ""
echo "To view logs:"
echo "  docker-compose -f docker-compose.test-network.yml logs node1"
echo ""
echo "To stop network:"
echo "  docker-compose -f docker-compose.test-network.yml down"
echo ""
echo "To run interactive shell:"
echo "  docker-compose -f docker-compose.test-network.yml exec node1 sh"
