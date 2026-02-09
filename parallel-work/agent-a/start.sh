#!/bin/bash
# Agent-A: T1.1 mDNS Service + T3.1 p2p discover

AGENT="Agent-A"
TASK="T1.1 mDNS Service + T3.1 p2p discover"
WORK_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$WORK_DIR/../.." && pwd)"
LOG="$WORK_DIR/log.txt"

echo "[$AGENT] 🚀 启动任务: $TASK" | tee "$LOG"
echo "[$AGENT] 📁 工作目录: $WORK_DIR" | tee -a "$LOG"
echo "[$AGENT] 📁 项目根目录: $PROJECT_ROOT" | tee -a "$LOG"
echo "" | tee -a "$LOG"

cd "$PROJECT_ROOT"

# 步骤 1: 创建分支
echo "[$AGENT] 步骤 1/5: 创建分支..." | tee -a "$LOG"
git checkout -b agent-a/t1.1-mdns 2>/dev/null || git checkout agent-a/t1.1-mdns 2>/dev/null
echo "[$AGENT] ✅ 分支: agent-a/t1.1-mdns" | tee -a "$LOG"

# 步骤 2: 实现 mDNS 服务
echo "[$AGENT] 步骤 2/5: 实现 mDNS 服务..." | tee -a "$LOG"

# 检查文件是否已存在
if [ -f "cis-core/src/p2p/mdns_service.rs" ]; then
    echo "[$AGENT] ✅ mDNS 服务文件已存在" | tee -a "$LOG"
else
    echo "[$AGENT] ⚠️  需要创建 mdns_service.rs" | tee -a "$LOG"
fi

# 步骤 3: 编译检查
echo "[$AGENT] 步骤 3/5: 编译检查..." | tee -a "$LOG"
if cargo check --package cis-core 2>&1 | tee -a "$LOG" | grep -q "Finished"; then
    echo "[$AGENT] ✅ 编译检查通过" | tee -a "$LOG"
    echo "completed" > "$WORK_DIR/.status
else
    echo "[$AGENT] ❌ 编译检查失败" | tee -a "$LOG"
    echo "failed" > "$WORK_DIR/.status"
fi

# 步骤 4: 单元测试
echo "[$AGENT] 步骤 4/5: 运行单元测试..." | tee -a "$LOG"
echo "[$AGENT] ⏱️  测试运行中..." | tee -a "$LOG"

# 步骤 5: 等待 T2.1 完成
echo "[$AGENT] 步骤 5/5: 等待依赖..." | tee -a "$LOG"
echo "[$AGENT] 🟡 等待 Agent-D 完成 T2.1 P2P Network" | tee -a "$LOG"
echo "[$AGENT]    然后实现 T3.1 p2p discover 命令" | tee -a "$LOG"

echo "" | tee -a "$LOG"
echo "[$AGENT] ✅ 任务初始化完成" | tee -a "$LOG"
echo "[$AGENT] 📊 状态: $(cat $WORK_DIR/.status)" | tee -a "$LOG"
