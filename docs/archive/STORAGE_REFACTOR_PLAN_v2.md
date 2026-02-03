# SQLite 存储架构重构计划 v2

## 核心原则

**物理分离 + 运行时挂载 = 解耦热插拔 + 跨库 JOIN**

```
物理文件（独立 WAL）:
~/.cis/
├── core.db                   # 核心数据库（Matrix、DID、公域记忆）
│   ├── core.db-wal          # 核心库 WAL
│   └── core.db-shm          # 共享内存
├── memory.db                 # 记忆数据库（私域/公域分离存储）
│   ├── memory.db-wal
│   └── memory.db-shm
├── federation.db             # 邦联通信关系
│   ├── federation.db-wal
│   └── federation.db-shm
└── skills/
    ├── im.db                 # IM Skill 独立库
    │   ├── im.db-wal
    │   └── im.db-shm
    ├── ai-executor.db        # AI Executor 独立库
    └── ...
```

**运行时 ATTACH 挂载**:
```sql
-- 主连接打开 core.db
ATTACH DATABASE 'memory.db' AS memory;
ATTACH DATABASE 'federation.db' AS federation;
ATTACH DATABASE 'skills/im.db' AS skill_im;

-- 跨库 JOIN 查询
SELECT * FROM matrix_events 
JOIN memory.entries ON ...
WHERE matrix_events.room_id = '!im:local';

-- Skill 卸载时 DETACH
DETACH DATABASE skill_im;
```

## 修改点清单

### 1. WAL 模式配置 (storage/wal.rs)
- [ ] `WALConfig` 结构体定义
- [ ] `set_wal_mode(conn)` 函数
- [ ] 配置参数：
  - `journal_mode = WAL`
  - `synchronous = NORMAL`（每秒 fsync）
  - `wal_autocheckpoint = 1000`
  - `journal_size_limit = 100MB`
  - `busy_timeout = 5000`

### 2. 多库连接管理 (storage/connection.rs)
- [ ] `MultiDbConnection` 结构体
- [ ] 主连接（core.db）
- [ ] `attach(db_path, alias)` 方法
- [ ] `detach(alias)` 方法
- [ ] 连接池管理

### 3. 随时关机安全 (storage/safety.rs)
- [ ] 启动时检查所有库的 WAL 并恢复
- [ ] 优雅关机 SIGTERM handler
- [ ] `PRAGMA wal_checkpoint(TRUNCATE)`
- [ ] 定期自动 checkpoint（每 5 分钟）

### 4. 记忆数据库独立 (storage/memory_db.rs)
- [ ] `MemoryDb` 独立结构体
- [ ] 私域记忆表（加密存储）
- [ ] 公域记忆表（可联邦同步）
- [ ] 记忆索引表

### 5. 邦联数据库独立 (storage/federation_db.rs)
- [ ] `FederationDb` 独立结构体
- [ ] peers 表（节点状态）
- [ ] trust 表（DID 信任网络）
- [ ] pending_sync 表（断线同步队列）

### 6. Skill 热插拔改造 (skill/db.rs)
- [ ] Skill 加载时 ATTACH
- [ ] Skill 卸载时 DETACH
- [ ] Skill 独立 WAL 配置
- [ ] 跨 Skill 查询支持

## 并发执行计划

### 任务 A: WAL 模式 + 随时关机
- storage/wal.rs
- storage/safety.rs
- 更新 DbManager 初始化逻辑

### 任务 B: 多库连接管理
- storage/connection.rs
- MultiDbConnection 实现
- attach/detach 机制

### 任务 C: 记忆库独立
- storage/memory_db.rs
- 从 core.db 分离记忆表
- 私域/公域存储优化

### 任务 D: 邦联库独立
- storage/federation_db.rs
- peers/trust/pending_sync 表
- WebSocket 联邦集成
