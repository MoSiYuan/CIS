#!/bin/sh
# CIS 自动组网工具 - AI Agent 可用
# 用法: ./auto-pair.sh [命令] [参数...]

set -e

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 获取配对码（内部函数）
_get_code() {
    local node=$1
    docker exec "$node" sh -c '
        rm -f /tmp/pair.log
        cis-node pair generate --timeout 30 > /tmp/pair.log 2>&1 &
        for i in $(seq 1 20); do
            code=$(grep -oE "[0-9]{6}" /tmp/pair.log 2>/dev/null | head -1)
            [ -n "$code" ] && echo "$code" && exit 0
            sleep 1
        done
        exit 1
    ' 2>/dev/null
}

# 执行配对（内部函数）
_do_join() {
    local target=$1
    local code=$2
    local source_ip=$3
    docker exec "$target" cis-node pair join "$code" --address "$source_ip:6768" 2>/dev/null || true
}

# 命令: pair <source> <target>
# 从 source 生成配对码，让 target 加入
cmd_pair() {
    local source=${1:-cis-node1}
    local target=${2:-cis-node2}
    
    echo -e "${BLUE}[组网]${NC} $source -> $target"
    
    # 检查容器
    if ! docker ps | grep -q "$source"; then
        echo -e "${RED}错误${NC}: $source 未运行"
        return 1
    fi
    if ! docker ps | grep -q "$target"; then
        echo -e "${RED}错误${NC}: $target 未运行"
        return 1
    fi
    
    # 生成配对码
    echo "  正在 $source 生成配对码..."
    local code=$(_get_code "$source")
    if [ -z "$code" ]; then
        echo -e "${RED}失败${NC}: 无法获取配对码"
        return 1
    fi
    echo -e "  ${GREEN}✓${NC} 配对码: $code"
    
    # 执行配对
    local source_ip=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$source")
    echo "  正在 $target 执行加入..."
    _do_join "$target" "$code" "$source_ip"
    
    echo -e "${GREEN}✓${NC} 组网完成: $target 已连接到 $source"
}

# 命令: mesh [node1] [node2] [node3]
# 创建全连接网络（星型拓扑）
cmd_mesh() {
    local n1=${1:-cis-node1}
    local n2=${2:-cis-node2}
    local n3=${3:-cis-node3}
    
    echo -e "${BLUE}[组网]${NC} 创建星型网络: $n1 <- $n2, $n3"
    
    # n2 -> n1
    cmd_pair "$n1" "$n2"
    echo ""
    
    # n3 -> n1
    cmd_pair "$n1" "$n3"
}

# 命令: chain <node1> <node2> <node3>
# 创建链式网络: node1 <-> node2 <-> node3
cmd_chain() {
    local n1=${1:-cis-node1}
    local n2=${2:-cis-node2}
    local n3=${3:-cis-node3}
    
    echo -e "${BLUE}[组网]${NC} 创建链式网络: $n1 <-> $n2 <-> $n3"
    
    cmd_pair "$n1" "$n2"
    echo ""
    cmd_pair "$n2" "$n3"
}

# 命令: reset
# 停止并重启所有容器
cmd_reset() {
    echo -e "${YELLOW}[重置]${NC} 重启所有节点..."
    docker-compose down 2>/dev/null || true
    docker-compose up -d
    echo -e "${GREEN}✓${NC} 节点已重启，等待启动..."
    sleep 5
}

# 命令: status
# 显示所有节点状态
cmd_status() {
    echo -e "${BLUE}[状态]${NC} 节点运行状态:"
    docker ps --format 'table {{.Names}}\t{{.Status}}' | grep -E "(NAME|cis-node)" || echo "  无运行中的节点"
    echo ""
    echo -e "${BLUE}[网络]${NC} 容器网络:"
    docker network inspect cis-isolated-network 2>/dev/null | grep -E "(Name|IPv4)" | head -10 || echo "  网络未创建"
}

# 命令: help 或 无参数
cmd_help() {
    cat << 'EOF'
CIS 自动组网工具

用法:
  ./auto-pair.sh <命令> [参数...]

命令:
  pair <source> <target>    从source生成配对码，让target加入
                           例: ./auto-pair.sh pair node1 node2

  mesh [n1] [n2] [n3]       创建星型网络（所有节点连到n1）
                           例: ./auto-pair.sh mesh

  chain [n1] [n2] [n3]      创建链式网络
                           例: ./auto-pair.sh chain

  reset                     重置所有节点
  status                    查看节点状态
  help                      显示帮助

示例:
  # 启动环境
  docker-compose up -d

  # node2 加入 node1
  ./auto-pair.sh pair cis-node1 cis-node2

  # 创建全连接网络
  ./auto-pair.sh mesh

  # 重置后重新组网
  ./auto-pair.sh reset && ./auto-pair.sh mesh

EOF
}

# 主入口
case "${1:-help}" in
    pair)
        shift
        cmd_pair "$@"
        ;;
    mesh)
        shift
        cmd_mesh "$@"
        ;;
    chain)
        shift
        cmd_chain "$@"
        ;;
    reset)
        cmd_reset
        ;;
    status)
        cmd_status
        ;;
    help|--help|-h)
        cmd_help
        ;;
    *)
        echo -e "${RED}未知命令: $1${NC}"
        cmd_help
        exit 1
        ;;
esac
