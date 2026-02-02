# MATRIX.md vs MATRIX-mini.md 对比分析

## 版本差异概览

| 维度 | MATRIX.md (v2.0) | MATRIX-mini.md (v3.0-PROTOCOL) |
|------|------------------|-------------------------------|
| **核心定位** | CIS = Matrix Homeserver (内嵌) | CIS = Matrix 协议实现体 |
| **实现范围** | 完整架构（4 Phases） | 最小可行集（MVP） |
| **技术选型** | 可自定义实现 | **强制使用 `ruma`** |
| **6767 联邦** | 必须实现 | **可选** |
| **工作量** | 4-6 周 | **2-3 周** |

---

## 1. 核心定位差异

### MATRIX.md (v2.0)
```rust
// CIS 节点 = 轻量级 Matrix Homeserver（库内嵌）
pub struct CisNode {
    hmi: HmiPort,      // 7676
    bmi: BmiPort,      // 6767: 必须实现 Federation
    core: CisCore,
    store: SqliteStore
}
```

**特点**:
- 强调完整的 Homeserver 实现
- EMS (Embedded Matrix Server) 作为独立组件
- Bridge 层做 CIS-Matrix 转换
- 6767 Federation 是核心架构部分

### MATRIX-mini.md (v3.0)
```rust
// 强制使用 ruma 类型系统
use ruma::{
    events::room::message::MessageEventContent,
    EventId, RoomId, UserId,
};

// 7676: 严格遵循 Matrix C-S API
// 6767: 可选，数据格式仍用 ruma 事件
```

**特点**:
- **"零外生依赖"** - 复用 Matrix 生态
- **强制 `ruma`** - 禁止手写 JSON 模拟 Matrix 事件
- **7676 严格标准** - 任何 Matrix 客户端可连接
- **6767 可选** - 可关闭或作为 CIS 扩展

---

## 2. 技术规范差异

### 数据格式

| 规范 | MATRIX.md | MATRIX-mini.md |
|------|-----------|----------------|
| 事件定义 | 可自行实现 | **必须从 `ruma::events` 导入** |
| JSON 结构 | 参考 Matrix 规范 | **强制 `ruma` 序列化** |
| ID 格式 | 建议遵循 | **强制 `ruma::identifiers`** |
| API 响应 | 手写或部分标准 | **强制 `ruma::api`** |

**关键约束** (MATRIX-mini):
> "所有事件必须经过 `ruma` 类型系统，禁止手写 JSON 模拟 Matrix 事件"

### 存储 Schema

**MATRIX.md**:
```sql
-- 自定义 schema
CREATE TABLE matrix_events (
    event_id TEXT PRIMARY KEY,
    content BLOB,  -- msgpack或json
    stream_ordering INTEGER AUTOINCREMENT
);

CREATE TABLE cis_tasks (
    task_id TEXT PRIMARY KEY,
    matrix_event_id TEXT REFERENCES matrix_events(event_id)
);
```

**MATRIX-mini.md**:
```sql
-- 标准 Matrix JSON
CREATE TABLE matrix_events (
    event_id TEXT PRIMARY KEY,  -- $xxx 标准格式
    content JSONB NOT NULL,     -- 必须是合法 Matrix JSON
    origin_server_ts INTEGER,   -- Matrix 标准时间戳
    unsigned JSONB              -- Matrix 标准 unsigned
);

-- CIS 扩展（不破坏兼容性）
CREATE TABLE cis_event_meta (
    event_id TEXT PRIMARY KEY,
    is_cis_generated BOOLEAN,
    task_ref TEXT
);
```

---

## 3. API 实现差异

### 必须实现端点

**MATRIX.md** (完整 4 Phase):
```
Phase 1: 7676 骨架
- versions, login, sync, createRoom, send

Phase 2: CIS 集成
- Bridge 层 !skill 指令

Phase 3: 6767 联邦
- key/v2/server, federation/v1/send (必须)

Phase 4: CIS 扩展
- /_cis/v1/task/forward
```

**MATRIX-mini.md** (MVP 3 Phase):
```
Phase 0 (Week 1): 协议骨架 - Element 能登录
P0: versions, login, sync, createRoom, send
P1: logout, join, messages, keys/upload
P2: E2EE 加密

Phase 1 (Week 2): 消息管道 - CIS 驱动
- m.room.message 事件处理
- CIS 结果回写 Matrix

Phase 2 (Week 3): 节点互联 - 可选
- 方案 A: 标准 Federation (重)
- 方案 B: 简化 HTTP + Matrix 事件 (轻，推荐)
```

### 6767 端口策略

