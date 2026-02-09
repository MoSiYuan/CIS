# Agent-F 任务分配

**Agent 标识**: Agent-F  
**任务**: T4.2 + T4.3  
**技能要求**: Matrix 协议、机器学习、向量嵌入  
**优先级**: P2  
**预估总时间**: 9 小时

---

## 任务清单

### 任务 1: T4.3 - Embedding 服务替换（优先）
**文件**: `plan/tasks/T4.3_embedding_service/README.md`  
**时间**: 4h  
**状态**: 🔴 立即开始（无依赖）

**核心目标**:
- 替换所有 mock embedding 实现
- 使用真实 `fastembed` 库
- 模型: `NomicEmbedTextV15`

**需要修改的文件**:
- `cis-core/src/memory/service.rs:929`
- `cis-core/src/task/vector.rs:415-421`
- `cis-core/src/vector/storage.rs:1876-1882`
- `cis-core/src/ai/embedding.rs:380`

**关键实现**:
```rust
pub struct EmbeddingService {
    model: TextEmbedding,
}

impl EmbeddingService {
    pub async fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15)
        )?;
        Ok(Self { model })
    }
    
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.model.embed(vec![text], None)?;
        Ok(embeddings[0].clone())
    }
}
```

---

### 任务 2: T4.2 - Federation 事件发送
**文件**: `plan/tasks/T4.2_federation_events/README.md`  
**时间**: 5h  
**状态**: 🔴 等待 T2.2 完成后开始

**核心目标**:
- 实现 Agent Federation 的真实 Matrix 事件发送
- 修复 TODO: FederationClient

**需要修改的文件**:
- `cis-core/src/agent/federation/agent.rs:192,271,293,320`

**关键接口**:
```rust
impl FederationClient {
    pub async fn send_heartbeat(&self) -> Result<()>;
    pub async fn send_task_request(&self, task: &TaskRequest) -> Result<String>;
    pub async fn subscribe_events(&self, callback: impl Fn(FederationEvent)) -> Result<()>;
}
```

---

## 执行顺序

```
┌─────────────────────────────────────────────────────┐
│  1. T4.3 (4h) - 无依赖，可立即开始                   │
│     - 替换所有 mock embedding                       │
│     - 使用 fastembed                                │
│     - 模型自动下载                                  │
│     - 提交 PR                                        │
│                                                      │
│  ↓ 同时等待 T2.2 完成                                │
│                                                      │
│  2. T4.2 (5h)                                        │
│     - 实现 Federation 事件发送                      │
│     - 心跳机制                                      │
│     - 事件订阅                                      │
│     - 提交 PR                                        │
└─────────────────────────────────────────────────────┘
```

---

## T4.3 详细说明

### 当前 Mock 代码
```rust
/// 模拟 embedding service（用于测试）
/// 简单的确定性模拟：根据文本哈希生成向量
fn mock_embed(text: &str) -> Vec<f32> {
    // 这是假的！需要替换
    let hash = calculate_hash(text);
    (0..768).map(|i| ((hash + i as u64) % 100) as f32 / 100.0).collect()
}
```

### 真实实现
```rust
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

// 模型自动下载 (~130MB)
// 首次使用时会下载 Nomic Embed Text v1.5
let model = TextEmbedding::try_new(
    InitOptions::new(EmbeddingModel::NomicEmbedTextV15)
        .with_show_download_progress(true)
)?;
```

### 需要替换的位置
1. `memory/service.rs` - MemoryService::embed
2. `task/vector.rs` - VectorTask::embed
3. `vector/storage.rs` - VectorStorage::search
4. `ai/embedding.rs` - LocalEmbeddingService

---

## T4.2 详细说明

### Federation 架构
```
┌─────────────┐      Matrix      ┌─────────────┐
│   Node A    │ ◄──────────────► │   Node B    │
│  (Agent)    │    Events        │  (Agent)    │
└─────────────┘                  └─────────────┘
```

### 需要实现的事件
- **心跳**: 定期广播存活状态
- **任务请求**: 跨节点任务分发
- **任务响应**: 任务结果返回
- **状态同步**: 节点状态同步

### Matrix 事件类型
```rust
enum FederationEvent {
    Heartbeat { node_id: String, timestamp: u64 },
    TaskRequest { task_id: String, content: String },
    TaskResponse { task_id: String, result: String },
    StatusUpdate { node_id: String, status: NodeStatus },
}
```

---

## 验收标准

### T4.3 验收
- [ ] 相同文本生成相同向量
- [ ] 相似文本向量距离近
- [ ] 批处理性能 >100 texts/sec
- [ ] 模型自动下载（首次使用）
- [ ] 所有 mock 代码被删除

### T4.2 验收
- [ ] 心跳事件真实发送到 Matrix Room
- [ ] 其他节点能收到并处理
- [ ] 断线后自动重连
- [ ] 消息顺序保证

---

## 依赖关系

**依赖你的 Agent**:
- 无（T4.3 是独立任务）

**你依赖的 Agent**:
- T2.2 (Agent-C) - 提供 MatrixServerManager

**T4.3 可立即开始！**

---

## 模型下载说明

### 首次运行
```rust
let model = TextEmbedding::try_new(...)?;
// 会自动下载:
// - nomic-embed-text-v1.5.onnx (~130MB)
// - tokenizer.json
```

### 下载位置
- Linux: `~/.cache/fastembed/`
- macOS: `~/Library/Caches/fastembed/`

### 离线使用
提前下载模型到上述位置，代码会自动检测。

---

## 开始工作

### 第一步: T4.3 (立即开始)
1. 阅读: `plan/tasks/T4.3_embedding_service/README.md`
2. 创建分支: `git checkout -b agent-f/t4.3-embedding`
3. 搜索所有 mock embedding 代码
4. 统一替换为 fastembed
5. 提交 PR

### 第二步: T4.2 (等待 T2.2)
1. 等待 Agent-C 完成 T2.2
2. 阅读: `plan/tasks/T4.2_federation_events/README.md`
3. 实现 FederationClient
4. 提交 PR

---

**T4.3 可以立即开始，不依赖其他任务！**
