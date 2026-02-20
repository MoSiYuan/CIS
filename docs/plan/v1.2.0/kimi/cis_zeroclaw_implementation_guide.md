# CIS 作为 ZeroClaw 插件 - 实用实施指南

## 📋 核心共识

> "ZeroClaw 解决 IM、Agent、Skill，CIS 专注于工具系统" —— 非常务实的决策

---

## 1. 能力边界划分

### 1.1 ZeroClaw 负责（不复刻）

```
┌─────────────────────────────────────────────────────────────┐
│                    ZeroClaw 负责领域                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ✅ Agent 核心                                               │
│     ├── AgentBuilder (构建、配置)                            │
│     ├── Agent Loop (推理循环)                                │
│     └── Session 管理                                         │
│                                                             │
│  ✅ 通道系统 (13+)                                           │
│     ├── Telegram / Discord / Slack                          │
│     ├── Matrix / Signal / WhatsApp                          │
│     ├── Email / Webhook / CLI                               │
│     └── Lark / DingTalk / QQ                                │
│                                                             │
│  ✅ AI 提供商 (22+)                                          │
│     ├── OpenAI / Anthropic / Gemini                         │
│     ├── Groq / Mistral / xAI / DeepSeek                     │
│     ├── 智谱 GLM / Moonshot / MiniMax                       │
│     └── Ollama / Llama.cpp (本地)                           │
│                                                             │
│  ✅ Skill 系统                                               │
│     ├── SkillForge (发现、评估、集成)                        │
│     ├── SKILL.toml / SKILL.md 格式                          │
│     └── open-skills 社区生态                                 │
│                                                             │
│  ✅ 基础工具 (20+)                                           │
│     ├── shell / file_read / file_write                      │
│     ├── memory_store / memory_recall                        │
│     ├── browser / web_search / http_request                 │
│     └── cron / git / hardware                               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 CIS 负责（核心差异化）

```
┌─────────────────────────────────────────────────────────────┐
│                    CIS 负责领域                              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ✅ 记忆系统（替换 ZeroClaw Memory）                         │
│     ├── sqlite-vec 向量索引（O(log N)）                      │
│     ├── 私域/公域分离                                        │
│     ├── 54周归档 + 精准索引                                  │
│     └── 可选：混合搜索（向量 + FTS5）                        │
│                                                             │
│  ✅ 网络系统（替换/扩展 ZeroClaw Channel）                   │
│     ├── P2P/QUIC 节点通信（0 Token）                         │
│     ├── DID 身份 + 硬件绑定                                  │
│     └── Matrix Room 联邦                                     │
│                                                             │
│  ✅ 安全系统（替换 ZeroClaw Security）                       │
│     ├── DID 身份系统                                         │
│     ├── ChaCha20-Poly1305 + Argon2id                         │
│     └── ACL 白名单                                           │
│                                                             │
│  ✅ 同步系统（新增）                                         │
│     ├── 公域记忆 P2P 同步                                    │
│     ├── CRDT 冲突解决                                        │
│     └── Merkle DAG 版本控制                                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 插件接口设计

### 2.1 Memory Backend 接口

```rust
// zeroclaw-cis-memory/src/lib.rs

use async_trait::async_trait;
use zeroclaw::memory::{Memory, MemoryEntry, MemoryCategory};
use cis_core::memory::{MemoryService, MemoryDomain, MemoryCategory as CisCategory};

pub struct CisMemoryBackend {
    service: MemoryService,
    node_id: String,
}

impl CisMemoryBackend {
    pub async fn open(node_id: &str, data_dir: &Path) -> Result<Self> {
        let service = MemoryService::open(node_id, data_dir).await?;
        Ok(Self {
            service,
            node_id: node_id.to_string(),
        })
    }
}

#[async_trait]
impl Memory for CisMemoryBackend {
    fn name(&self) -> &str {
        "cis-memory"
    }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        // 映射 ZeroClaw category 到 CIS domain
        let domain = match category {
            MemoryCategory::Core | MemoryCategory::Private => MemoryDomain::Private,
            _ => MemoryDomain::Public,
        };

        let cis_category = map_category(category);

        self.service.set(key, content.as_bytes(), domain, cis_category).await
            .map_err(|e| anyhow::anyhow!("CIS store failed: {}", e))
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // 使用 CIS 语义搜索
        let results = self.service.semantic_search(query, limit, 0.6).await
            .map_err(|e| anyhow::anyhow!("CIS search failed: {}", e))?;

        Ok(results.into_iter().map(|r| r.into()).collect())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
        // CIS get 实现
        todo!()
    }

    async fn forget(&self, key: &str) -> anyhow::Result<bool> {
        // CIS delete 实现
        todo!()
    }

    async fn health_check(&self) -> bool {
        self.service.health_check().await
    }
}
```

