# CIS 为主、兼容 ZeroClaw 架构 - 重构计划

## 📋 核心定位

> **CIS 是主项目（私人项目），ZeroClaw 作为兼容层/参考实现**
> 
> 目标：吸收 ZeroClaw 的优秀设计，保持 CIS 独立演进

---

## 1. 架构定位

### 1.1 系统层级

```
┌─────────────────────────────────────────────────────────────┐
│                    CIS Core (主项目)                         │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  CIS 核心模块                        │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌────────┐ │   │
│  │  │ Memory  │  │ Network │  │ Security│  │  Sync  │ │   │
│  │  │ Service │  │ Manager │  │  (DID)  │  │        │ │   │
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └───┬────┘ │   │
│  │       │            │            │           │       │   │
│  │  ┌────┴────────────┴────────────┴───────────┴────┐  │   │
│  │  │              CIS Storage Layer                 │  │   │
│  │  │  (SQLite + sqlite-vec + 私域/公域分离)        │  │   │
│  │  └────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────┘   │
│                            │                                 │
│  ┌─────────────────────────▼────────────────────────────┐   │
│  │              ZeroClaw Compatibility Layer             │   │
│  │  (可选模块，按需启用)                                  │   │
│  │                                                     │   │
│  │  ┌──────────────┐  ┌──────────────┐               │   │
│  │  │ ZcProvider   │  │ ZcChannel    │               │   │
│  │  │ Adapter      │  │ Adapter      │               │   │
│  │  │              │  │              │               │   │
│  │  │ • 22+ 提供商 │  │ • 13+ 通道   │               │   │
│  │  │ • 统一接口   │  │ • 统一接口   │               │   │
│  │  └──────────────┘  └──────────────┘               │   │
│  │                                                     │   │
│  │  ┌──────────────┐  ┌──────────────┐               │   │
│  │  │ ZcSkill      │  │ ZcTool       │               │   │
│  │  │ Adapter      │  │ Adapter      │               │   │
│  │  │              │  │              │               │   │
│  │  │ • SKILL.toml │  │ • 20+ 工具   │               │   │
│  │  │ • open-skills│  │ • 沙箱执行   │               │   │
│  │  └──────────────┘  └──────────────┘               │   │
│  └────────────────────────────────────────────────────┘   │
│                            │                                 │
│  ┌─────────────────────────▼────────────────────────────┐   │
│  │              Configuration Layer                     │   │
│  │                                                     │   │
│  │  [cis]                                              │   │
│  │  mode = "standalone"  # 或 "zeroclaw-compatible"    │   │
│  │                                                     │   │
│  │  [zeroclaw]  # 仅在兼容模式下启用                   │   │
│  │  providers = ["openai", "anthropic"]                │   │
│  │  channels = ["telegram", "discord"]                 │   │
│  │  skills = ["deploy", "git"]                         │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 两种运行模式

```rust
// CIS 运行模式枚举
pub enum CisMode {
    /// 独立模式：仅使用 CIS 核心模块
    Standalone,

    /// ZeroClaw 兼容模式：启用 ZeroClaw 适配器
    ZeroClawCompatible {
        /// 启用的提供商
        providers: Vec<String>,
        /// 启用的通道
        channels: Vec<String>,
        /// 启用的技能
        skills: Vec<String>,
    },
}

