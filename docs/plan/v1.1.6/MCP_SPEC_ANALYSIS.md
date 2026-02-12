# MCP 规范对齐分析

> **版本**: v1.1.6
> **Team**: Team O
> **日期**: 2026-02-12
> **状态**: 规范对齐分析

## 概述

本文档分析 CIS MCP Adapter 实现与 Model Context Protocol (MCP) 规范的对齐情况，识别缺失功能，并制定完善计划。

**MCP 规范版本**: 2024-11-05 (Current)
**参考规范**: https://spec.modelcontextprotocol.io/

---

## 当前实现状态

### ✅ 已实现功能

#### 1. 基础协议
- **JSON-RPC 2.0**: ✅ 完整实现
  - 请求解析: `McpRequest`
  - 响应格式: `McpResponse`
  - 错误处理: `McpError` with 标准错误码

#### 2. 生命周期管理
- **Initialize**: ✅ 实现
  - 协议版本协商: `2024-11-05`
  - 能力声明: `ServerCapabilities`
  - 服务器信息: `ServerInfo`

- **Ping**: ✅ 实现
  - 心跳检测

#### 3. Tools 功能
- **tools/list**: ✅ 完整实现
  - 列出所有可用工具
  - 包含输入 schema (JSON Schema)

- **tools/call**: ✅ 完整实现
  - 工具调用
  - 参数传递
  - 错误处理
  - 结果格式化

#### 4. Resources 功能
- **resources/list**: ✅ 基础实现
  - 列出资源
  - URI 命名
  - MIME 类型

- **resources/read**: ✅ 基础实现
  - 读取资源内容
  - 文本格式返回

---

### ❌ 缺失功能

#### 1. Resources 完整 CRUD

**当前状态**:
- ✅ `resources/list` - 基础列表
- ✅ `resources/read` - 基础读取
- ❌ `resources/subscribe` - 订阅更新
- ❌ `resources/unsubscribe` - 取消订阅
- ❌ `resources/create` - 创建资源
- ❌ `resources/update` - 更新资源
- ❌ `resources/delete` - 删除资源
- ❌ 元数据支持: `_comment`, `annotations`, `metadata`

**MCP 规范要求**:
```json
// 资源模板
{
  "uri": "file:///path/to/file.txt",
  "name": "My File",
  "description": "Optional description",
  "mimeType": "text/plain",
  "metadata": {
    "key": "value"
  },
  "annotations": {
    "role": "user",
    "priority": 1
  }
}
```

#### 2. Prompts 管理

**当前状态**: ❌ 完全缺失

**MCP 规范要求**:
- `prompts/list` - 列出所有 prompt 模板
- `prompts/get` - 获取特定 prompt 详情
- `prompts/render` - 渲染 prompt（带参数）

**Prompt 结构**:
```json
{
  "name": "summarize_code",
  "description": "Summarize the given code",
  "arguments": [
    {
      "name": "code",
      "description": "The code to summarize",
      "required": true
    }
  ],
  "metadata": {
    "category": "code-analysis"
  }
}
```

#### 3. 高级工具功能

**当前状态**: 基础实现，缺失高级特性

**缺失功能**:
- ❌ 流式响应 (`StreamingToolCallResult`)
- ❌ 工具调用进度 (`Progress` token)
- ❌ 取消操作 (`Cancellation`)
- ❌ 工具权限控制

#### 4. 消息和上下文

**当前状态**: ❌ 未实现

**MCP 规范要求**:
- `set_level` - 设置日志级别
- `logging/list` - 列出日志消息
- 上下文传播 (Context 传递)

#### 5. 采样和限制

**当前状态**: ❌ 未实现

**MCP 规范要求**:
- `sampling/create_message` - 创建采样消息
- 速率限制 (Rate limiting)
- 配额管理 (Quota management)

#### 6. 补全功能 (Completion)

**当前状态**: ❌ 未实现

**MCP 规范要求**:
- `complete/complete` - 自动补全建议
- `CompleteResult` - 补全结果
- 支持资源、prompt、argument 补全

#### 7. 分页 (Pagination)

**当前状态**: ❌ 未实现

**MCP 规范要求**:
- `PaginationToken` - 分页标记
- `PageResult` - 分页结果
- 游标分页支持

