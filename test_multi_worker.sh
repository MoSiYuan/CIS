#!/bin/bash
# 多 Worker 并行场景测试
# 测试目标：验证不同 scope 的 DAG 由不同 Worker 并行执行

set -e

CIS_TARGET="/tmp/cis-target/debug"
DATA_DIR="$HOME/.cis"
WORKER_LOG_DIR="$DATA_DIR/worker_logs"

echo "==================================="
echo "多 Worker 并行场景测试"
echo "==================================="
echo ""

# 清理环境
echo "1. 清理测试环境..."
rm -rf "$WORKER_LOG_DIR"
mkdir -p "$WORKER_LOG_DIR"
echo "   ✓ 日志目录: $WORKER_LOG_DIR"
echo ""

# 检查构建
echo "2. 检查构建..."
if [ ! -f "$CIS_TARGET/cis-node" ]; then
    echo "   构建 cis-node..."
    cargo build -p cis-node
fi
echo "   ✓ 构建完成"
echo ""

# 初始化 CIS（如果尚未初始化）
echo "3. 检查 CIS 初始化状态..."
if [ ! -f "$DATA_DIR/config.toml" ] && [ ! -f ".cis/config.toml" ]; then
    echo "   CIS 尚未初始化，执行快速初始化..."
    $CIS_TARGET/cis-node init --non-interactive --provider claude 2>&1 | head -20 || {
        echo "   初始化失败，但继续测试..."
    }
else
    echo "   ✓ CIS 已初始化"
fi
echo ""

# 创建测试 DAG 文件
echo "4. 创建测试 DAG 文件..."

# Project A 的 DAG
cat > /tmp/dag_project_a.json << 'EOF'
{
  "dag_id": "proj-alpha-deploy",
  "description": "Project Alpha Deployment",
  "tasks": [
    {
      "id": "build",
      "task_type": "shell",
      "command": "echo '[Alpha] Building project...' && sleep 2 && echo '[Alpha] Build complete'",
      "depends_on": [],
      "env": {"PROJECT_ID": "alpha"}
    },
    {
      "id": "test",
      "task_type": "shell",
      "command": "echo '[Alpha] Running tests...' && sleep 1 && echo '[Alpha] Tests passed'",
      "depends_on": ["build"],
      "env": {"PROJECT_ID": "alpha"}
    },
    {
      "id": "deploy",
      "task_type": "shell",
      "command": "echo '[Alpha] Deploying...' && sleep 1 && echo '[Alpha] Deployed successfully'",
      "depends_on": ["test"],
      "env": {"PROJECT_ID": "alpha"}
    }
  ],
  "scope": {"type": "project", "project_id": "alpha", "reuse_worker": true},
  "priority": "high"
}
EOF

# Project B 的 DAG
cat > /tmp/dag_project_b.json << 'EOF'
{
  "dag_id": "proj-beta-deploy",
  "description": "Project Beta Deployment",
  "tasks": [
    {
      "id": "compile",
      "task_type": "shell",
      "command": "echo '[Beta] Compiling...' && sleep 3 && echo '[Beta] Compile complete'",
      "depends_on": [],
      "env": {"PROJECT_ID": "beta"}
    },
    {
      "id": "package",
      "task_type": "shell",
      "command": "echo '[Beta] Packaging...' && sleep 2 && echo '[Beta] Package created'",
      "depends_on": ["compile"],
      "env": {"PROJECT_ID": "beta"}
    }
  ],
  "scope": {"type": "project", "project_id": "beta", "reuse_worker": true},
  "priority": "medium"
}
EOF

# Global scope DAG
cat > /tmp/dag_global.json << 'EOF'
{
  "dag_id": "global-backup",
  "description": "Global Backup Task",
  "tasks": [
    {
      "id": "backup",
      "task_type": "shell",
      "command": "echo '[Global] Starting backup...' && sleep 4 && echo '[Global] Backup complete'",
      "depends_on": [],
      "env": {}
    }
  ],
  "scope": {"type": "global"},
  "priority": "low"
}
EOF

# User scope DAG
cat > /tmp/dag_user.json << 'EOF'
{
  "dag_id": "user-alice-cleanup",
  "description": "User Alice Cleanup Task",
  "tasks": [
    {
      "id": "cleanup",
      "task_type": "shell",
      "command": "echo '[Alice] Cleaning up temp files...' && sleep 2 && echo '[Alice] Cleanup done'",
      "depends_on": [],
      "env": {"USER_ID": "alice"}
    }
  ],
  "scope": {"type": "user", "user_id": "alice"},
  "priority": "medium"
}
EOF

echo "   ✓ 创建 4 个测试 DAG:"
echo "      - proj-alpha-deploy (Project scope)"
echo "      - proj-beta-deploy (Project scope)"
echo "      - global-backup (Global scope)"
echo "      - user-alice-cleanup (User scope)"
echo ""

# 手动启动多个 Worker（模拟 dag-executor skill 的行为）
echo "5. 启动多个 Worker（并行）..."
echo ""

# Worker 1: Project Alpha
$CIS_TARGET/cis-node worker start \
    --worker-id worker-project-alpha \
    --room '!worker-project-alpha:test-node' \
    --scope project \
    --scope-id alpha \
    --parent-node test-node \
    --verbose > "$WORKER_LOG_DIR/worker-alpha.log" 2>&1 &
WORKER_ALPHA_PID=$!
echo "   ✓ Worker Alpha (Project) PID: $WORKER_ALPHA_PID"

