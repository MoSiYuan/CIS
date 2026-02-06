# Room Store 设计 - 每 Room 独立 SQLite

## 核心设计

```
┌─────────────────────────────────────────────────────────────────┐
│                     RoomStoreManager                             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  rooms: HashMap<RoomId, Arc<RoomStore>>                │   │
│  │  base_path: PathBuf (e.g., ~/.cis/rooms/)              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│         ┌────────────────────┼────────────────────┐             │
│         │                    │                    │             │
│         ▼                    ▼                    ▼             │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐      │
│  │  RoomStore  │     │  RoomStore  │     │  RoomStore  │      │
│  │  !dag-1     │     │  !dag-2     │     │  !status    │      │
│  │  ─────────  │     │  ─────────  │     │  ─────────  │      │
│  │  events.db  │     │  events.db  │     │  events.db  │      │
│  │  (SQLite)   │     │  (SQLite)   │     │  (SQLite)   │      │
│  └─────────────┘     └─────────────┘     └─────────────┘      │
└─────────────────────────────────────────────────────────────────┘

文件结构:
~/.cis/rooms/
├── !dag-abc123:cis.local/
│   ├── events.db          # 事件存储
│   ├── events.db-wal      # WAL 文件
│   └── meta.json          # Room 元数据
├── !dag-def456:cis.local/
│   ├── events.db
│   └── meta.json
└── !status-general:cis.local/
    ├── events.db
    └── meta.json
```

## RoomStore Schema

```sql
-- events 表: 存储所有 Matrix 事件
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,      -- Matrix event ID ($abc123)
    room_id TEXT NOT NULL,               -- Room ID
    sender TEXT NOT NULL,                -- 发送者 (@user:cis.local)
    event_type TEXT NOT NULL,            -- 事件类型 (m.room.message, cis.dag.*)
    content TEXT NOT NULL,               -- JSON 内容
    origin_server_ts INTEGER NOT NULL,   -- 发送时间戳 (毫秒)
    received_at INTEGER NOT NULL,        -- 接收时间戳 (毫秒)
    
    -- 索引
    UNIQUE(event_id)
);

-- 创建索引
CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_sender ON events(sender);
CREATE INDEX idx_events_timestamp ON events(origin_server_ts);
CREATE INDEX idx_events_type_timestamp ON events(event_type, origin_server_ts);

-- room_state 表: 当前状态（加速查询）
CREATE TABLE room_state (
    key TEXT PRIMARY KEY,                -- 状态键
    value TEXT NOT NULL,                 -- JSON 值
    event_id TEXT NOT NULL,              -- 来源事件 ID
    updated_at INTEGER NOT NULL          -- 更新时间
);

-- sync_positions 表: 各节点同步位置
CREATE TABLE sync_positions (
    node_id TEXT PRIMARY KEY,            -- 节点 ID
    last_event_id TEXT NOT NULL,         -- 最后同步事件
    last_timestamp INTEGER NOT NULL,     -- 最后同步时间
    updated_at INTEGER NOT NULL          -- 更新时间
);
```

## API 设计

```rust
/// Room 存储管理器
pub struct RoomStoreManager {
    /// Room ID -> RoomStore 映射
    rooms: RwLock<HashMap<RoomId, Arc<RoomStore>>>,
    /// 基础存储路径
    base_path: PathBuf,
    /// 全局配置
    config: RoomStoreConfig,
}

impl RoomStoreManager {
    /// 创建/获取 RoomStore
    pub async fn get_or_create(&self, room_id: &RoomId) -> Result<Arc<RoomStore>>;
    
    /// 获取已存在的 RoomStore
    pub async fn get(&self, room_id: &RoomId) -> Option<Arc<RoomStore>>;
    
    /// 关闭并移除 RoomStore
    pub async fn close_room(&self, room_id: &RoomId) -> Result<()>;
    
    /// 列出所有 rooms
    pub async fn list_rooms(&self) -> Vec<RoomInfo>;
    
    /// 归档 room（关闭并移动到归档目录）
    pub async fn archive_room(&self, room_id: &RoomId) -> Result<()>;
}

/// 单个 Room 的存储
pub struct RoomStore {
    room_id: RoomId,
    db: Arc<Mutex<Connection>>,
    path: PathBuf,
    event_tx: broadcast::Sender<RoomEvent>,
}

impl RoomStore {
    /// 存储事件
    pub async fn store_event(&self, event: &MatrixEvent) -> Result<EventId>;
    
    /// 批量存储事件
    pub async fn store_events(&self, events: &[MatrixEvent]) -> Result<Vec<EventId>>;
    
    /// 查询事件（分页）
    pub async fn query_events(
        &self,
        filter: EventFilter,
        pagination: Pagination,
    ) -> Result<PaginatedEvents>;
    
    /// 获取事件流（实时订阅）
    pub fn subscribe_events(&self) -> broadcast::Receiver<RoomEvent>;
    
    /// 获取历史事件（用于同步）
    pub async fn get_events_since(
        &self,
        since_event_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MatrixEvent>>;
    
    /// 获取最新 N 条事件
    pub async fn get_latest_events(&self, n: usize) -> Result<Vec<MatrixEvent>>;
    
    /// 统计信息
    pub async fn stats(&self) -> Result<RoomStats>;
    
    /// 导出到 JSONL
    pub async fn export_to_jsonl(&self, path: &Path) -> Result<()>;
    
    /// 从 JSONL 导入
    pub async fn import_from_jsonl(&self, path: &Path) -> Result<()>;
}

/// 事件过滤器
pub struct EventFilter {
    pub event_types: Option<Vec<MatrixEventType>>,
    pub senders: Option<Vec<String>>,
    pub since: Option<i64>,  // timestamp
    pub until: Option<i64>,
    pub contains: Option<String>,  // content 包含文本
}

/// 分页参数
pub struct Pagination {
    pub limit: usize,
    pub before: Option<String>,  // event_id
    pub after: Option<String>,
}

/// Room 统计
pub struct RoomStats {
    pub total_events: usize,
    pub earliest_event: Option<i64>,
    pub latest_event: Option<i64>,
    pub event_type_counts: HashMap<String, usize>,
    pub db_size_bytes: usize,
}
```