| 维度 | MATRIX.md | MATRIX-mini.md |
|------|-----------|----------------|
| 必要性 | **必须** | **可选** |
| 协议 | Federation API | 可选简化方案 |
| 认证 | mTLS + Ed25519 | 可简化 |
| 数据格式 | Matrix PDU | `ruma::events` |

**MATRIX-mini 推荐**:
> "Phase 2 先用方案 B，数据标准化即可，协议可简化"

---

## 4. Phase 划分对比

### 时间线对比

```
MATRIX.md (4-6 周):
Week 1: 7676 骨架
Week 2: CIS 集成
Week 3: 6767 联邦（重）
Week 4: CIS 扩展

MATRIX-mini.md (2-3 周):
Week 1: Phase 0 - Element 登录（MVP）
Week 2: Phase 1 - 消息管道（可用）
Week 3: Phase 2 - 节点互联（可选）
```

### 验收标准对比

**MATRIX.md Phase 1**:
```bash
curl -k https://localhost:7676/_matrix/client/versions  # 200
# Element 配置后能登录并发送文字
```

**MATRIX-mini.md Phase 0**:
```bash
# 更严格：使用 ruma 类型
GET /_matrix/client/versions
# 返回 ruma::api::client::discovery::get_supported_versions::Response

# Element 完成登录握手（发现 + 登录）
```

---

## 5. 客户端接入对比

### 配置方式

**MATRIX.md**:
```
Element 配置 https://localhost:7676 后能登录
（可能需要 .well-known 或手动配置）
```

**MATRIX-mini.md** (标准化):
```
Cinny:
  Homeserver URL: http://192.168.1.50:7676
  （自动请求 /.well-known/matrix/client）

Element:
  服务器地址: http://kitchen.local:7676
  登录方式: 密码登录（简化版）或 Token 登录
```

---

## 6. 关键优势对比

### MATRIX.md 优势
- 架构描述更完整
- Bridge 层设计详细
- 联邦功能更完善

### MATRIX-mini.md 优势
- **实现更快** (2-3 周 vs 4-6 周)
- **强制标准化** (ruma 类型系统)
- **风险更低** (使用成熟库)
- **可选联邦** (不阻塞核心功能)
- **数据兼容** (标准 Matrix JSON)

---

## 7. 推荐结论

### 建议采用 **MATRIX-mini.md** 作为执行规范

**理由**:

1. **更快交付**
   - Phase 0 (1周): Element 可登录
   - Phase 1 (1周): CIS 可驱动
   - Phase 2 (可选): 节点互联

2. **技术债务更低**
   - 强制 `ruma` 避免自定义格式
   - 与 Matrix 生态 100% 兼容
   - 存储可被标准客户端读取

3. **灵活性更高**
   - 6767 可选，可后期添加
   - Phase 2 可用简化方案
   - 不阻塞 CIS 核心功能

4. **维护成本更低**
   - `ruma` 库维护 Matrix 标准
   - 无需自己解析 Matrix JSON
   - 类型安全

### 整合路线图

```
立即开始 (并行):
├─ Claude: Native IM Skill (验证 CIS Skill 接口)
├─ CIS Core: Phase 0 (7676 MVP)
└─ CIS Core: Phase 1 (CIS-Matrix 桥接)

后续 (可选):
└─ Phase 2: 6767 节点互联 (CIS 联邦)
```

---

## 8. 对 CIS 架构的影响

### 当前 CIS 架构 + MATRIX-mini

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS-MATRIX Node                          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                 Matrix Layer (Phase 0-1)             │   │
│  │  ┌─────────────┐  ┌─────────────────────────────┐   │   │
│  │  │  7676 HMI   │  │  6767 BMI (Phase 2 可选)    │   │   │
│  │  │  (ruma/api) │  │  (ruma/events, 可简化传输)  │   │   │
│  │  └──────┬──────┘  └─────────────────────────────┘   │   │
│  │         │                                          │   │
│  │         └──────┬──────────────────┐                │   │
│  │                │  CIS-Matrix Bridge│◄── 消息转换     │   │
│  │                └──────────────────┘                │   │
│  └────────────────────────────────────────────────────┘   │
│                           │                                 │
│  ┌────────────────────────┼──────────────────────────────┐ │
│  │                      CIS Core                         │ │
│  │  ┌─────────────┐  ┌────┴────┐  ┌─────────────┐       │ │
│  │  │   Skills    │  │  Memory │  │   Agent     │       │ │
│  │  │ (IM Skill)  │  │Service  │  │  Provider   │       │ │
│  │  └─────────────┘  └─────────┘  └─────────────┘       │ │
│  └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**关键整合点**:
- `ruma::events` 作为 CIS 内部事件标准
- Matrix 房间 = CIS 任务频道
- Matrix 消息 = CIS Skill 触发器
- SQLite 存储标准 Matrix JSON
