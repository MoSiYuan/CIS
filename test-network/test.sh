#!/bin/sh
# CIS Docker 组网测试脚本

set -e

cd "$(dirname "$0")"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}================================${NC}"
echo -e "${BLUE}  CIS Docker 组网测试${NC}"
echo -e "${BLUE}================================${NC}"
echo

# 检查 Docker
check_docker() {
    if ! command -v docker >/dev/null 2>&1; then
        echo -e "${RED}错误: Docker 未安装${NC}"
        exit 1
    fi
    if ! command -v docker-compose >/dev/null 2>&1; then
        echo -e "${RED}错误: docker-compose 未安装${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓${NC} Docker 环境正常"
}

# 检查二进制文件
check_binary() {
    if [ -f "./cis-node" ]; then
        # 检查是否是 Linux 二进制
        if file ./cis-node | grep -q "Linux"; then
            echo -e "${GREEN}✓${NC} 找到 Linux 二进制文件"
            return 0
        else
            echo -e "${YELLOW}!${NC} 现有二进制不是 Linux 版本 ($(file ./cis-node | awk '{print $NF}'))"
            return 1
        fi
    fi
    return 1
}

# 清理环境
cleanup() {
    echo -e "${YELLOW}清理现有环境...${NC}"
    docker-compose down 2>/dev/null || true
    docker-compose -f docker-compose.build.yml down 2>/dev/null || true
}

# 使用预编译二进制启动
start_prebuilt() {
    echo -e "${YELLOW}使用预编译二进制启动...${NC}"
    docker-compose up -d --build
}

# 从源码构建启动
start_build() {
    echo -e "${YELLOW}从源码构建 (这可能需要几分钟)...${NC}"
    docker-compose -f docker-compose.build.yml up -d --build
}

# 等待节点就绪
wait_nodes() {
    echo
    echo -e "${YELLOW}等待节点启动...${NC}"
    for i in $(seq 1 30); do
        if docker ps | grep -q "cis-node1" && docker ps | grep -q "cis-node2"; then
            echo -e "${GREEN}✓${NC} 节点已启动"
            return 0
        fi
        sleep 2
        echo -n "."
    done
    echo -e "${RED}超时${NC}"
    return 1
}

# 运行组网测试
run_test() {
    echo
    echo -e "${BLUE}================================${NC}"
    echo -e "${BLUE}  运行组网测试${NC}"
    echo -e "${BLUE}================================${NC}"
    echo
    
    # 检查 agent-pair.sh 是否存在
    if [ ! -f "./agent-pair.sh" ]; then
        echo -e "${RED}错误: agent-pair.sh 不存在${NC}"
        return 1
    fi
    
    # 等待服务完全启动
    echo "等待 CIS 服务就绪..."
    sleep 5
    
    # 运行组网
    echo -e "${YELLOW}执行星型组网...${NC}"
    ./agent-pair.sh mesh
    
    echo
    echo -e "${GREEN}✓${NC} 组网测试完成"
}

# 显示状态
show_status() {
    echo
    echo -e "${BLUE}================================${NC}"
    echo -e "${BLUE}  节点状态${NC}"
    echo -e "${BLUE}================================${NC}"
    echo
    docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Networks}}' | grep -E "(NAME|cis-)" || echo "  无运行节点"
    echo
    echo "日志查看:"
    echo "  docker-compose logs -f node1"
    echo
    echo "进入容器:"
    echo "  docker exec -it cis-node1 sh"
    echo
    echo "手动组网:"
    echo "  ./agent-pair.sh mesh"
}

# 主流程
main() {
    check_docker
    
    # 根据参数选择模式
    case "${1:-auto}" in
        build)
            cleanup
            start_build
            ;;
        prebuilt|binary)
            if ! check_binary; then
                echo -e "${RED}错误: 没有可用的 Linux 二进制文件${NC}"
                echo "请使用: ./test.sh build"
                exit 1
            fi
            cleanup
            start_prebuilt
            ;;
        cleanup|clean)
            cleanup
            echo -e "${GREEN}✓${NC} 已清理"
            exit 0
            ;;
        status)
            show_status
            exit 0
            ;;
        auto|*)
            # 自动模式：优先使用预编译二进制
            if check_binary; then
                cleanup
                start_prebuilt
            else
                echo -e "${YELLOW}!${NC} 未找到 Linux 二进制，将使用构建模式"
                cleanup
                start_build
            fi
            ;;
    esac
    
    wait_nodes
    run_test
    show_status
}

main "$@"
