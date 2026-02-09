#!/bin/sh
# CIS Agent 组网脚本 - 极简版本
# AI Agent 直接调用，无需交互
#
# 用法:
#   ./agent-pair.sh node1 node2    # 让 node2 加入 node1
#   ./agent-pair.sh mesh           # 创建全网状网络

set -e

N1=${1:-cis-node1}
N2=${2:-cis-node2}
N3=${3:-cis-node3}

# 获取容器IP
_get_ip() {
    docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$1" 2>/dev/null
}

# 在容器内生成配对码并返回
_gen_code() {
    local node=$1
    docker exec "$node" cis-node pair generate 2>&1 | grep -oE '[0-9]{6}' | head -1
}

# 两个节点配对
_pair_nodes() {
    local source=$1
    local target=$2
    
    echo "组网: $target -> $source"
    
    # 生成配对码
    local code=$(_gen_code "$source")
    [ -z "$code" ] && echo "错误: 无法生成配对码" && return 1
    
    echo "  配对码: $code"
    
    # 获取源节点IP
    local ip=$(_get_ip "$source")
    
    # 目标节点加入
    docker exec "$target" cis-node pair join "$code" --address "$ip:6768" 2>/dev/null || true
    
    echo "  完成"
}

# mesh 全网状网络
_mesh() {
    echo "创建星型网络 (所有节点连到 $N1)..."
    _pair_nodes "$N1" "$N2"
    _pair_nodes "$N1" "$N3"
    echo "星型网络创建完成"
}

# chain 链式网络
_chain() {
    echo "创建链式网络 ($N1 <-> $N2 <-> $N3)..."
    _pair_nodes "$N1" "$N2"
    _pair_nodes "$N2" "$N3"
    echo "链式网络创建完成"
}

# 主逻辑
case "$1" in
    mesh)
        _mesh
        ;;
    chain)
        _chain
        ;;
    status)
        docker ps --format 'table {{.Names}}\t{{.Status}}' | grep -E "(NAME|cis-node)" || echo "无运行节点"
        ;;
    *)
        # 默认: 配对两个节点
        if [ $# -ge 2 ]; then
            _pair_nodes "$1" "$2"
        else
            echo "用法:"
            echo "  $0 node1 node2     # 让 node2 加入 node1"
            echo "  $0 mesh            # 创建星型网络"
            echo "  $0 chain           # 创建链式网络"
            echo "  $0 status          # 查看节点状态"
            exit 1
        fi
        ;;
esac
