# CIS 存储与目录设计

## 设计原则

1. **数据隔离**: 核心记忆与 Skill 数据严格分离
2. **热插拔**: Skill 可独立加载/卸载，不影响核心
3. **跨平台**: 支持 macOS, Linux, Windows
4. **向后兼容**: 支持数据迁移和版本升级

---

## 目录结构

### 基础目录 (Base Directory)

| 平台 | 路径 |
|------|------|
| macOS | `~/Library/Application Support/CIS` |
| Linux | `~/.local/share/cis` 或 `$XDG_DATA_HOME/cis` |
| Windows | `%LOCALAPPDATA%\CIS` |

### 完整目录树

```
$CIS_DATA_DIR/
├── config.toml              # 主配置文件
├── node.key                 # 节点私钥 (加密存储)
├── logs/
│   ├── cis.log              # 主日志
│   └── skills/
│       ├── im.log
│       └── ai-executor.log
│
├── core/                    # 核心数据目录
│   ├── core.db              # 核心记忆数据库 (SQLite)
│   ├── core.db-wal          # WAL 文件
│   └── backup/              # 自动备份
│       ├── core.db.2026-01-01
│       └── core.db.2026-02-01
│
├── skills/                  # Skill 数据目录
│   ├── registry.json        # Skill 注册表 (元数据、状态)
│   ├── installed/           # 已安装的 Skill 代码
│   │   ├── native/
│   │   │   ├── ai-executor/
│   │   │   ├── init-wizard/
│   │   │   └── im/          # Claude 开发的 IM skill
│   │   └── wasm/
│   │       ├── memory-organizer.wasm
│   │       └── push-client.wasm
│   │
│   └── data/                # Skill 独立数据库目录
│       ├── ai-executor/
│       │   └── data.db      # ai-executor 专用数据库
│       ├── im/
│       │   └── data.db      # IM 专用数据库 (消息、会话等)
│       └── memory-organizer/
│           └── data.db
│
├── cache/                   # 缓存目录 (可安全删除)
│   ├── ai/
│   │   └── responses/       # AI 响应缓存
│   ├── http/
│   │   └── ...
│   └── tmp/
│       └── ...
│
└── runtime/                 # 运行时数据 (重启后清空)
    ├── pid                  # 进程 PID
    ├── sockets/             # 本地 socket 文件
    └── locks/               # 文件锁
```

---

## 数据库分离设计

### 核心数据库 (`core/core.db`)

只存储 CIS 核心运行的必要数据，**不包含任何 Skill 业务数据**。

```sql
-- 核心任务表
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    priority INTEGER,
    created_at INTEGER,
    -- ...
);

-- 核心记忆索引 (引用 Skill 数据位置)
CREATE TABLE memory_index (
    key TEXT PRIMARY KEY,
    skill_name TEXT,           -- 关联的 Skill (NULL 表示核心)
    storage_type TEXT,         -- 'core' | 'skill'
    category TEXT,
    created_at INTEGER,
    updated_at INTEGER,
    -- 不存储实际 value，只存储引用
);

-- 节点配置
CREATE TABLE node_config (
    key TEXT PRIMARY KEY,
    value BLOB,
    encrypted BOOLEAN
);

-- P2P 节点信息 (Phase 4)
CREATE TABLE peers (
    node_id TEXT PRIMARY KEY,
    last_seen INTEGER,
    -- ...
);
```

### Skill 数据库 (`skills/data/{skill_name}/data.db`)

每个 Skill 拥有独立的数据库，完全隔离。

```sql
-- ai-executor/data.db
CREATE TABLE execution_history (
    id INTEGER PRIMARY KEY,
    agent TEXT,
    prompt_hash TEXT,          -- 隐私：只存 hash
    exit_code INTEGER,
    executed_at INTEGER
);

-- im/data.db
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    sender TEXT,
    content TEXT,
    msg_type TEXT,
    created_at INTEGER,
    -- ...
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    session_type TEXT,
    name TEXT,
    last_message_id TEXT,
    unread_count INTEGER,
    updated_at INTEGER
);

-- memory-organizer/data.db
CREATE TABLE keywords (
    memory_key TEXT,
    keyword TEXT,
    PRIMARY KEY (memory_key, keyword)
);

CREATE TABLE summaries (
    memory_key TEXT PRIMARY KEY,
    summary TEXT,
    generated_at INTEGER
);
```

---

## 热插拔支持

### Skill 生命周期

```
Installed → Registered → Loaded → Active → Unloading → Unloaded → Removed
                ↑_________|___________|       |
                         Pause      Resume     |
                                      ↑_________|
```

### 热插拔实现

1. **独立数据库连接**
   ```rust
   pub struct SkillDb {
       name: String,
       conn: rusqlite::Connection,
       path: PathBuf,
   }
   
   impl SkillDb {
       pub fn open(skill_name: &str, base_dir: &Path) -> Result<Self> {
           let path = base_dir.join("skills/data").join(skill_name).join("data.db");
           // 每个 Skill 独立连接
           let conn = rusqlite::Connection::open(&path)?;
           Ok(Self { name: skill_name.to_string(), conn, path })
       }
       
       pub fn close(self) -> Result<()> {
           // 显式关闭连接
           drop(self.conn);
           Ok(())
       }
   }
   ```

