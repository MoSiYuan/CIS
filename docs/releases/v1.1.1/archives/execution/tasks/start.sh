#!/bin/bash
# CIS v1.1.0 Agent 快速启动脚本
# 用法: ./start.sh [agent-name]
# 示例: ./start.sh agent-a

set -e

AGENT=${1:-}
PROJECT_ROOT="/Users/jiangxiaolong/work/project/CIS"
TASKS_DIR="$PROJECT_ROOT/plan/tasks"

cd "$PROJECT_ROOT"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}"
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║           CIS v1.1.0 Agent 启动脚本                         ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

print_help() {
    echo "用法: $0 [agent-name]"
    echo ""
    echo "可用的 Agent:"
    echo "  agent-a    - 内存安全修复 (P1-1)"
    echo "  agent-b    - WebSocket测试 (P1-2)"
    echo "  agent-c    - 项目注册表 (P1-3)"
    echo "  agent-d    - CI/CD强化 (P1-5)"
    echo "  agent-e    - 编译警告清理 (P1-6)"
    echo "  agent-f    - 文档测试 (P1-7)"
    echo ""
    echo "示例:"
    echo "  $0 agent-a"
    echo "  $0 agent-b"
}

setup_agent_a() {
    echo -e "${GREEN}启动 Agent-A: 内存安全修复 (P1-1)${NC}"
    
    # 检查分支
    if git branch | grep -q "feat/phase1-p1-1-memory-safety"; then
        git checkout feat/phase1-p1-1-memory-safety
    else
        git checkout -b feat/phase1-p1-1-memory-safety
    fi
    
    # 显示任务文档
    echo -e "${YELLOW}任务文档:${NC}"
    cat "$TASKS_DIR/phase1/P1-1_memory_safety.md" | head -30
    
    # 显示关键文件
    echo -e "${YELLOW}关键文件:${NC}"
    echo "  - cis-core/src/memory/service.rs"
    echo "  - cis-core/src/storage/db.rs"
    
    # 运行测试查看当前状态
    echo -e "${YELLOW}当前测试状态:${NC}"
    cargo test -p cis-core --lib memory::service::tests::test_memory_service_delete 2>&1 | tail -5 || true
    
    echo -e "${GREEN}Agent-A 准备就绪！开始执行 P1-1 任务。${NC}"
}

setup_agent_b() {
    echo -e "${GREEN}启动 Agent-B: WebSocket测试修复 (P1-2)${NC}"
    
    if git branch | grep -q "feat/phase1-p1-2-websocket-tests"; then
        git checkout feat/phase1-p1-2-websocket-tests
    else
        git checkout -b feat/phase1-p1-2-websocket-tests
    fi
    
    echo -e "${YELLOW}任务文档:${NC}"
    cat "$TASKS_DIR/phase1/P1-2_websocket_tests.md" | head -30
    
    echo -e "${YELLOW}关键文件:${NC}"
    echo "  - cis-core/src/matrix/websocket/server.rs"
    
    echo -e "${YELLOW}当前测试状态:${NC}"
    cargo test -p cis-core --lib matrix::websocket::server::tests 2>&1 | tail -10 || true
    
    echo -e "${GREEN}Agent-B 准备就绪！开始执行 P1-2 任务。${NC}"
}

setup_agent_c() {
    echo -e "${GREEN}启动 Agent-C: 项目注册表测试 (P1-3)${NC}"
    
    if git branch | grep -q "feat/phase1-p1-3-project-registry"; then
        git checkout feat/phase1-p1-3-project-registry
    else
        git checkout -b feat/phase1-p1-3-project-registry
    fi
    
    echo -e "${GREEN}Agent-C 准备就绪！开始执行 P1-3 任务。${NC}"
    echo "关键文件: cis-core/src/skill/project_registry.rs"
}

setup_agent_d() {
    echo -e "${GREEN}启动 Agent-D: CI/CD强化 (P1-5)${NC}"
    
    if git branch | grep -q "feat/phase1-p1-5-ci-cd"; then
        git checkout feat/phase1-p1-5-ci-cd
    else
        git checkout -b feat/phase1-p1-5-ci-cd
    fi
    
    echo -e "${GREEN}Agent-D 准备就绪！开始执行 P1-5 任务。${NC}"
    echo "关键目录: .github/workflows/"
}

setup_agent_e() {
    echo -e "${GREEN}启动 Agent-E: 编译警告清理 (P1-6)${NC}"
    
    if git branch | grep -q "feat/phase1-p1-6-clippy-warnings"; then
        git checkout feat/phase1-p1-6-clippy-warnings
    else
        git checkout -b feat/phase1-p1-6-clippy-warnings
    fi
    
    echo -e "${YELLOW}当前警告数量:${NC}"
    cargo clippy -p cis-core 2>&1 | grep "warning:" | wc -l
    
    echo -e "${GREEN}Agent-E 准备就绪！开始执行 P1-6 任务。${NC}"
}

setup_agent_f() {
    echo -e "${GREEN}启动 Agent-F: 文档测试 (P1-7)${NC}"
    
    if git branch | grep -q "feat/phase1-p1-7-doc-tests"; then
        git checkout feat/phase1-p1-7-doc-tests
    else
        git checkout -b feat/phase1-p1-7-doc-tests
    fi
    
    echo -e "${YELLOW}当前 DocTest 状态:${NC}"
    cargo test --doc -p cis-core 2>&1 | tail -5 || true
    
    echo -e "${GREEN}Agent-F 准备就绪！开始执行 P1-7 任务。${NC}"
}

# 主逻辑
print_header

case "$AGENT" in
    agent-a|a)
        setup_agent_a
        ;;
    agent-b|b)
        setup_agent_b
        ;;
    agent-c|c)
        setup_agent_c
        ;;
    agent-d|d)
        setup_agent_d
        ;;
    agent-e|e)
        setup_agent_e
        ;;
    agent-f|f)
        setup_agent_f
        ;;
    *)
        print_help
        exit 1
        ;;
esac

echo ""
echo -e "${BLUE}══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}下一步:${NC}"
echo "1. 阅读完整的任务文档: cat plan/tasks/phase1/P1-X_xxx.md"
echo "2. 开始编码修改"
echo "3. 完成后提交: git add . && git commit -m '...'"
echo "4. 更新进度: 编辑 plan/tasks/EXECUTION_STATUS.md"
echo ""
echo "遇到问题？查看 plan/tasks/AGENT_ASSIGNMENTS.md"
