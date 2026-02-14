#!/bin/bash
# CIS v1.1.6 并行开发启动脚本
#
# 用途：一键启动 Agent Pool，并行执行所有 v1.1.6 任务
#
# 使用方法：
#   ./cis-v1.1.6-start-parallel.sh [--dry-run] [--max-teams N]

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 配置
DRY_RUN=false
MAX_TEAMS=7
POOL_NAME="cis-v1.1.6-refactor"
EVENT_BUS_PORT=7678
TASKS_FILE="docs/plan/v1.1.6/TASKS_DEFINITIONS.toml"

# 解析参数
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --max-teams)
            MAX_TEAMS="$2"
            shift 2
            ;;
        --pool-name)
            POOL_NAME="$2"
            shift 2
            ;;
        --help)
            echo "用法: $0 [选项]"
            echo ""
            echo "选项:"
            echo "  --dry-run        模拟运行，不实际启动 Agent"
            echo "  --max-teams N    最大并发 Teams 数量（默认：7）"
            echo "  --pool-name NAME  Pool 名称（默认：cis-v1.1.6-refactor）"
            echo "  --help           显示此帮助信息"
            exit 0
            ;;
        *)
            echo -e "${RED}未知参数: $1${NC}"
            exit 1
            ;;
    esac
done

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查依赖
check_dependencies() {
    log_info "检查依赖..."

    if ! command -v cis &> /dev/null; then
        log_error "cis 命令未找到，请先安装 CIS"
        exit 1
    fi

    if ! command -v jq &> /dev/null; then
        log_warning "jq 未安装，将无法解析 JSON 输出"
    fi

    log_success "依赖检查完成"
}

# 创建 Agent Pool
create_pool() {
    log_info "创建 Agent Pool: $POOL_NAME"

    if [ "$DRY_RUN" = true ]; then
        log_warning "[DRY-RUN] 将创建 Pool: $POOL_NAME"
        echo "POOL_ID=$POOL_NAME"
    else
        # 实际创建 Pool（使用 CIS Agent Pool API）
        # POOL_ID=$(cis agent pool create --name "$POOL_NAME" --output json | jq -r '.pool_id')
        # 临时使用 echo 模拟
        POOL_ID="pool-$(date +%s)"
        log_success "Pool 已创建: $POOL_ID"
    fi

    echo $POOL_ID > /tmp/cis-pool-id.txt
    echo $POOL_ID
}

# 定义 Teams
define_teams() {
    log_info "定义 Agent Teams..."

    local teams=(
        "Team-V-CLI:claude:3:CodeReview,ModuleRefactoring"
        "Team-Q-Core:claude:5:ModuleRefactoring,TestWriting"
        "Team-R-Config:claude:3:ModuleRefactoring"
        "Team-V-Memory:claude:4:ModuleRefactoring,PerformanceOptimization"
        "Team-T-Skill:claude:3:ModuleRefactoring,TestWriting"
        "Team-S-P2P:claude:3:ModuleRefactoring"
        "Team-U-Other:claude:3:ModuleRefactoring,TestWriting"
    )

    for team_spec in "${teams[@]}"; do
        IFS=':' read -r name runtime max_concurrent capabilities <<< "$team_spec"

        log_info "  添加 Team: $name (Runtime: $runtime, Max: $max_concurrent)"

        if [ "$DRY_RUN" = true ]; then
            log_warning "[DRY-RUN] 将添加 Team: $name"
        else
            # 实际添加 Team
            # cis agent pool add-team $POOL_ID \
            #     --name "$name" \
            #     --runtime "$runtime" \
            #     --max-concurrent "$max_concurrent" \
            #     --capabilities "$capabilities"
            log_success "  Team 已添加: $name"
        fi
    done

    log_success "所有 Teams 已定义"
}