### 2.2 Network Backend 接口

```rust
// zeroclaw-cis-network/src/lib.rs

use async_trait::async_trait;
use zeroclaw::channels::{Channel, ChannelMessage, SendMessage};
use cis_core::network::{NetworkManager, DidIdentity};

/// CIS P2P 网络作为 ZeroClaw Channel
pub struct CisP2PChannel {
    network: NetworkManager,
    identity: DidIdentity,
}

impl CisP2PChannel {
    pub async fn new(node_id: &str) -> Result<Self> {
        let identity = DidIdentity::generate_or_load(node_id).await?;
        let network = NetworkManager::new(identity.did().to_string()).await?;
        Ok(Self { network, identity })
    }
}

#[async_trait]
impl Channel for CisP2PChannel {
    fn name(&self) -> &str {
        "cis-p2p"
    }

    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        // 发送到 P2P 网络
        let payload = serde_json::json!({
            "type": "message",
            "content": message.content,
            "recipient": message.recipient,
        });

        self.network.broadcast(payload.to_string().as_bytes()).await
            .map_err(|e| anyhow::anyhow!("P2P send failed: {}", e))
    }

    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        // 监听 P2P 消息并转换为 ChannelMessage
        todo!()
    }

    async fn health_check(&self) -> bool {
        self.network.health_check().await
    }
}
```

---

## 3. 配置示例

### 3.1 ZeroClaw 配置（使用 CIS 插件）

```toml
# ~/.zeroclaw/config.toml

[agent]
name = "my-agent"
provider = "anthropic"
model = "claude-3-5-sonnet"

# 使用 CIS Memory Backend
[memory]
backend = "cis"

[memory.cis]
node_id = "my-workstation"
data_dir = "~/.cis/data"
vector_dimensions = 384

# 使用 CIS P2P Network Backend
[network]
backend = "cis-p2p"

[network.cis]
listen_addr = "0.0.0.0:0"  # 自动选择端口
bootstrap_nodes = [
    "did:cis:AbCdEf...",
]

# 使用 CIS Security Backend
[security]
backend = "cis"

[security.cis]
did_method = "cis"
key_type = "ed25519"
```

### 3.2 启动命令

```bash
# 使用 CIS 后端的 ZeroClaw
zeroclaw channel start telegram     --memory-backend cis     --network-backend cis-p2p     --security-backend cis
```

---

## 4. 开发计划

### 4.1 Phase 1: Memory Backend（1-2 周）

```
Week 1:
├── Day 1-2: 定义 zeroclaw::memory::Memory trait 扩展
├── Day 3-4: 实现 CisMemoryBackend
└── Day 5: 集成测试

Week 2:
├── Day 1-2: 性能基准测试（对比 ZeroClaw 原生）
├── Day 3-4: 混合搜索实现（向量 + FTS5）
└── Day 5: 文档和示例
```

### 4.2 Phase 2: Network Backend（2-3 周）

```
Week 3-4:
├── 实现 CisP2PChannel
├── DID 身份集成
├── P2P 消息路由
└── 与 ZeroClaw Channel 系统集成

Week 5:
├── Matrix Room 联邦
├── 跨节点记忆同步
└── 测试和文档
```

### 4.3 Phase 3: 优化（1-2 周）

```
Week 6-7:
├── 性能优化
├── 错误处理完善
├── 配置热加载
└── 发布到 crates.io
```