impl CisMode {
    /// 创建 Agent
    pub async fn create_agent(&self, config: &CisConfig) -> Result<Box<dyn Agent>> {
        match self {
            CisMode::Standalone => {
                // 使用 CIS 原生 Agent
                Ok(Box::new(CisAgent::new(config).await?))
            }
            CisMode::ZeroClawCompatible { providers, channels, skills } => {
                // 使用 ZeroClaw Agent + CIS 后端
                Ok(Box::new(ZcCompatibleAgent::new(
                    config,
                    providers.clone(),
                    channels.clone(),
                    skills.clone(),
                ).await?))
            }
        }
    }
}
```

---

## 2. 模块重构计划

### 2.1 Phase 1: 项目结构优化（参考 ZeroClaw）

#### 当前 CIS 结构

```
cis-core/
├── src/
│   ├── lib.rs
│   ├── memory/
│   │   ├── mod.rs
│   │   ├── service.rs      # 大文件，需拆分
│   │   ├── scope.rs
│   │   └── weekly_archived.rs
│   ├── storage/
│   │   └── memory_db.rs    # 大文件，需拆分
│   ├── vector/
│   │   └── storage.rs      # 大文件，需拆分
│   ├── network/
│   │   └── manager.rs      # 大文件，需拆分
│   └── security/
│       └── identity.rs     # 大文件，需拆分
```

#### 目标 CIS 结构（参考 ZeroClaw）

```
cis-core/
├── src/
│   ├── lib.rs
│   ├── config.rs           # 配置管理（参考 ZeroClaw config）
│   ├── error.rs            # 错误类型（统一错误处理）
│   ├── types.rs            # 公共类型定义
│   │
│   ├── memory/
│   │   ├── mod.rs          # 模块导出
│   │   ├── traits.rs       # Memory trait（新增）
│   │   ├── service.rs      # MemoryService（精简）
│   │   ├── ops/            # 操作拆分（参考 ZeroClaw）
│   │   │   ├── get.rs
│   │   │   ├── set.rs
│   │   │   ├── search.rs
│   │   │   └── sync.rs
│   │   ├── scope.rs        # MemoryScope
│   │   ├── weekly.rs       # WeeklyArchivedMemory
│   │   └── backends/       # 后端实现（可插拔）
│   │       ├── sqlite.rs
│   │       └── mock.rs
│   │
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── db.rs           # 数据库连接管理
│   │   ├── schema.rs       # Schema 定义
│   │   └── migrations.rs   # 迁移脚本
│   │
│   ├── vector/
│   │   ├── mod.rs
│   │   ├── traits.rs       # VectorIndex trait（新增）
│   │   ├── storage.rs      # VectorStorage
│   │   └── backends/
│   │       ├── sqlite_vec.rs
│   │       └── flat.rs     # 暴力搜索（测试用）
│   │
│   ├── network/
│   │   ├── mod.rs
│   │   ├── traits.rs       # Transport trait（新增）
│   │   ├── manager.rs      # NetworkManager（精简）
│   │   ├── ops/            # 操作拆分
│   │   │   ├── send.rs
│   │   │   ├── receive.rs
│   │   │   └── broadcast.rs
│   │   ├── p2p/            # P2P 实现
│   │   │   ├── quic.rs
│   │   │   └── discovery.rs
│   │   └── matrix/         # Matrix 联邦
│   │       └── client.rs
│   │
│   ├── security/
│   │   ├── mod.rs
│   │   ├── traits.rs       # Identity + Encryption traits（新增）
│   │   ├── identity.rs     # CisIdentity
│   │   ├── did.rs          # DID 实现
│   │   └── crypto.rs       # 加密实现
│   │
│   └── sync/
│       ├── mod.rs
│       ├── traits.rs       # Sync trait（新增）
│       ├── engine.rs       # 同步引擎
│       └── crdt.rs         # CRDT 实现
│
├── zeroclaw-compat/        # ZeroClaw 兼容层（可选 crate）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── provider/       # Provider 适配器
│       │   ├── mod.rs
│       │   ├── adapter.rs
│       │   └── factory.rs
│       ├── channel/        # Channel 适配器
│       │   ├── mod.rs
│       │   ├── adapter.rs
│       │   └── factory.rs
│       ├── skill/          # Skill 适配器
│       │   ├── mod.rs
│       │   ├── adapter.rs
│       │   └── loader.rs
│       └── tool/           # Tool 适配器
│           ├── mod.rs
│           └── adapter.rs
│
└── cis-cli/                # CLI 工具
    └── src/
        └── main.rs