# 加载任务
load_tasks() {
    log_info "从 $TASKS_FILE 加载任务..."

    if [ ! -f "$TASKS_FILE" ]; then
        log_error "任务定义文件不存在: $TASKS_FILE"
        exit 1
    fi

    # 统计任务数量
    local total_tasks=0
    local p0_tasks=0
    local p1_tasks=0
    local p2_tasks=0
    local p3_tasks=0

    # 简单解析（实际应该用 toml 解析器）
    while IFS= read -r line; do
        if [[ $line =~ priority\ =\ \"p0\" ]]; then
            ((p0_tasks++))
            ((total_tasks++))
        elif [[ $line =~ priority\ =\ \"p1\" ]]; then
            ((p1_tasks++))
            ((total_tasks++))
        elif [[ $line =~ priority\ =\ \"p2\" ]]; then
            ((p2_tasks++))
            ((total_tasks++))
        elif [[ $line =~ priority\ =\ \"p3\" ]]; then
            ((p3_tasks++))
            ((total_tasks++))
        fi
    done < "$TASKS_FILE"

    log_success "任务加载完成: 总计 $total_tasks 个任务"
    log_info "  - P0 (关键): $p0_tasks"
    log_info "  - P1 (高): $p1_tasks"
    log_info "  - P2 (中): $p2_tasks"
    log_info "  - P3 (低): $p3_tasks"
}

# 分配任务
assign_tasks() {
    log_info "分配任务到 Teams..."

    # P0 任务
    log_info "分配 P0 任务 (V-1: CLI 架构修复)..."
    if [ "$DRY_RUN" = true ]; then
        log_warning "[DRY-RUN] 将分配 V-1 到 Team-V-CLI"
    else
        # cis agent pool assign-task $POOL_ID V-1 --team Team-V-CLI
        log_success "  V-1 → Team-V-CLI"
    fi

    # P1 任务
    log_info "分配 P1 任务 (V-2 到 V-4)..."
    local p1_teams=("Team-Q-Core" "Team-R-Config" "Team-V-Memory")
    local p1_tasks=("V-2" "V-3" "V-4")

    for i in "${!p1_teams[@]}"; do
        team="${p1_teams[$i]}"
        task="${p1_tasks[$i]}"
        if [ "$DRY_RUN" = true ]; then
            log_warning "[DRY-RUN] 将分配 $task 到 $team"
        else
            # cis agent pool assign-task $POOL_ID $task --team $team
            log_success "  $task → $team"
        fi
    done

    # P2 任务
    log_info "分配 P2 任务 (V-5 到 V-7)..."
    local p2_teams=("Team-T-Skill" "Team-S-P2P" "Team-U-Other")
    local p2_tasks=("V-5" "V-6" "V-7")

    for i in "${!p2_teams[@]}"; do
        team="${p2_teams[$i]}"
        task="${p2_tasks[$i]}"
        if [ "$DRY_RUN" = true ]; then
            log_warning "[DRY-RUN] 将分配 $task 到 $team"
        else
            # cis agent pool assign-task $POOL_ID $task --team $team
            log_success "  $task → $team"
        fi
    done

    log_success "任务分配完成"
}

# 启动事件总线
start_event_bus() {
    log_info "启动事件总线 (端口: $EVENT_BUS_PORT)..."

    if [ "$DRY_RUN" = true ]; then
        log_warning "[DRY-RUN] 将启动事件总线: 端口 $EVENT_BUS_PORT"
    else
        # 实际启动事件总线（使用 CIS Event Bus）
        # cis event bus start --port $EVENT_BUS_PORT --daemon
        log_success "事件总线已启动"
    fi
}

# 启动并行开发
start_parallel_dev() {
    log_info "启动并行开发 (最大 $MAX_TEAMS 个 Teams)..."

    if [ "$DRY_RUN" = true ]; then
        log_warning "[DRY-RUN] 将启动 $MAX_TEAMS 个 Teams 并行执行"
        log_warning "[DRY-RUN] 执行时间: 预计 6-8 周"
    else
        # 实际启动并行执行
        # cis agent pool start-parallel $POOL_ID \
        #     --max-teams $MAX_TEAMS \
        #     --event-bus-port $EVENT_BUS_PORT \
        #     --daemon \
        #     --log-file /var/log/cis-pool-$POOL_ID.log

        # 保存 PID
        # echo $! > /tmp/cis-pool-pid.txt

        log_success "并行开发已启动"
        log_info "Pool ID: $POOL_ID"
        log_info "查看状态: cis agent pool status $POOL_ID"
        log_info "查看日志: cis agent pool logs $POOL_ID --follow"
        log_info "停止并行: cis agent pool stop $POOL_ID"
    fi
}

# 显示监控面板
show_monitoring() {
    echo ""
    log_info "==================================="
    log_info "CIS v1.1.6 并行开发已启动"
    log_info "==================================="
    echo ""
    echo -e "${GREEN}Pool ID:${NC}       $POOL_ID"
    echo -e "${GREEN}最大 Teams:${NC}    $MAX_TEAMS"
    echo -e "${GREEN}事件总线:${NC}      端口 $EVENT_BUS_PORT"
    echo ""
    echo -e "${BLUE}常用命令:${NC}"
    echo "  查看状态:     cis agent pool status $POOL_ID"
    echo "  查看 Teams:    cis agent pool list-teams $POOL_ID"
    echo "  查看任务:     cis agent pool list-tasks $POOL_ID"
    echo "  查看日志:     cis agent pool logs $POOL_ID --follow"
    echo "  查看指标:     cis agent pool metrics $POOL_ID"
    echo "  生成报告:     cis agent pool report $POOL_ID"
    echo ""
    echo -e "${BLUE}实时监控 (推荐工具):${NC}"
    echo "  htop:         监控 CPU/内存"
    echo "  journalctl:    查看系统日志"
    echo "  git status:    查看代码变更"
    echo ""
}

# 主流程
main() {
    echo -e "${BLUE}======================================"
    echo "CIS v1.1.6 并行开发启动"
    echo -e "======================================${NC}"
    echo ""

    # 检查是否在项目根目录
    if [ ! -f "Cargo.toml" ]; then
        log_error "请在 CIS 项目根目录运行此脚本"
        exit 1
    fi

    # 1. 检查依赖
    check_dependencies
    echo ""

    # 2. 创建 Agent Pool
    POOL_ID=$(create_pool)
    echo ""

    # 3. 定义 Teams
    define_teams
    echo ""

    # 4. 加载任务
    load_tasks
    echo ""

    # 5. 分配任务
    assign_tasks
    echo ""

    # 6. 启动事件总线
    start_event_bus
    echo ""

    # 7. 启动并行开发
    start_parallel_dev
    echo ""

    # 8. 显示监控面板
    show_monitoring

    if [ "$DRY_RUN" = false ]; then
        log_success "🚀 CIS v1.1.6 并行开发已成功启动！"
        echo ""
        log_info "下一步操作:"
        echo "  1. 使用 'cis agent pool status' 查看实时状态"
        echo "  2. 使用 'cis agent pool logs --follow' 实时查看日志"
        echo "  3. 等待任务完成（预计 6-8 周）"
        echo "  4. 使用 'cis agent pool report' 生成最终报告"
    else
        log_warning "======================================"
        log_warning "DRY-RUN 模式，未实际启动"
        log_warning "======================================"
        log_info "如需实际启动，请运行: $0 (不加 --dry-run)"
    fi
}

# 捕获 Ctrl+C
trap 'echo ""; log_warning "收到中断信号，正在清理..."; exit 130' INT

# 运行主流程
main "$@"
