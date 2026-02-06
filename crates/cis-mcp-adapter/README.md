# CIS MCP Server

CIS 的 Model Context Protocol (MCP) 服务，将 CIS 能力暴露给 AI Agent。

## 安装

```bash
cargo build -p cis-mcp-adapter
```

## 配置

### Claude Desktop

复制配置文件到 Claude Desktop 配置目录：

```bash
# macOS
cp config/claude-desktop.json ~/Library/Application\ Support/Claude/claude_desktop_config.json

# Linux
cp config/claude-desktop.json ~/.config/claude/claude_desktop_config.json
```

### OpenCode

```bash
cp config/opencode.yaml ~/.opencode/mcp.yaml
```

## 可用工具

### DAG 工具
- `dag_create_run` - 创建 DAG 运行
- `dag_get_status` - 获取 DAG 状态（含 TODO list）
- `dag_control` - 控制 DAG（pause/resume/abort）
- `dag_list` - 列出 DAG 运行
- `dag_todo_propose` - 向 Worker 提案 TODO 变更
- `dag_worker_list` - 列出 Workers

### Skill 工具
- `skill_execute` - 执行 Skill

### Memory 工具
- `memory_store` - 存储记忆
- `memory_recall` - 召回记忆

### Context 工具
- `context_extract` - 提取项目上下文

## 测试

```bash
cd tests
./test_mcp.sh
```

## 使用示例

Agent 可以通过 MCP 调用 CIS 能力：

```
User: "检查我的 DAG 运行状态"
Agent: 调用 dag_list
CIS: 返回运行列表
Agent: 用户有 3 个运行中的 DAG...
```
