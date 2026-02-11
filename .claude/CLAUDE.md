# CIS (Cluster of Independent Systems) - Claude 使用指南

> **版本**: v1.1.5  
> **适用对象**: Claude Code CLI, Claude Desktop, Claude API  
> **最后更新**: 2026-02-11

---

## 快速开始

当你作为 Claude 被用户使用 CIS 时，请遵循以下引导：

```
┌─────────────────────────────────────────────────────────────┐
│  用户任务 → 识别需求 → 选择 CIS 能力 → 执行 → 记忆归档       │
└─────────────────────────────────────────────────────────────┘
```

### 1. 识别 CIS 能力需求

| 用户场景 | CIS 能力 | 调用方式 |
|---------|---------|---------|
| "记住这个偏好" / "以后提醒我" | **记忆存储** | `memory.set` |
| "查找之前的配置" / "我设置过什么" | **记忆搜索** | `memory.search` |
| "执行这个 workflow" / "按步骤执行" | **DAG 编排** | `dag.execute` |
| "与其他设备同步" / "分享给团队" | **P2P 网络** | `p2p.sync` |
| "用 Element 登录" / "Matrix 消息" | **联邦网关** | `matrix.*` |

---

## 记忆系统使用指南

### 核心概念

CIS 记忆分为 **私域 (Private)** 和 **公域 (Public)**：

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS 记忆架构                              │
├────────────────────────────┬────────────────────────────────┤
│        私域记忆             │          公域记忆               │
│        (Private)           │          (Public)              │
├────────────────────────────┼────────────────────────────────┤
│ • 本地加密存储              │ • 明文存储                      │
│ • 永不同步                  │ • 可 P2P 同步                   │
│ • 敏感信息                  │ • 共享配置                      │
│ • API Keys, 个人偏好        │ • 项目设置, 团队约定             │
└────────────────────────────┴────────────────────────────────┘
```

### Claude 应该何时使用记忆

**✅ 应该存储到 CIS 记忆：**
- 用户明确说"记住"、"保存"、"记下来"
- 用户的偏好设置 (主题、语言、默认行为)
- 项目特定的配置约定
- 需要跨会话保持的上下文

**❌ 不应该存储：**
- 临时计算结果
- 敏感凭证 (使用系统 keychain 或私域记忆)
- 一次性查询结果

### 记忆操作模板

#### 存储记忆

```rust
// 当用户说："记住我喜欢深色主题"
// 你应该调用 CIS 记忆 API:

use cis_core::memory::{MemoryService, MemoryDomain, MemoryCategory};

// 存储到公域 (可同步给其他设备)
service.set(
    "user/preference/theme", 
    b"dark", 
    MemoryDomain::Public,      // 或 Private
    MemoryCategory::Context
).await?;

// 语义索引 (支持自然语言搜索)
service.set_with_embedding(
    "user/preference/theme",
    b"用户偏好使用深色主题界面",
    MemoryDomain::Public,
    MemoryCategory::Context
).await?;
```

#### 搜索记忆

```rust
// 当用户说："我之前设置过什么主题？"
// 使用语义搜索:

let results = service.semantic_search(
    "用户主题偏好设置",  // 自然语言查询
    5,                    // 返回数量
    0.7                   // 相似度阈值
).await?;

// 或精确查找
if let Some(item) = service.get("user/preference/theme").await? {
    println!("主题: {:?}", String::from_utf8_lossy(&item.value));
}
```

### 记忆键命名规范

```
{domain}/{category}/{identifier}

示例：
• user/preference/language          → 用户语言偏好
• project/{id}/database-config      → 项目数据库配置  
• workflow/{name}/last-run          → 工作流上次运行
• device/{hostname}/settings        → 设备特定设置
```

---

## DAG 编排使用指南

### 什么是 DAG

DAG (有向无环图) 用于编排多步骤任务，支持：
- **依赖管理**: 任务按依赖顺序执行
- **并行执行**: 无依赖的任务并行运行
- **故障恢复**: 支持重试和回滚
- **多级决策**: Mechanical → Recommended → Confirmed → Arbitrated

### Claude 应该何时使用 DAG

**✅ 适合使用 DAG 的场景：**
- 多步骤 workflow (代码审查 → 测试 → 部署)
- 需要按顺序执行的任务链
- 可以并行化的独立子任务
- 需要持久化和追踪的复杂操作

**❌ 不适合 DAG 的场景：**
- 简单的单步命令
- 需要实时交互的操作
- 一次性临时任务

### DAG 定义模板

```toml
# 保存为: .cis/dags/my-workflow.toml