#### 8. 服务器发现和连接

**当前状态**: ❌ 仅 stdio 模式

**MCP 规范要求**:
- ✅ STDIO 传输: 已实现
- ❌ SSE (Server-Sent Events): 未实现
- ❌ WebSocket: 未实现
- ❌ 服务器发现: 未实现
- ❌ 重连机制: 未实现

---

## 规范对齐矩阵

| 功能模块 | MCP 规范 | 当前实现 | 缺失内容 | 优先级 |
|---------|----------|----------|----------|--------|
| **Base Protocol** | | | | |
| JSON-RPC 2.0 | Required | ✅ | - | - |
| Transports | Required | ⚠️ | SSE, WebSocket | P1 |
| Initialization | Required | ✅ | - | - |
| Ping | Required | ✅ | - | - |
| **Resources** | | | | |
| List | Required | ✅ | 元数据支持 | P2 |
| Read | Required | ✅ | 二进制支持 | P2 |
| Subscribe | Optional | ❌ | 完整实现 | P1 |
| Create | Optional | ❌ | 完整实现 | P2 |
| Update | Optional | ❌ | 完整实现 | P2 |
| Delete | Optional | ❌ | 完整实现 | P2 |
| **Prompts** | | | | |
| List | Required | ❌ | 完整实现 | P0 |
| Get | Required | ❌ | 完整实现 | P0 |
| Render | Required | ❌ | 完整实现 | P0 |
| **Tools** | | | | |
| List | Required | ✅ | - | - |
| Call | Required | ✅ | 流式响应 | P1 |
| Progress | Optional | ❌ | 完整实现 | P2 |
| **Utilities** | | | | |
| Completion | Optional | ❌ | 完整实现 | P2 |
| Logging | Optional | ❌ | 完整实现 | P3 |
| Pagination | Optional | ❌ | 完整实现 | P2 |
| **Advanced** | | | | |
| Cancellation | Optional | ❌ | 完整实现 | P2 |
| Sampling | Optional | ❌ | 完整实现 | P3 |
| Roots | Optional | ❌ | 完整实现 | P3 |

**优先级说明**:
- **P0**: 核心功能，必须实现
- **P1**: 重要功能，强烈推荐
- **P2**: 增强功能，推荐实现
- **P3**: 可选功能，可延后

---

## 实现计划

### Phase 1: 核心功能补全 (3 天)

#### 1.1 Prompts 模块实现 (1 天)
- [ ] 创建 `prompts.rs` 模块
- [ ] 实现 Prompt 结构和存储
- [ ] 实现 `prompts/list`
- [ ] 实现 `prompts/get`
- [ ] 实现 `prompts/render`
- [ ] Prompt 参数验证

#### 1.2 Resources CRUD 增强 (1 天)
- [ ] 实现 `resources/create`
- [ ] 实现 `resources/update`
- [ ] 实现 `resources/delete`
- [ ] 添加元数据支持
- [ ] 添加注解支持

#### 1.3 资源订阅机制 (1 天)
- [ ] 实现 `resources/subscribe`
- [ ] 实现 `resources/unsubscribe`
- [ ] 订阅状态管理
- [ ] 变更通知机制

### Phase 2: 高级功能 (2 天)

#### 2.1 流式响应 (1 天)
- [ ] 实现流式工具调用
- [ ] SSE 流式传输
- [ ] 流式进度报告

#### 2.2 工具增强 (1 天)
- [ ] 实现进度 token
- [ ] 实现取消操作
- [ ] 工具权限控制

### Phase 3: 实用功能 (1 天)

#### 3.1 补全功能 (0.5 天)
- [ ] 实现 `complete/complete`
- [ ] 支持资源补全
- [ ] 支持 prompt 参数补全

#### 3.2 日志功能 (0.5 天)
- [ ] 实现 `logging/set_level`
- [ ] 实现 `logging/list`
- [ ] 日志消息过滤

### Phase 4: 传输层扩展 (1 天)

#### 4.1 SSE 传输 (0.5 天)
- [ ] SSE 服务器实现
- [ ] SSE 事件格式

#### 4.2 服务器发现 (0.5 天)
- [ ] mDNS 发现
- [ ] 服务器注册