```

### 2.2 Phase 2: 配置系统重构（参考 ZeroClaw）

#### 当前 CIS 配置

```rust
// 分散的配置，不够统一
pub struct CisConfig {
    pub node_id: String,
    pub data_dir: PathBuf,
    // ... 其他字段分散在各模块
}
```

#### 目标 CIS 配置（参考 ZeroClaw）

```rust
// cis-core/src/config.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// CIS 主配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CisConfig {
    /// 节点标识
    pub node_id: String,

    /// 数据目录
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    /// 记忆模块配置
    #[serde(default)]
    pub memory: MemoryConfig,

    /// 网络模块配置
    #[serde(default)]
    pub network: NetworkConfig,

    /// 安全模块配置
    #[serde(default)]
    pub security: SecurityConfig,

    /// 同步模块配置
    #[serde(default)]
    pub sync: SyncConfig,

    /// ZeroClaw 兼容配置（可选）
    #[serde(default)]
    pub zeroclaw: Option<ZeroclawConfig>,
}

/// 记忆模块配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 后端类型
    #[serde(default = "default_memory_backend")]
    pub backend: String,

    /// 向量维度
    #[serde(default = "default_vector_dimensions")]
    pub vector_dimensions: usize,

    /// 最大记忆条目数
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,

    /// 归档配置
    #[serde(default)]
    pub archive: ArchiveConfig,
}

/// ZeroClaw 兼容配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroclawConfig {
    /// 启用的提供商
    pub providers: Vec<String>,

    /// 启用的通道
    pub channels: Vec<String>,

    /// 启用的技能
    pub skills: Vec<String>,

    /// 工具配置
    #[serde(default)]
    pub tools: ToolConfig,
}

// TOML 配置示例
const DEFAULT_CONFIG: &str = r#"
# CIS 主配置
node_id = "my-workstation"
data_dir = "~/.cis"

[memory]
backend = "sqlite"
vector_dimensions = 384
max_entries = 100000

[memory.archive]
enabled = true
max_weeks = 54

[network]
transport = "quic"
listen_addr = "0.0.0.0:0"
bootstrap_nodes = []

[security]
key_type = "ed25519"
did_method = "cis"

[sync]
enabled = true
interval_seconds = 300

# ZeroClaw 兼容配置（可选）
[zeroclaw]
providers = ["openai", "anthropic"]
channels = ["telegram", "discord"]
skills = ["deploy", "git"]
"#;
```

### 2.3 Phase 3: Trait 定义（核心抽象）

```rust
// cis-core/src/memory/traits.rs

use async_trait::async_trait;

/// 记忆后端 trait
#[async_trait]
pub trait MemoryBackend: Send + Sync {
    /// 后端名称
    fn name(&self) -> &str;

    /// 存储记忆
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain) -> Result<()>;

    /// 获取记忆
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;

    /// 删除记忆
    async fn delete(&self, key: &str) -> Result<bool>;

    /// 语义搜索
    async fn search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>>;

    /// 列出键
    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>>;
}

/// 向量索引 trait
#[async_trait]
pub trait VectorIndex: Send + Sync {
    /// 索引名称
    fn name(&self) -> &str;

    /// 添加向量
    async fn add(&self, key: &str, vector: &[f32]) -> Result<()>;

    /// 搜索相似向量
    async fn search(&self, query: &[f32], limit: usize) -> Result<Vec<VectorSearchResult>>;

    /// 删除向量
    async fn remove(&self, key: &str) -> Result<()>;
}

/// 传输层 trait
#[async_trait]
pub trait Transport: Send + Sync {
    /// 传输层名称
    fn name(&self) -> &str;

    /// 发送数据
    async fn send(&self, target: &NodeId, data: &[u8]) -> Result<()>;

    /// 接收数据
    async fn receive(&self) -> Result<(NodeId, Vec<u8>)>;

    /// 广播
    async fn broadcast(&self, data: &[u8]) -> Result<usize>;

    /// 本地地址
    fn local_addr(&self) -> String;
}

/// 身份 trait
#[async_trait]
pub trait Identity: Send + Sync {
    /// DID
    fn did(&self) -> &str;

    /// 签名数据
    async fn sign(&self, data: &[u8]) -> Result<Vec<u8>>;

    /// 验证签名
    async fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool>;
}
```

---

## 3. ZeroClaw 兼容层设计

### 3.1 Provider 适配器

```rust
// zeroclaw-compat/src/provider/adapter.rs

