#!/bin/bash
# Agent-D: T2.1 P2P Network + T3.2 (等待中)

AGENT="Agent-D"
TASK="T2.1 P2P Network + T3.2"
WORK_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$WORK_DIR/../.." && pwd)"
LOG="$WORK_DIR/log.txt"

echo "[$AGENT] 🟡 任务: $TASK" | tee "$LOG"
echo "[$AGENT] 状态: 等待依赖" | tee -a "$LOG"
echo "" | tee -a "$LOG"

cd "$PROJECT_ROOT"

# 检查依赖是否完成
A_DONE=false
B_DONE=false

if [ -f "$PROJECT_ROOT/parallel-work/agent-a/.status" ]; then
    A_STATUS=$(cat "$PROJECT_ROOT/parallel-work/agent-a/.status")
    if [ "$A_STATUS" = "completed" ]; then
        A_DONE=true
        echo "[$AGENT] ✅ T1.1 (Agent-A) 已完成" | tee -a "$LOG"
    fi
fi

if [ -f "$PROJECT_ROOT/parallel-work/agent-b/.status" ]; then
    B_STATUS=$(cat "$PROJECT_ROOT/parallel-work/agent-b/.status")
    if [ "$B_STATUS" = "completed" ]; then
        B_DONE=true
        echo "[$AGENT] ✅ T1.2 (Agent-B) 已完成" | tee -a "$LOG"
    fi
fi

if [ "$A_DONE" = "true" ] && [ "$B_DONE" = "true" ]; then
    echo "[$AGENT] 🚀 依赖已满足，可以开始 T2.1" | tee -a "$LOG"
    echo "in_progress" > "$WORK_DIR/.status"
    
    # 开始 T2.1
    echo "[$AGENT] 创建分支 agent-d/t2.1-network..." | tee -a "$LOG"
    git checkout -b agent-d/t2.1-network 2>/dev/null || git checkout agent-d/t2.1-network 2>/dev/null
    
    # 实现 P2PNetwork...
    echo "[$AGENT] 实现 P2P Network 状态管理..." | tee -a "$LOG"
    
else
    echo "[$AGENT] ⏳ 等待中..." | tee -a "$LOG"
    if [ "$A_DONE" = "false" ]; then
        echo "[$AGENT]    - 等待 T1.1 (Agent-A: mDNS)" | tee -a "$LOG"
    fi
    if [ "$B_DONE" = "false" ]; then
        echo "[$AGENT]    - 等待 T1.2 (Agent-B: QUIC)" | tee -a "$LOG"
    fi
    echo "waiting" > "$WORK_DIR/.status"
fi

echo "" | tee -a "$LOG"
echo "[$AGENT] 📊 状态: $(cat $WORK_DIR/.status)" | tee -a "$LOG"