---

## 技术实现细节

### 1. Prompts 存储

```rust
// Prompt 定义
pub struct Prompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<PromptArgument>,
    pub metadata: PromptMetadata,
}

pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
}

// Prompt 存储
pub struct PromptStore {
    prompts: HashMap<String, Prompt>,
    templates: HashMap<String, Template>,
}
```

### 2. Resources 订阅

```rust
pub struct ResourceSubscription {
    pub id: String,
    pub uri: String,
    pub subscriber: String,
    pub created_at: DateTime<Utc>,
}

pub struct SubscriptionManager {
    subscriptions: HashMap<String, ResourceSubscription>,
    watchers: HashMap<String, Vec<Subscriber>>,
}
```

### 3. 流式响应

```rust
pub struct StreamingToolResult {
    pub content: Vec<ContentChunk>,
    pub is_complete: bool,
    pub next_token: Option<String>,
}

pub enum ContentChunk {
    Text { delta: String },
    Data { bytes: Vec<u8> },
    Metadata { key: String, value: String },
}
```

### 4. 补全引擎

```rust
pub struct CompletionEngine {
    resources: ResourceIndex,
    prompts: PromptIndex,
    tools: ToolIndex,
}

impl CompletionEngine {
    pub async fn complete(
        &self,
        typ: CompletionType,
        query: &str,
        context: &CompletionContext
    ) -> Result<Vec<CompletionItem>>;
}
```

---

## 测试策略

### 单元测试
- 每个 MCP 方法的独立测试
- 错误处理测试
- 边界条件测试

### 集成测试
- MCP 客户端-服务器交互
- 多客户端并发测试
- 传输层切换测试

### 兼容性测试
- MCP 官方测试套件
- 第三方客户端兼容性
- 规范版本兼容性

---

## 验收标准

### 功能完整性
- [ ] 实现 MCP 规范所有 Required 功能
- [ ] 实现 80% 以上的 Optional 功能
- [ ] 通过 MCP 官方测试套件

### 代码质量
- [ ] 测试覆盖率 > 70%
- [ ] 文档完整性 > 90%
- [ ] 零 Clippy 警告

### 性能指标
- [ ] 响应时间 < 100ms (P99)
- [ ] 并发连接支持 > 100
- [ ] 内存占用 < 100MB (空闲)

---

## 风险和缓解

### 技术风险

| 风险 | 影响 | 概率 | 缓解措施 |
|-----|------|------|---------|
| SSE 传输不稳定 | 高 | 中 | 降级到轮询机制 |
| 流式响应复杂度高 | 中 | 高 | 分阶段实现 |
| 订阅管理复杂 | 中 | 中 | 使用成熟库 |
| 兼容性问题 | 低 | 低 | 严格遵循规范 |

### 实施风险

| 风险 | 影响 | 概率 | 缓解措施 |
|-----|------|------|---------|
| 时间延期 | 中 | 中 | 优先级管理 |
| 测试覆盖不足 | 高 | 低 | 提前编写测试 |
| 文档不完整 | 中 | 中 | 边开发边写文档 |

---

## 参考资源

- [MCP 官方规范](https://spec.modelcontextprotocol.io/)
- [MCP TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [MCP Python SDK](https://github.com/modelcontextprotocol/python-sdk)
- [JSON-RPC 2.0 规范](https://www.jsonrpc.org/specification)

---

## 附录

### A. MCP 传输层对比

| 传输方式 | 优点 | 缺点 | 适用场景 |
|---------|------|------|---------|
| STDIO | 简单、安全 | 单客户端 | 本地 CLI |
| SSE | 标准、简单 | 单向 | Web 应用 |
| WebSocket | 双向、高效 | 复杂 | 实时应用 |

### B. 错误码标准

```rust
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // MCP 特定
    pub const RESOURCE_NOT_FOUND: i32 = -32001;
    pub const PROMPT_NOT_FOUND: i32 = -32002;
    pub const TOOL_EXECUTION_ERROR: i32 = -32003;
    pub const SUBSCRIPTION_FAILED: i32 = -32004;
}
```

---

**文档状态**: 🟢 完成
**审核状态**: ⏳ 待审核
**下一步**: 开始实现 Phase 1 - Prompts 模块