use cis_core::memory::MemoryService;
use zeroclaw::providers::Provider;

/// CIS Memory 作为 ZeroClaw Provider 的后端
pub struct CisProviderBackend {
    memory: Arc<MemoryService>,
    provider: Box<dyn Provider>,
}

impl CisProviderBackend {
    pub async fn new(
        memory: Arc<MemoryService>,
        provider_name: &str,
        api_key: Option<&str>,
    ) -> Result<Self> {
        let provider = zeroclaw::providers::create_provider(provider_name, api_key)?;
        Ok(Self { memory, provider })
    }

    /// 执行 Agent 循环
    pub async fn run_agent_loop(&self, channel: &dyn Channel) -> Result<()> {
        // 使用 ZeroClaw 的 Agent 循环
        // 但使用 CIS Memory 作为记忆后端
        todo!()
    }
}
```

### 3.2 Channel 适配器

```rust
// zeroclaw-compat/src/channel/adapter.rs

/// ZeroClaw Channel 包装为 CIS 接口
pub struct ZcChannelAdapter {
    inner: Box<dyn zeroclaw::channels::Channel>,
    memory: Arc<MemoryService>,
}

impl ZcChannelAdapter {
    pub fn new(channel: Box<dyn zeroclaw::channels::Channel>, memory: Arc<MemoryService>) -> Self {
        Self { inner: channel, memory }
    }
}

#[async_trait]
impl cis_core::channels::Channel for ZcChannelAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    async fn send(&self, message: &Message) -> Result<()> {
        let zc_msg = zeroclaw::channels::SendMessage::new(&message.content, &message.recipient);
        self.inner.send(&zc_msg).await.map_err(|e| e.into())
    }

    async fn listen(&self, tx: mpsc::Sender<Message>) -> Result<()> {
        // 包装 ZeroClaw ChannelMessage 为 CIS Message
        todo!()
    }
}
```

---

## 4. 实施路线图

### 4.1 第一阶段：项目结构优化（2-3 周）

```
Week 1:
├── Day 1-2: 创建新的模块结构
│   ├── 创建 traits.rs 文件
│   ├── 拆分 ops/ 目录
│   └── 创建 backends/ 目录
├── Day 3-4: 重构 Memory 模块
│   ├── 定义 MemoryBackend trait
│   ├── 重构 MemoryService 使用 trait
│   └── 创建 MockMemoryBackend
└── Day 5: 测试和验证

Week 2:
├── Day 1-2: 重构 Network 模块
│   ├── 定义 Transport trait
│   └── 重构 NetworkManager
├── Day 3-4: 重构 Security 模块
│   ├── 定义 Identity trait
│   └── 重构 CisIdentity
└── Day 5: 测试和验证

Week 3:
├── Day 1-2: 重构配置系统
│   ├── 统一 CisConfig
│   └── TOML 序列化/反序列化
├── Day 3-4: 重构错误处理
│   ├── 统一 CisError
│   └── 错误链追踪
└── Day 5: 文档更新
```

### 4.2 第二阶段：ZeroClaw 兼容层（2-3 周）

```
Week 4:
├── Day 1-2: 创建 zeroclaw-compat crate
├── Day 3-4: 实现 Provider 适配器
└── Day 5: 测试 Provider 适配

Week 5:
├── Day 1-2: 实现 Channel 适配器
├── Day 3-4: 实现 Skill 适配器
└── Day 5: 测试 Channel/Skill 适配

Week 6:
├── Day 1-2: 实现 Tool 适配器
├── Day 3-4: 集成测试
└── Day 5: 文档和示例
```

### 4.3 第三阶段：功能增强（2-3 周）

```
Week 7-8:
├── 混合搜索（向量 + FTS5）
├── 性能优化
└── 监控和日志

Week 9:
├── 完善文档
├── 编写教程
└── 发布准备
```

---

## 5. 配置示例

### 5.1 独立模式（默认）

```toml
# ~/.cis/config.toml
node_id = "my-workstation"
data_dir = "~/.cis"