[skill]
name = "code-review-and-deploy"
version = "1.0.0"
type = "dag"
description = "代码审查并部署"

[dag]
policy = "all_success"  # 所有任务必须成功

[[dag.tasks]]
id = "1"
name = "获取代码变更"
skill = "git-diff"
level = { type = "mechanical", retry = 3 }  # 机械级，自动重试

[[dag.tasks]]
id = "2"
name = "AI 代码审查"
skill = "ai-code-review"
deps = ["1"]  # 依赖任务 1
level = { type = "confirmed" }  # 确认级，需要人工确认
agent = "claude"  # 指定使用 Claude

[[dag.tasks]]
id = "3"
name = "运行测试"
skill = "cargo-test"
deps = ["2"]
level = { type = "mechanical", retry = 2 }

[[dag.tasks]]
id = "4"
name = "部署"
skill = "deploy"
deps = ["3"]
level = { type = "recommended", timeout = 300, default_action = "execute" }
```

### 四级决策级别

| 级别 | 适用场景 | 行为 | 示例 |
|-----|---------|------|------|
| **Mechanical** | 低风险、可自动化 | 自动重试，无需确认 | 代码格式化、静态检查 |
| **Recommended** | 中等风险、有默认 | 执行但通知，可撤销 | 测试运行、文档生成 |
| **Confirmed** | 高风险、需批准 | 等待人工确认 | 代码提交、配置变更 |
| **Arbitrated** | 关键决策、多方参与 | 需要多方投票 | 架构变更、发布决策 |

### 执行 DAG

```bash
# 通过 CLI 执行
cis skill run code-review-and-deploy

# 或在 Rust 代码中:
use cis_core::scheduler::{DagScheduler, TaskDag};

let dag = TaskDag::from_file(".cis/dags/my-workflow.toml").await?;
let scheduler = DagScheduler::new();
let result = scheduler.execute(dag).await?;
```

---

## 网络依赖使用指南

### P2P 组网模式

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS 组网模式                             │
├──────────────┬──────────────┬──────────────┬────────────────┤
│   单机模式    │  局域网 mDNS  │   P2P 公网    │    混合模式     │
├──────────────┼──────────────┼──────────────┼────────────────┤
│ 个人使用      │ 团队内网      │ 分布式团队    │ 企业部署        │
│ 无网络依赖    │ 自动发现      │ NAT穿透       │ 云端+本地       │
│ ✅ 可用       │ ✅ 可用       │ ✅ 可用       │ ⚠️ 部分可用     │
└──────────────┴──────────────┴──────────────┴────────────────┘
```

### Claude 应该何时使用网络功能

**✅ 使用 P2P 网络：**
- 用户有多个 CIS 节点需要同步
- 需要与团队成员共享记忆/配置
- 跨设备状态同步

**❌ 不使用网络：**
- 单机使用场景
- 敏感数据 (使用私域记忆)

### 网络配置模板

```toml
# ~/.cis/config.toml

[p2p]
enabled = true
listen_port = 7677
bootstrap_nodes = [
    "/dns/cis-bootstrap.example.com/tcp/7677/p2p/12D3KooW..."
]

[p2p.discovery]
enable_mdns = true        # 局域网自动发现
enable_dht = true         # DHT 公网发现

[p2p.sync]
auto_sync = true          # 自动同步公域记忆
sync_interval_secs = 300  # 每5分钟同步
```

### 公域记忆同步

```rust
// 当用户说："把这个配置同步到其他设备"
// 公域记忆会自动同步，无需手动操作

// 手动触发同步
let pending = memory_service.get_pending_sync(100).await?;
for marker in pending {
    p2p.sync_memory(&marker.key, &marker.sync_peers).await?;
    memory_service.mark_synced(&marker.key).await?;
}
```

---

## 完整对话示例

### 示例 1: 记忆用户偏好

**用户**: "记住我喜欢用深色主题"