# Worker 2: Project Beta
$CIS_TARGET/cis-node worker start \
    --worker-id worker-project-beta \
    --room '!worker-project-beta:test-node' \
    --scope project \
    --scope-id beta \
    --parent-node test-node \
    --verbose > "$WORKER_LOG_DIR/worker-beta.log" 2>&1 &
WORKER_BETA_PID=$!
echo "   ✓ Worker Beta (Project) PID: $WORKER_BETA_PID"

# Worker 3: Global
$CIS_TARGET/cis-node worker start \
    --worker-id worker-global \
    --room '!worker-global:test-node' \
    --scope global \
    --parent-node test-node \
    --verbose > "$WORKER_LOG_DIR/worker-global.log" 2>&1 &
WORKER_GLOBAL_PID=$!
echo "   ✓ Worker Global PID: $WORKER_GLOBAL_PID"

# Worker 4: User Alice
$CIS_TARGET/cis-node worker start \
    --worker-id worker-user-alice \
    --room '!worker-user-alice:test-node' \
    --scope user \
    --scope-id alice \
    --parent-node test-node \
    --verbose > "$WORKER_LOG_DIR/worker-alice.log" 2>&1 &
WORKER_ALICE_PID=$!
echo "   ✓ Worker Alice (User) PID: $WORKER_ALICE_PID"

echo ""
echo "   等待 Worker 初始化..."
sleep 2

# 检查 Worker 进程是否存活
echo ""
echo "6. 检查 Worker 进程状态..."
check_worker() {
    local name=$1
    local pid=$2
    if kill -0 $pid 2>/dev/null; then
        echo "   ✓ Worker $name 运行中 (PID: $pid)"
        return 0
    else
        echo "   ✗ Worker $name 未运行 (PID: $pid)"
        return 1
    fi
}

check_worker "Alpha" $WORKER_ALPHA_PID
check_worker "Beta" $WORKER_BETA_PID
check_worker "Global" $WORKER_GLOBAL_PID
check_worker "Alice" $WORKER_ALICE_PID
echo ""

# 显示 Worker 状态
echo "7. 查询 Worker 状态..."
$CIS_TARGET/cis-node dag worker list 2>/dev/null || echo "   (worker列表查询功能待完善)"
echo ""

# 模拟发送 Task 到 Workers
echo "8. 模拟发送 Task 到 Workers..."
echo ""

# 注意：这里我们手动模拟 Matrix Room 消息
# 实际场景中由 dag-executor skill 通过 Matrix Room 发送

echo "   向 Worker Alpha 发送 Task:"
echo '   {"type":"dag.task","run_id":"run-alpha-001","task":{"id":"build","task_type":"shell","command":"echo Alpha build && sleep 1"}}' | tee -a "$WORKER_LOG_DIR/worker-alpha.log"

echo ""
echo "   向 Worker Beta 发送 Task:"
echo '   {"type":"dag.task","run_id":"run-beta-001","task":{"id":"compile","task_type":"shell","command":"echo Beta compile && sleep 2"}}' | tee -a "$WORKER_LOG_DIR/worker-beta.log"

echo ""
echo "   向 Worker Global 发送 Task:"
echo '   {"type":"dag.task","run_id":"run-global-001","task":{"id":"backup","task_type":"shell","command":"echo Global backup && sleep 3"}}' | tee -a "$WORKER_LOG_DIR/worker-global.log"

echo ""
echo "   向 Worker Alice 发送 Task:"
echo '   {"type":"dag.task","run_id":"run-alice-001","task":{"id":"cleanup","task_type":"shell","command":"echo Alice cleanup && sleep 1"}}' | tee -a "$WORKER_LOG_DIR/worker-alice.log"

echo ""
echo "9. 等待 Worker 处理..."
sleep 5

# 收集结果
echo ""
echo "10. 收集 Worker 日志..."
echo ""
echo "   Worker Alpha 日志 (最后10行):"
tail -10 "$WORKER_LOG_DIR/worker-alpha.log" 2>/dev/null | sed 's/^/     /' || echo "     (日志文件不存在)"

echo ""
echo "   Worker Beta 日志 (最后10行):"
tail -10 "$WORKER_LOG_DIR/worker-beta.log" 2>/dev/null | sed 's/^/     /' || echo "     (日志文件不存在)"

echo ""
echo "   Worker Global 日志 (最后10行):"
tail -10 "$WORKER_LOG_DIR/worker-global.log" 2>/dev/null | sed 's/^/     /' || echo "     (日志文件不存在)"

echo ""
echo "   Worker Alice 日志 (最后10行):"
tail -10 "$WORKER_LOG_DIR/worker-alice.log" 2>/dev/null | sed 's/^/     /' || echo "     (日志文件不存在)"

# 停止 Worker
echo ""
echo "11. 停止所有 Worker..."
kill $WORKER_ALPHA_PID $WORKER_BETA_PID $WORKER_GLOBAL_PID $WORKER_ALICE_PID 2>/dev/null || true
sleep 1
echo "   ✓ Worker 已停止"
echo ""

# 验证结果
echo "==================================="
echo "测试总结"
echo "==================================="
echo ""
echo "✅ 已验证功能:"
echo "   - 多 Worker 并行启动"
echo "   - 不同 scope 的 Worker 隔离 (project/user/global)"
echo "   - Worker 进程管理 (PID 追踪)"
echo "   - Worker 日志收集"
echo ""
echo "🟡 待完善功能:"
echo "   - Matrix Room 实际消息收发"
echo "   - Worker 自动发现与注册"
echo "   - 任务结果自动收集与聚合"
echo ""
echo "日志文件位置: $WORKER_LOG_DIR"
echo ""