[memory]
backend = "sqlite"
vector_dimensions = 384

[network]
transport = "quic"
```

### 5.2 ZeroClaw 兼容模式

```toml
# ~/.cis/config.toml
node_id = "my-workstation"
data_dir = "~/.cis"

[memory]
backend = "sqlite"

# ZeroClaw 兼容配置
[zeroclaw]
providers = ["openai", "anthropic"]
channels = ["telegram", "discord"]
skills = ["deploy", "git"]

[[zeroclaw.tools]]
name = "shell"
enabled = true

[[zeroclaw.tools]]
name = "file_read"
enabled = true
```

### 5.3 启动命令

```bash
# 独立模式
cis agent start

# ZeroClaw 兼容模式
cis agent start --mode zeroclaw --channel telegram --provider openai

# 混合模式（CIS 核心 + ZeroClaw 通道）
cis agent start --channel telegram  # 使用 ZeroClaw Telegram 适配器
```

---

## 6. 关键设计决策

### 6.1 决策矩阵

| 决策 | 选项 A | 选项 B | 选择 | 理由 |
|------|--------|--------|------|------|
| Trait vs 泛型 | Trait object | 泛型 | **Trait** | 运行时灵活性 |
| 配置格式 | TOML | YAML/JSON | **TOML** | Rust 生态标准 |
| 错误处理 | thiserror | anyhow | **thiserror** | 结构化错误 |
| 异步运行时 | Tokio | async-std | **Tokio** | 生态成熟 |
| 序列化 | serde | 手动 | **serde** | 标准方案 |

### 6.2 向后兼容

```rust
// 保持旧 API 作为 deprecated 别名
#[deprecated(since = "0.2.0", note = "Use MemoryBackend trait instead")]
pub type MemoryService = Arc<dyn MemoryBackend>;

// 新旧 API 共存一个版本周期
pub mod legacy {
    pub use crate::memory::service::MemoryService;
}

pub mod new {
    pub use crate::memory::traits::MemoryBackend;
}
```

---

## 7. 成功标准

### 7.1 技术指标

| 指标 | 当前 | 目标 | 验证方式 |
|------|------|------|---------|
| 代码行数 | ~166,000 | ~120,000 | `find . -name "*.rs" -exec wc -l {} +` |
| 模块数量 | 15 | 10 | 目录结构 |
| 测试覆盖率 | 65% | 80% | `cargo tarpaulin` |
| 文档覆盖率 | 30% | 70% | `cargo doc` |
| 编译时间 | 60s | 45s | `cargo build --release` |

### 7.2 功能指标

| 功能 | 当前 | 目标 |
|------|------|------|
| 独立模式 | ✅ | ✅ 保持 |
| ZeroClaw 兼容 | ❌ | ✅ 新增 |
| 混合搜索 | ❌ | ✅ 新增 |
| 配置热加载 | ❌ | ✅ 新增 |
| 插件系统 | ❌ | ✅ 新增 |

---

## 8. 总结

### 核心价值

```
┌─────────────────────────────────────────────────────────────┐
│                    重构核心价值                              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ✅ CIS 保持独立演进                                         │
│     • 私人项目，完全可控                                     │
│     • 核心差异化能力保留                                     │
│                                                             │
│  ✅ 吸收 ZeroClaw 优秀设计                                   │
│     • 项目结构更清晰                                         │
│     • 配置系统更统一                                         │
│     • 模块拆分更合理                                         │
│                                                             │
│  ✅ 可选的 ZeroClaw 兼容                                     │
│     • 按需启用，不影响核心                                   │
│     • 获得 22+ 提供商、13+ 通道                              │
│     • 复用 Skill 生态                                        │
│                                                             │
│  ✅ 长期可维护性                                             │
│     • Trait 抽象，易于测试                                   │
│     • 模块化设计，易于扩展                                   │
│     • 代码量减少 ~30%                                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 下一步行动

1. **今天**：创建重构分支 `refactor/traits`
2. **本周**：完成 Phase 1 设计文档
3. **下周**：开始 Memory 模块重构
4. **持续**：每周 review，调整计划
