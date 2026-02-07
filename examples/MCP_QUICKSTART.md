# MCP 快速启动 - 跨平台编译场景

## 场景

Claude 想要同时在多台电脑上编译项目:
- 电脑A (Mac) 发起任务
- 电脑B (Linux + NVIDIA) 编译 CUDA
- 电脑C (Mac Studio) 编译 Metal

## 启动步骤

### Step 1: 电脑B 和 电脑C 启动 Worker

```bash
# 在电脑B (CUDA) 上执行
cis-mcp &
cis-node worker run \
  --worker-id "cuda-builder" \
  --room "!cis-room:matrix.org" \
  --parent-node "computer-b"

# 在电脑C (Metal) 上执行  
cis-mcp &
cis-node worker run \
  --worker-id "metal-builder" \
  --room "!cis-room:matrix.org" \
  --parent-node "computer-c"
```

### Step 2: 电脑A 通过 Claude 发起任务

```
User: "启动跨平台编译"

Claude: 调用 cis-mcp tool
  ↓
dag_create_run
  dag_file: "cross_compile_pipeline.toml"
  target_scope: "project"
  
CIS: 创建 DAG 运行，分发任务
  ├─ 任务1 → 电脑B (target_node: computer-b)
  └─ 任务2 → 电脑C (target_node: computer-c)
  
电脑B Worker: 认领任务
  ├─ 检查 git
  ├─ 拉代码
  ├─ 编译 CUDA
  ├─ 失败? → 启动 Claude Agent 修复
  └─ 成功 → 报告状态
  
电脑C Worker: 认领任务
  ├─ 检查 git
  ├─ 拉代码
  ├─ 编译 Metal
  ├─ 失败? → 启动 Claude Agent 修复
  └─ 成功 → 报告状态
  
CIS: 等待两边完成
  ↓
合并结果 → 发布到 git

Claude: 报告完成
  "CUDA 和 Metal 编译已完成，已发布到 release/v2024.02.06 分支"
```

## 实时查询

```
User: "编译进度如何?"

Claude: 调用 dag_get_status
  ↓
返回:
  CUDA (电脑B): 编译中... 75%
  Metal (电脑C): 已完成 ✓
  
User: "CUDA 编译失败了?"

Claude: 查看 TODO 列表
  ├─ cuda-compile: 失败 (日志: syntax error in kernel.cu:45)
  └─ cuda-fix-and-retry: 运行中 (Claude Agent 正在修复)
  
Claude: "CUDA 编译遇到语法错误，已启动自动修复 Agent，预计 2 分钟完成"
```

## 动态调整

```
User: "CUDA 编译太慢了，先发布 Metal 版本"

Claude: 调用 dag_todo_propose
  changes:
    modify:
      - id: "publish-release"
        condition: "metal-test.status == 'completed'"
        description: "仅发布 Metal 版本"
  reason: "用户要求优先发布 Metal"
  
Worker: 审核提案
  ✓ 接受修改
  
CIS: 立即发布 Metal 版本
  (CUDA 继续后台编译)
```

## 一键命令

```bash
# 快速启动整个流水线
cis-pipeline cross-compile \
  --targets computer-b,computer-c \
  --on-failure auto-fix \
  --notify "编译完成"
```
