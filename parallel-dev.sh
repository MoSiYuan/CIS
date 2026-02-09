#!/bin/bash
# CIS 并行开发执行器
# 同时管理多个 Agent 任务

set -e

PROJECT_ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_ROOT"

show_banner() {
    echo "╔══════════════════════════════════════════════════════════════╗"
    echo "║           CIS v1.1.3 并行开发执行器                          ║"
    echo "║           Parallel Development Executor                      ║"
    echo "╚══════════════════════════════════════════════════════════════╝"
    echo ""
}

show_status() {
    echo "📊 当前任务状态:"
    echo ""
    
    # Agent-A
    if [ -f "parallel-work/agent-a/.status" ]; then
        STATUS_A=$(cat parallel-work/agent-a/.status)
        echo "  🟢 Agent-A (T1.1 mDNS): $STATUS_A"
    else
        echo "  ⚪ Agent-A (T1.1 mDNS): 未启动"
    fi
    
    # Agent-B
    if [ -f "parallel-work/agent-b/.status" ]; then
        STATUS_B=$(cat parallel-work/agent-b/.status)
        echo "  🟢 Agent-B (T1.2 QUIC): $STATUS_B"
    else
        echo "  ⚪ Agent-B (T1.2 QUIC): 未启动"
    fi
    
    # Agent-C
    if [ -f "parallel-work/agent-c/.status" ]; then
        STATUS_C=$(cat parallel-work/agent-c/.status)
        echo "  🟢 Agent-C (T1.3 PID): $STATUS_C"
    else
        echo "  ⚪ Agent-C (T1.3 PID): 未启动"
    fi
    
    # Agent-D
    if [ -f "parallel-work/agent-d/.status" ]; then
        STATUS_D=$(cat parallel-work/agent-d/.status)
        echo "  🟡 Agent-D (T2.1 P2P): $STATUS_D [等待T1.1,T1.2]"
    else
        echo "  🟡 Agent-D (T2.1 P2P): 等待中 [等待T1.1,T1.2]"
    fi
    
    # Agent-E
    if [ -f "parallel-work/agent-e/.status" ]; then
        STATUS_E=$(cat parallel-work/agent-e/.status)
        echo "  🟢 Agent-E (T2.3 Detector): $STATUS_E"
    else
        echo "  ⚪ Agent-E (T2.3 Detector): 未启动"
    fi
    
    # Agent-F
    if [ -f "parallel-work/agent-f/.status" ]; then
        STATUS_F=$(cat parallel-work/agent-f/.status)
        echo "  🟢 Agent-F (T4.3 Embedding): $STATUS_F"
    else
        echo "  ⚪ Agent-F (T4.3 Embedding): 未启动"
    fi
    
    echo ""
}

start_all() {
    echo "🚀 启动全部可并行任务..."
    echo ""
    
    # 启动 Agent-A (后台)
    ./parallel-work/agent-a/start.sh 2>&1 | sed 's/^/[Agent-A] /' &
    PID_A=$!
    
    # 启动 Agent-B (后台)
    ./parallel-work/agent-b/start.sh 2>&1 | sed 's/^/[Agent-B] /' &
    PID_B=$!
    
    # 启动 Agent-C (后台)
    ./parallel-work/agent-c/start.sh 2>&1 | sed 's/^/[Agent-C] /' &
    PID_C=$!
    
    # 启动 Agent-E (后台)
    ./parallel-work/agent-e/start.sh 2>&1 | sed 's/^/[Agent-E] /' &
    PID_E=$!
    
    # 启动 Agent-F (后台)
    ./parallel-work/agent-f/start.sh 2>&1 | sed 's/^/[Agent-F] /' &
    PID_F=$!
    
    echo "✅ 全部 Agent 已启动:"
    echo "   Agent-A PID: $PID_A"
    echo "   Agent-B PID: $PID_B"
    echo "   Agent-C PID: $PID_C"
    echo "   Agent-E PID: $PID_E"
    echo "   Agent-F PID: $PID_F"
    echo ""
    echo "💡 查看日志: tail -f parallel-work/*/log.txt"
    echo "💡 查看状态: ./parallel-dev.sh status"
}

create_task_files() {
    echo "📝 创建任务文件..."
    
    # Agent-A: T1.1 mDNS
    mkdir -p parallel-work/agent-a
    echo "in_progress" > parallel-work/agent-a/.status
    echo "T1.1: mDNS Service" > parallel-work/agent-a/.task
    
    # Agent-B: T1.2 QUIC
    mkdir -p parallel-work/agent-b
    echo "in_progress" > parallel-work/agent-b/.status
    echo "T1.2: QUIC Transport" > parallel-work/agent-b/.task
    
    # Agent-C: T1.3 PID
    mkdir -p parallel-work/agent-c
    echo "in_progress" > parallel-work/agent-c/.status
    echo "T1.3: PID Manager" > parallel-work/agent-c/.task
    
    # Agent-D: T2.1 P2P (等待中)
    mkdir -p parallel-work/agent-d
    echo "waiting" > parallel-work/agent-d/.status
    echo "T2.1: P2P Network" > parallel-work/agent-d/.task
    
    # Agent-E: T2.3 Detector
    mkdir -p parallel-work/agent-e
    echo "in_progress" > parallel-work/agent-e/.status
    echo "T2.3: Agent Detector" > parallel-work/agent-e/.task
    
    # Agent-F: T4.3 Embedding
    mkdir -p parallel-work/agent-f
    echo "in_progress" > parallel-work/agent-f/.status
    echo "T4.3: Embedding Service" > parallel-work/agent-f/.task
    
    echo "✅ 任务文件创建完成"
}

# 主逻辑
show_banner

case "${1:-status}" in
    start)
        create_task_files
        start_all
        ;;
    status)
        show_status
        echo "💡 用法:"
        echo "   ./parallel-dev.sh start   # 启动全部任务"
        echo "   ./parallel-dev.sh status  # 查看状态"
        ;;
    *)
        echo "❌ 未知命令: $1"
        echo "用法: ./parallel-dev.sh {start|status}"
        exit 1
        ;;
esac
