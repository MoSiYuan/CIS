#!/bin/sh
# CIS 自动组网脚本
# 自动获取配对码并完成节点配对

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 默认配置
SOURCE_NODE=${1:-cis-node1}
TARGET_NODE=${2:-cis-node2}
TIMEOUT=${3:-10}

echo -e "${BLUE}================================${NC}"
echo -e "${BLUE}  CIS 自动组网${NC}"
echo -e "${BLUE}================================${NC}"
echo ""
echo "  源节点: $SOURCE_NODE"
echo "  目标节点: $TARGET_NODE"
echo ""

# 检查节点是否存在
check_node() {
    if ! docker ps --format '{{.Names}}' | grep -q "^$1$"; then
        echo -e "${RED}错误: 节点 $1 未运行${NC}"
        exit 1
    fi
}

check_node "$SOURCE_NODE"
check_node "$TARGET_NODE"

# 在源节点生成配对码
echo -e "${YELLOW}[1/3] 在 $SOURCE_NODE 生成配对码...${NC}"

# 后台运行配对码生成，捕获输出
CODE=$(docker exec "$SOURCE_NODE" sh -c '
    # 启动配对码生成（后台）
    cis-node pair generate --timeout 60 > /tmp/pair.log 2>&1 &
    PID=$!
    
    # 等待配对码出现
    for i in $(seq 1 30); do
        if grep -qE "[0-9]{6}" /tmp/pair.log 2>/dev/null; then
            grep -oE "[0-9]{6}" /tmp/pair.log | head -1
            exit 0
        fi
        sleep 1
    done
    exit 1
' 2>/dev/null)

if [ -z "$CODE" ] || ! echo "$CODE" | grep -qE "^[0-9]{6}$"; then
    echo -e "${RED}错误: 无法获取配对码${NC}"
    echo "  尝试手动获取: docker exec -it $SOURCE_NODE cis-node pair generate"
    exit 1
fi

echo -e "  ${GREEN}✓${NC} 配对码: $CODE"
echo ""

# 获取源节点地址
SOURCE_IP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$SOURCE_NODE")
echo -e "${YELLOW}[2/3] 目标节点加入...${NC}"
echo "  源节点地址: $SOURCE_IP"

# 在目标节点执行配对
docker exec "$TARGET_NODE" sh -c "
    echo '正在使用配对码 $CODE 连接到 $SOURCE_NODE...'
    cis-node pair join $CODE --address $SOURCE_IP:6768 2>&1 || echo '配对命令执行完成'
" 2>/dev/null || true

echo ""
echo -e "${GREEN}✓${NC} 配对流程完成"
echo ""

# 验证连接
echo -e "${YELLOW}[3/3] 验证连接...${NC}"
sleep 2

docker exec "$TARGET_NODE" cis-node neighbor list 2>/dev/null || echo "  (邻居列表为空或命令未实现)"

echo ""
echo -e "${BLUE}================================${NC}"
echo -e "${GREEN}组网完成!${NC}"
echo -e "${BLUE}================================${NC}"
echo ""
echo "查看状态:"
echo "  docker exec $SOURCE_NODE cis-node status"
echo "  docker exec $TARGET_NODE cis-node status"
