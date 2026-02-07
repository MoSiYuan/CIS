# 跨平台编译流水线部署指南

## 场景

- **电脑A** (主控): MacBook Pro, 负责发起任务
- **电脑B** (CUDA): Linux + NVIDIA GPU, 负责 CUDA 编译
- **电脑C** (Metal): Mac Studio, 负责 Metal 编译

## 部署步骤

### 1. 所有电脑安装 CIS

```bash
# 克隆仓库
git clone https://github.com/your-org/cis.git
cd cis

# 构建
cargo build --release

# 添加到 PATH
sudo cp target/release/cis-node /usr/local/bin/
sudo cp target/release/cis-mcp /usr/local/bin/
```

### 2. 配置 Matrix Room (所有电脑)

编辑 `~/.cis/config.toml`:

```toml
[node]
id = "computer-a"  # 电脑A填 computer-a, 电脑B填 computer-b, 电脑C填 computer-c

[matrix]
enabled = true
homeserver = "https://matrix.org"
room_id = "!your-room-id:matrix.org"
access_token = "your-token"
```

### 3. 电脑B 和 电脑C 启动 Worker

```bash
# 电脑B (CUDA)
cis-node worker run \
  --worker-id "cuda-worker" \
  --room "!your-room-id:matrix.org" \
  --scope project \
  --scope-id "ml-accelerator" \
  --parent-node "computer-b"

# 电脑C (Metal)  
cis-node worker run \
  --worker-id "metal-worker" \
  --room "!your-room-id:matrix.org" \
  --scope project \
  --scope-id "ml-accelerator" \
  --parent-node "computer-c"
```

### 4. 电脑A 发起编译

```bash
# 方法1: 使用 DAG 文件
cis-node dag run examples/cross_compile_pipeline.toml

# 方法2: 使用 MCP (Claude)
# Claude: "启动跨平台编译"
```

## 执行流程

```
电脑A (Claude)
    ↓ 发起 DAG 运行
Matrix Room
    ↓ 广播 DAG 任务
├──────────────┬──────────────┐
▼              ▼              ▼
电脑B         电脑C          电脑A
(CUDA)       (Metal)       (合并)
├─ 检查git    ├─ 检查git     ├─ 等待结果
├─ 拉代码     ├─ 拉代码      ├─ 合并分支
├─ 编译CUDA   ├─ 编译Metal   └─ 发布
├─ 测试       ├─ 测试
└─ 报告       └─ 报告
```

## 自动修复流程

当编译失败时:

1. **Worker 检测到失败** → 触发 `claude-code-fix` Skill
2. **启动 Claude Agent** → 分析错误日志
3. **自动修复代码** → 修改源代码
4. **重试编译** → 最多5次
5. **成功后提交** → `git commit -m "fix: auto-fix by CIS"`

## 监控命令

```bash
# 查看运行状态 (电脑A)
cis-node dag status

# 查看 TODO 列表
cis-node dag todo list --run-id <run-id>

# 查看 Worker 状态
cis-node dag worker list

# 查看日志
cis-node dag logs --run-id <run-id> --follow
```

## 故障排查

### Worker 未认领任务

```bash
# 检查 Worker 是否在线
cis-node dag worker list

# 检查 target_node 是否匹配
# 确保 DAG 中的 target_node 与 Worker 的 node_id 一致
```

### 编译一直失败

```bash
# 查看 Agent 会话
cis-node dag sessions --run-id <run-id>

# 附加到会话查看 Claude 的修复过程
cis-node dag attach --session <session-id>
```

### Matrix 连接问题

```bash
# 测试 Matrix 连接
cis-node matrix ping

# 查看 Room 成员
cis-node matrix members --room <room-id>
```