**Claude 应该**:
```rust
// 1. 识别意图: 存储用户偏好到 CIS 记忆
// 2. 选择域: Public (跨设备同步)
// 3. 执行存储

service.set_with_embedding(
    "user/preference/theme",
    b"dark",
    MemoryDomain::Public,
    MemoryCategory::Context
).await?;

// 4. 确认
"好的，我已记住你偏好深色主题。这个设置会在你所有 CIS 设备间同步。"
```

### 示例 2: 创建代码审查 DAG

**用户**: "帮我创建一个代码审查 workflow"

**Claude 应该**:
```toml
// 1. 创建 DAG 配置文件
// 2. 保存到 .cis/dags/code-review.toml
// 3. 使用四级决策: Mechanical → Confirmed
```

**回复**:
"已为你创建代码审查 workflow，包含以下步骤：
1. 获取代码变更 (Mechanical - 自动执行)
2. AI 代码审查 (Confirmed - 需要确认)
3. 运行测试 (Mechanical - 自动执行)

运行: `cis skill run code-review`
"

### 示例 3: 同步配置到团队

**用户**: "把这个项目配置分享给团队"

**Claude 应该**:
```rust
// 1. 确认是公域记忆 (会自动同步)
// 2. 如果当前是私域，询问是否转换
// 3. 手动触发同步

service.set(
    "project/xyz/config",
    config_json,
    MemoryDomain::Public,  // 公域 = 可同步
    MemoryCategory::Context
).await?;

// 4. 触发 P2P 同步
p2p.broadcast_public_memory().await?;
```

**回复**:
"已将项目配置保存到公域记忆，正在同步到已连接的团队节点。团队成员在下次同步后将能看到此配置。"

---

## API 快速参考

### 记忆 API

```rust
// 存储
service.set(key, value, domain, category).await?;
service.set_with_embedding(key, value, domain, category).await?;

// 读取
let item = service.get(key).await?;
let items = service.search(query, options).await?;
let results = service.semantic_search(query, limit, threshold).await?;

// 删除
service.delete(key).await?;

// 列出
let keys = service.list_keys(Some(domain)).await?;
```

### DAG API

```rust
// 从文件加载
let dag = TaskDag::from_file("path/to/dag.toml").await?;

// 编程式构建
let mut dag = TaskDag::new();
dag.add_node(id, deps, level, rollback)?;
dag.validate()?;

// 执行
let scheduler = DagScheduler::new();
let result = scheduler.execute(dag).await?;
```

### P2P API

```rust
// 网络管理
let p2p = P2PNetwork::new(node_id, did, bind_addr).await?;
p2p.start().await?;

// 发现节点
let peers = p2p.get_connected_peers().await;

// 广播
p2p.broadcast(topic, data).await?;

// 同步记忆
p2p.sync_memory(key, peers).await?;
```

---

## 故障排查

### 记忆搜索不到

```bash
# 检查向量引擎状态
cis memory status

# 重建索引
cis memory rebuild-index

# 检查存储位置
ls -la ~/.cis/data/core/
```

### DAG 执行失败

```bash
# 查看执行日志
cis dag logs <execution-id>

# 检查任务状态
cis dag status <execution-id>

# 重试失败任务
cis dag retry <execution-id>
```

### P2P 连接问题

```bash
# 检查网络状态
cis p2p status

# 查看发现的节点
cis p2p peers

# 手动连接
cis p2p connect <node-id>
```

---

## 最佳实践

1. **记忆键命名**: 使用 `domain/category/identifier` 层次结构
2. **域选择**: 敏感信息用 Private，共享配置用 Public
3. **DAG 设计**: 机械任务自动执行，高风险任务需确认
4. **错误处理**: 所有记忆/DAG/网络操作都要处理错误
5. **索引优化**: 重要记忆使用 `set_with_embedding` 建立语义索引

---

## 相关文档

- [Agent 配置指南](./docs/AGENT_CONFIGURATION_GUIDE.md)
- [分布式 DAG 协调器](./docs/DISTRIBUTED_DAG_COORDINATOR.md)
- [组网指南](./docs/NETWORKING.md)
- [存储设计](./docs/STORAGE_DESIGN.md)
- [快速开始](./docs/getting-started/quickstart.md)

---

**提示**: 本文档自动注入到 Claude 的上下文中。当用户提及 CIS 相关功能时，请参考本文档的指导。