## 使用场景

### 场景 1: DAG 执行事件存储

```rust
// Executor 发送事件
let room_store = room_manager.get_or_create(&room_id).await?;

// 存储 DAG 运行事件
room_store.store_event(&MatrixEvent {
    event_type: "cis.dag.run.created".to_string(),
    content: json!({
        "run_id": "dag-run-abc123",
        "dag_name": "refactor",
    }),
    ..Default::default()
}).await?;

// 查询历史
let events = room_store
    .query_events(
        EventFilter::by_type(MatrixEventType::CisDagTaskStatus),
        Pagination::new().limit(50),
    )
    .await?;
```

### 场景 2: 新节点加入同步

```rust
// 新节点加入 room，请求历史事件
let room_store = room_manager.get_or_create(&room_id).await?;

// 从已知位置同步
let since = sync_positions.get(&room_id, &my_node_id).await?;
let events = room_store.get_events_since(since.as_deref(), 1000).await?;

// 处理事件
for event in events {
    apply_event(event).await?;
}

// 更新同步位置
sync_positions.update(&room_id, &my_node_id, &events.last().event_id).await?;
```

### 场景 3: 归档旧 Room

```rust
// DAG 运行完成，归档 room
room_manager.archive_room(&room_id).await?;

// 文件移动到 ~/.cis/rooms/archived/
// 释放内存中的 RoomStore
```

## 性能优化

### 1. WAL 模式
```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
```

### 2. 批量写入
```rust
// 批量插入事件
pub async fn store_events_batch(&self, events: &[MatrixEvent]) -> Result<()> {
    let mut conn = self.db.lock().await;
    let tx = conn.transaction()?;
    
    for event in events {
        tx.execute(
            "INSERT INTO events (...) VALUES (...)",
            params![...],
        )?;
    }
    
    tx.commit()?;
    Ok(())
}
```

### 3. 连接池
每个 RoomStore 使用独立连接，避免锁竞争。

### 4. 热 Room 缓存
```rust
pub struct RoomStoreManager {
    rooms: RwLock<HashMap<RoomId, Arc<RoomStore>>>,
    hot_rooms: LruCache<RoomId, Arc<RoomStore>>,  // 最近使用的 Room
}
```

## 与现有 Matrix 模块集成

```rust
// MatrixNucleus 集成 RoomStore
pub struct MatrixNucleus {
    // ... existing fields ...
    room_store_manager: Arc<RoomStoreManager>,
}

impl MatrixNucleus {
    /// 发送事件时自动存储
    pub async fn send_event(&self, room_id: &RoomId, event: MatrixEvent) -> Result<()> {
        // 1. 存储到本地 RoomStore
        let store = self.room_store_manager.get_or_create(room_id).await?;
        store.store_event(&event).await?;
        
        // 2. 联邦广播
        self.broadcaster.broadcast_event(room_id, &event).await?;
        
        Ok(())
    }
    
    /// 接收联邦事件时存储
    pub async fn receive_federated_event(&self, event: MatrixEvent) -> Result<()> {
        let store = self.room_store_manager
            .get_or_create(&RoomId::new(&event.room_id))
            .await?;
        store.store_event(&event).await?;
        
        // 触发本地处理
        self.handle_event(event).await?;
        
        Ok(())
    }
}
```

## 文件实现列表

| 文件 | 行数 | 功能 |
|------|------|------|
| `storage/room_store.rs` | ~400 | RoomStore 核心实现 |
| `storage/room_manager.rs` | ~300 | RoomStoreManager 管理多个 room |
| `storage/room_types.rs` | ~150 | 类型定义 (Filter, Pagination, Stats) |
| `storage/mod.rs` | ~50 | 模块导出 |
| **总计** | **~900** | |

## 下一步

1. 创建 `storage/room_store.rs` 基础结构
2. 实现 Schema 初始化
3. 实现 store_event / query_events
4. 集成到 MatrixNucleus
5. 添加归档功能
