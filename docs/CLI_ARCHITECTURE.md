# CIS CLI 架构设计

## 设计原则

### Docker 风格命令结构

```
cis <resource> <action> [options] [arguments]
```

### 资源类型

| 资源 | 说明 | 主要命令 |
|------|------|----------|
| `worker` | Worker 进程 | ls, run, inspect, stop, rm, logs, stats, top |
| `node` | 联邦节点 | ls, bind, inspect, disconnect, blacklist, unblacklist |
| `dag` | DAG 工作流 | ls, run, inspect, rm, logs, pause, unpause |
| `task` | 任务 | ls, create, inspect, cancel, retry, rm |
| `skill` | Skill 插件 | ls, load, activate, deactivate, rm |
| `network` | 网络/联邦 | peers, connect, disconnect, sync |

### 通用命令模式

#### 列表查询
```bash
# 列出运行中的资源
cis worker ls
cis dag ls
cis task ls

# 列出所有（包括停止的）
cis worker ls -a
cis dag ls --all

# 过滤器
cis worker ls --filter status=running --filter scope=global
cis task ls --filter status=failed

# 格式化输出
cis worker ls --format json
cis node ls --format wide

# 安静模式（仅 ID）
cis worker ls -q
```

#### 详情查看
```bash
# 查看资源详情
cis worker inspect <id>
cis dag inspect <id>

# 自定义格式
cis worker inspect <id> --format "{{.Status}}"
cis node inspect <id> --format "{{.DID}}"
```

#### 创建/运行
```bash
# 创建并运行
cis worker run --room <room> --scope global
cis dag run <dag-id> --param key=value

# 仅创建
cis task create --name <name> --type <type>
```

#### 停止/删除
```bash
# 停止
cis worker stop <id>
cis dag pause <id>
cis task cancel <id>

# 强制停止
cis worker stop <id> --force

# 删除
cis worker rm <id>
cis dag rm <id>

# 批量删除
cis worker rm <id1> <id2> <id3>

# 清理已停止的
cis worker prune
```

#### 日志和监控
```bash
# 查看日志
cis worker logs <id>
cis worker logs <id> -f --tail 100

# 资源统计
cis worker stats
cis worker stats <id>

# Top 排序
cis worker top --sort cpu --limit 10
```

## 数据服务层

### 架构图

```
┌─────────────────────────────────────────────────────────┐
│                      访问层                              │
├─────────────┬─────────────┬─────────────┬───────────────┤
│   CLI       │    GUI      │    API      │   WebSocket   │
│  (cis-node) │  (cis-gui)  │  (HTTP/gRPC)│  (Realtime)   │
└──────┬──────┴──────┬──────┴──────┬──────┴───────┬───────┘
       │             │             │              │
       └─────────────┴──────┬──────┴──────────────┘
                            │
┌───────────────────────────┼─────────────────────────────┐
│                    服务层  │                             │
│  ┌───────────┬───────────┼───────────┬───────────────┐  │
│  │ Worker    │   Node    │   DAG     │    Task       │  │
│  │ Service   │  Service  │  Service  │   Service     │  │
│  └───────────┴───────────┴───────────┴───────────────┘  │
└─────────────────────────────────────────────────────────┘
                            │
┌───────────────────────────┼─────────────────────────────┐
│                    数据层  │                             │
│  ┌───────────┬───────────┼───────────┬───────────────┐  │
│  │ SQLite    │   DID     │  Matrix   │    File       │  │
│  │  (Core)   │ (Identity)│  (Events) │   System      │  │
│  └───────────┴───────────┴───────────┴───────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 服务接口

```rust
// 统一的服务特征
#[async_trait]
pub trait ResourceService: Send + Sync {
    type Resource;
    type ResourceSummary;
    type ResourceInfo;

    async fn list(&self, options: ListOptions) -> Result<PaginatedResult<Self::ResourceSummary>>;
    async fn inspect(&self, id: &str) -> Result<Self::ResourceInfo>;
    async fn remove(&self, id: &str, force: bool) -> Result<()>;
    async fn stats(&self, id: &str) -> Result<ResourceStats>;
}
```

## 实现规范

### 1. 命令分组

每个资源类型对应一个命令模块：

```
cis-node/src/commands/
├── mod.rs           # 命令模块入口
├── worker.rs        # Worker 命令（已完成）
├── node.rs          # 节点命令
├── dag.rs           # DAG 命令
├── task.rs          # 任务命令
├── skill.rs         # Skill 命令
└── network.rs       # 网络命令
```

### 2. 错误处理

- 使用统一的错误类型 `CisError`
- CLI 层负责将错误转换为人类可读的消息
- 支持 `--debug` 显示详细错误信息

### 3. 输出格式

```rust
pub enum OutputFormat {
    Table,    // 默认表格格式
    Json,     // JSON 格式（用于脚本）
    Wide,     // 宽表格（显示更多列）
    Quiet,    // 仅 ID
}
```

### 4. ID 匹配

- 支持完整 ID：`cis worker inspect abc123def-456`
- 支持前缀匹配：`cis worker inspect abc`（如果唯一）
- 模糊匹配错误提示：`Multiple workers match prefix 'abc'`

### 5. 批量操作

```bash
# 管道支持
cis worker ps -q | xargs cis worker stop

# 直接批量
cis worker stop <id1> <id2> <id3>
cis worker rm $(cis worker ps -aq)
```

## 迁移计划

### 阶段 1: Worker CLI（已完成）
- ✅ Docker 风格命令
- ✅ Service 层抽象

### 阶段 2: 核心资源
- ⬜ Node CLI (`cis node`)
- ⬜ DAG CLI (`cis dag`)
- ⬜ Task CLI (`cis task`)

### 阶段 3: 扩展资源
- ⬜ Skill CLI 重构 (`cis skill`)
- ⬜ Network CLI 重构 (`cis network`)

### 阶段 4: API 暴露
- ⬜ HTTP REST API
- ⬜ gRPC API
- ⬜ WebSocket 实时推送