2. **Skill 注册表** (`skills/registry.json`)
   ```json
   {
     "version": "1.0",
     "skills": {
       "ai-executor": {
         "name": "ai-executor",
         "version": "1.0.0",
         "type": "native",
         "path": "skills/installed/native/ai-executor",
         "state": "active",
         "db_path": "skills/data/ai-executor/data.db",
         "permissions": ["ai_call", "memory_read"],
         "loaded_at": 1738473600,
         "pid": null
       },
       "im": {
         "name": "im",
         "version": "0.1.0",
         "type": "native",
         "path": "skills/installed/native/im",
         "state": "active",
         "db_path": "skills/data/im/data.db",
         "permissions": ["network", "memory_read", "memory_write"],
         "loaded_at": 1738473600,
         "pid": null
       }
     }
   }
   ```

3. **加载/卸载流程**
   ```rust
   impl SkillManager {
       pub async fn load_skill(&self, name: &str) -> Result<()> {
           // 1. 检查注册表
           let meta = self.registry.get(name)?;
           
           // 2. 创建 Skill 数据库连接
           let skill_db = SkillDb::open(name, &self.data_dir)?;
           
           // 3. 初始化 Skill
           let skill = load_skill_from_path(&meta.path).await?;
           
           // 4. 创建 SkillContext (注入 Host API + SkillDb)
           let ctx = SkillContext::new(self.core_api.clone(), skill_db);
           
           // 5. 初始化
           skill.init(ctx.config()).await?;
           
           // 6. 激活
           self.active_skills.insert(name.to_string(), ActiveSkill {
               skill: Arc::new(skill),
               db: skill_db,
               ctx,
           });
           
           // 7. 更新注册表
           self.registry.update_state(name, SkillState::Active);
           
           Ok(())
       }
       
       pub async fn unload_skill(&self, name: &str) -> Result<()> {
           // 1. 获取 Skill
           let active = self.active_skills.remove(name)?;
           
           // 2. 调用 shutdown
           active.skill.shutdown().await?;
           
           // 3. 关闭数据库连接
           active.db.close()?;
           
           // 4. 更新注册表
           self.registry.update_state(name, SkillState::Inactive);
           
           Ok(())
       }
   }
   ```

---

## 跨平台实现

### 目录获取

```rust
pub struct Paths;

impl Paths {
    /// 获取数据目录
    pub fn data_dir() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            dirs::data_dir().expect("Failed to get data dir").join("CIS")
        }
        
        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    dirs::home_dir()
                        .expect("Failed to get home dir")
                        .join(".local/share/cis")
                })
        }
        
        #[cfg(target_os = "windows")]
        {
            dirs::data_local_dir()
                .expect("Failed to get local data dir")
                .join("CIS")
        }
    }
    
    /// 核心数据库路径
    pub fn core_db() -> PathBuf {
        Self::data_dir().join("core/core.db")
    }
    
    /// Skill 数据库路径
    pub fn skill_db(skill_name: &str) -> PathBuf {
        Self::data_dir()
            .join("skills/data")
            .join(skill_name)
            .join("data.db")
    }
}
```

---

## 备份与迁移

### 自动备份策略

```rust
pub struct BackupManager {
    core_db_path: PathBuf,
    backup_dir: PathBuf,
    max_backups: usize,  // 默认 10
}

impl BackupManager {
    pub fn backup_core(&self) -> Result<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = self.backup_dir.join(format!("core.db.{}", timestamp));
        
        // 使用 SQLite backup API
        let src = rusqlite::Connection::open(&self.core_db_path)?;
        let mut dst = rusqlite::Connection::open(&backup_path)?;
        src.backup(rusqlite::DatabaseName::Main, &mut dst, None)?;
        
        // 清理旧备份
        self.cleanup_old_backups()?;
        
        Ok(backup_path)
    }
}
```

### Skill 数据迁移

Skill 升级时：
1. 新 Skill 版本可访问旧版本数据库
2. Skill 自行处理 schema 迁移
3. 失败时回滚到旧版本

---

## 发布目录结构

### macOS (.app Bundle)

```
CIS.app/
├── Contents/
│   ├── Info.plist
│   ├── MacOS/
│   │   └── cis                    # 主二进制
│   └── Resources/
│       └── builtin-skills/        # 内置 Skill
└── ...

# 用户数据在 ~/Library/Application Support/CIS
```

### Linux (AppImage/deb/rpm)

```
/opt/cis/
├── bin/cis                        # 主二进制
├── lib/builtin-skills/            # 内置 Skill
└── share/
    └── applications/cis.desktop

# 用户数据在 ~/.local/share/cis
```

### Windows (Installer/Portable)

```
%PROGRAMFILES%\CIS\
├── cis.exe                        # 主二进制
├── builtin-skills\                # 内置 Skill
└── ...

# 用户数据在 %LOCALAPPDATA%\CIS
# 便携模式：当前目录下的 data\ 文件夹
```

---

## 总结

| 组件 | 位置 | 数据库 |
|------|------|--------|
| 核心任务/配置 | `core/` | `core.db` |
| Skill 代码 | `skills/installed/` | - |
| Skill 数据 | `skills/data/{name}/` | `data.db` |
| 日志 | `logs/` | - |
| 缓存 | `cache/` | - |
| 运行时 | `runtime/` | - |

**热插拔保障**: 每个 Skill 独立数据库，卸载时关闭连接即可，不影响核心。