---

## 5. 代码复用清单

### 5.1 CIS 代码复用

| 模块 | 复用程度 | 说明 |
|------|---------|------|
| `cis-core/src/memory/` | 90% | 直接复用，包装为 trait |
| `cis-core/src/vector/` | 100% | sqlite-vec 直接复用 |
| `cis-core/src/network/` | 80% | 适配 Channel trait |
| `cis-core/src/security/` | 90% | DID + 加密直接复用 |
| `cis-core/src/sync/` | 100% | 联邦同步直接复用 |

### 5.2 ZeroClaw 代码复用

| 模块 | 复用程度 | 说明 |
|------|---------|------|
| `zeroclaw/src/agent/` | 100% | 直接使用 |
| `zeroclaw/src/channels/` | 100% | 直接使用 |
| `zeroclaw/src/providers/` | 100% | 直接使用 |
| `zeroclaw/src/skills/` | 100% | 直接使用 |
| `zeroclaw/src/tools/` | 80% | 基础工具直接使用 |

---

## 6. 预期成果

### 6.1 代码量减少

| 项目 | 当前 CIS | 整合后 | 减少 |
|------|---------|--------|------|
| 总代码行数 | ~166,000 | ~50,000 | -70% |
| 核心模块 | 15+ | 3 (memory/network/security) | -80% |
| 维护负担 | 高 | 低 | -70% |

### 6.2 功能增强

| 功能 | CIS 独立 | 整合后 |
|------|---------|--------|
| AI 提供商 | 需自建 | 22+ (ZeroClaw) |
| 通信通道 | 需自建 | 13+ (ZeroClaw) |
| Skill 生态 | 需自建 | 3000+ (OpenClaw) |
| 工具系统 | 部分 | 20+ (ZeroClaw) |
| Agent 循环 | 需自建 | 成熟 (ZeroClaw) |

---

## 7. 快速开始模板

### 7.1 项目结构

```
zeroclaw-cis/
├── Cargo.toml
├── src/
│   ├── lib.rs           # 公共接口
│   ├── memory/
│   │   ├── mod.rs       # Memory Backend
│   │   └── hybrid.rs    # 混合搜索
│   ├── network/
│   │   ├── mod.rs       # P2P Channel
│   │   └── sync.rs      # 联邦同步
│   └── security/
│       └── mod.rs       # DID Security
├── examples/
│   ├── basic.rs         # 基础使用
│   └── distributed.rs   # 分布式示例
└── tests/
    └── integration.rs   # 集成测试
```

### 7.2 Cargo.toml

```toml
[package]
name = "zeroclaw-cis"
version = "0.1.0"
edition = "2021"

[dependencies]
# ZeroClaw
coderlaw = { git = "https://github.com/zeroclaw-labs/zeroclaw" }

# CIS Core
cis-core = { path = "../cis/cis-core" }

# Async
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }

# Error handling
anyhow = "1"
thiserror = "1"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

## 8. 总结

### 核心收益

```
┌─────────────────────────────────────────────────────────────┐
│                    整合收益总结                              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ✅ 减少 70% 重复代码                                        │
│     • 不复刻 IM、Agent、Skill、Providers、Channels          │
│     • 专注 CIS 核心差异化                                    │
│                                                             │
│  ✅ 获得 ZeroClaw 完整生态                                   │
│     • 22+ AI 提供商                                          │
│     • 13+ 通信通道                                           │
│     • 3000+ Skill 生态                                       │
│                                                             │
│  ✅ 保留 CIS 核心能力                                        │
│     • P2P 联邦网络                                           │
│     • DID 身份安全                                           │
│     • 私域/公域记忆分离                                      │
│                                                             │
│  ✅ 降低长期维护成本                                         │
│     • ZeroClaw 社区维护核心                                  │
│     • 您只需维护 3 个插件模块                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 下一步行动

1. **今天**：Fork ZeroClaw，创建 `zeroclaw-cis` 仓库
2. **本周**：实现 `CisMemoryBackend` 原型
3. **下周**：集成测试，验证可行性
4. **持续**：逐步完善 network 和 security 模块
