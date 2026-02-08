//! 独立的邦联通信数据库
//!
//! 存储 P2P 网络相关数据：节点状态、信任网络、断线同步队列等

use rusqlite::{Connection, Row};
use std::path::{Path, PathBuf};

use crate::error::{CisError, Result};

/// 邦联数据库
///
/// 独立存储 P2P 网络相关数据，包括 DID 信任网络、网络节点状态、断线同步队列和联邦日志
pub struct FederationDb {
    conn: Connection,
    path: PathBuf,
}

impl std::fmt::Debug for FederationDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FederationDb")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl FederationDb {
    /// 打开邦联数据库
    pub fn open(path: &Path) -> Result<Self> {
        // 确保目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CisError::Storage(format!("Failed to create federation db directory: {}", e)))?;
        }

        let conn = Connection::open(path)
            .map_err(|e| CisError::Storage(format!("Failed to open federation db: {}", e)))?;

        let db = Self {
            conn,
            path: path.to_path_buf(),
        };
        
        // 配置 WAL 模式
        db.configure_wal()?;
        
        // 初始化 schema
        db.init_schema()?;
        
        Ok(db)
    }

    /// 配置 WAL 模式（随时关机安全）
    fn configure_wal(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA journal_size_limit = 100000000;
             PRAGMA temp_store = memory;"
        ).map_err(|e| CisError::Storage(format!("Failed to configure WAL: {}", e)))?;
        Ok(())
    }

    /// 初始化 Schema（根据 MATRIX-final.md）
    fn init_schema(&self) -> Result<()> {
        // DID 信任网络表
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS did_trust (
                trustor TEXT,
                trustee TEXT,
                trust_level INTEGER CHECK(trust_level IN (0,1,2)),
                -- 0=黑名单, 1=读, 2=写
                updated_at INTEGER,
                PRIMARY KEY (trustor, trustee)
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create did_trust table: {}", e)))?;

        // 网络节点状态表（WebSocket 联邦视图）
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS network_peers (
                node_id TEXT PRIMARY KEY,
                did TEXT NOT NULL,
                endpoint_ws TEXT,
                status INTEGER, -- 0=离线, 1=在线, 2=打洞中
                last_seen INTEGER,
                rtt_ms INTEGER, -- 网络延迟
                public_key TEXT
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create network_peers table: {}", e)))?;

        // 断线同步队列（记忆补全机制）
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS pending_sync (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                target_node TEXT,
                room_id TEXT,
                since_event_id TEXT,
                priority INTEGER, -- 0=公域优先, 1=私域按需
                created_at INTEGER,
                retry_count INTEGER DEFAULT 0
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create pending_sync table: {}", e)))?;

        // 联邦消息日志（用于追溯）
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS federation_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                direction TEXT, -- 'in' or 'out'
                node_id TEXT,
                event_type TEXT,
                event_id TEXT,
                size_bytes INTEGER,
                status TEXT, -- 'success', 'failed', 'pending'
                timestamp INTEGER
            )",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create federation_logs table: {}", e)))?;

        // 创建索引
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_did_trust_trustor ON did_trust(trustor)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_did_trust_trustor: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_did_trust_trustee ON did_trust(trustee)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_did_trust_trustee: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_network_peers_did ON network_peers(did)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_network_peers_did: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_network_peers_status ON network_peers(status)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_network_peers_status: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pending_sync_target ON pending_sync(target_node)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_pending_sync_target: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pending_sync_priority ON pending_sync(priority)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_pending_sync_priority: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_federation_logs_node ON federation_logs(node_id)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_federation_logs_node: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_federation_logs_timestamp ON federation_logs(timestamp)",
            [],
        ).map_err(|e| CisError::Storage(format!("Failed to create idx_federation_logs_timestamp: {}", e)))?;

        Ok(())
    }

    // === DID 信任网络 ===

    /// 设置信任级别
    pub fn set_trust(&self, trustor: &str, trustee: &str, level: TrustLevel) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let level_int = level as i32;
        
        self.conn.execute(
            "INSERT INTO did_trust (trustor, trustee, trust_level, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(trustor, trustee) DO UPDATE SET
             trust_level = excluded.trust_level,
             updated_at = excluded.updated_at",
            rusqlite::params![trustor, trustee, level_int, now],
        ).map_err(|e| CisError::Storage(format!("Failed to set trust: {}", e)))?;
        
        Ok(())
    }

    /// 获取信任级别
    pub fn get_trust(&self, trustor: &str, trustee: &str) -> Result<TrustLevel> {
        let mut stmt = self.conn.prepare(
            "SELECT trust_level FROM did_trust WHERE trustor = ?1 AND trustee = ?2"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare get_trust query: {}", e)))?;

        let result = stmt.query_row([trustor, trustee], |row| {
            let level: i32 = row.get(0)?;
            Ok(TrustLevel::from_i32(level))
        });

        match result {
            Ok(level) => Ok(level),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(TrustLevel::Blocked),
            Err(e) => Err(CisError::Storage(format!("Failed to get trust: {}", e))),
        }
    }

    /// 获取信任列表
    pub fn list_trusted(&self, trustor: &str, level: TrustLevel) -> Result<Vec<String>> {
        let level_int = level as i32;
        let mut stmt = self.conn.prepare(
            "SELECT trustee FROM did_trust WHERE trustor = ?1 AND trust_level >= ?2"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare list_trusted query: {}", e)))?;

        let trustees: Result<Vec<String>> = stmt
            .query_map(rusqlite::params![trustor, level_int], |row| row.get(0))
            .map_err(|e| CisError::Storage(format!("Failed to query trusted list: {}", e)))?
            .map(|r| r.map_err(CisError::Database))
            .collect();

        trustees
    }

    // === 网络节点状态 ===

    /// 注册/更新节点
    pub fn upsert_peer(&self, info: &PeerInfo) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let status_int = info.status as i32;
        
        self.conn.execute(
            "INSERT INTO network_peers (node_id, did, endpoint_ws, status, last_seen, rtt_ms, public_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(node_id) DO UPDATE SET
             did = excluded.did,
             endpoint_ws = excluded.endpoint_ws,
             status = excluded.status,
             last_seen = excluded.last_seen,
             rtt_ms = excluded.rtt_ms,
             public_key = excluded.public_key",
            rusqlite::params![
                info.node_id,
                info.did,
                info.endpoint_ws,
                status_int,
                now,
                info.rtt_ms,
                info.public_key
            ],
        ).map_err(|e| CisError::Storage(format!("Failed to upsert peer: {}", e)))?;
        
        Ok(())
    }

    /// 获取节点信息
    pub fn get_peer(&self, node_id: &str) -> Result<Option<PeerInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT node_id, did, endpoint_ws, status, last_seen, rtt_ms, public_key
             FROM network_peers WHERE node_id = ?1"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare get_peer query: {}", e)))?;

        let result = stmt.query_row([node_id], Self::row_to_peer_info);

        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::Storage(format!("Failed to get peer: {}", e))),
        }
    }

    /// 列出在线节点
    pub fn list_online_peers(&self) -> Result<Vec<PeerInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT node_id, did, endpoint_ws, status, last_seen, rtt_ms, public_key
             FROM network_peers WHERE status = 1"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare list_online_peers query: {}", e)))?;

        let peers: Result<Vec<PeerInfo>> = stmt
            .query_map([], Self::row_to_peer_info)
            .map_err(|e| CisError::Storage(format!("Failed to query online peers: {}", e)))?
            .map(|r| r.map_err(CisError::Database))
            .collect();

        peers
    }

    /// 更新节点状态
    pub fn update_peer_status(&self, node_id: &str, status: PeerStatus) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let status_int = status as i32;
        
        self.conn.execute(
            "UPDATE network_peers SET status = ?1, last_seen = ?2 WHERE node_id = ?3",
            rusqlite::params![status_int, now, node_id],
        ).map_err(|e| CisError::Storage(format!("Failed to update peer status: {}", e)))?;
        
        Ok(())
    }

    /// 更新 RTT
    pub fn update_peer_rtt(&self, node_id: &str, rtt_ms: i32) -> Result<()> {
        self.conn.execute(
            "UPDATE network_peers SET rtt_ms = ?1 WHERE node_id = ?2",
            rusqlite::params![rtt_ms, node_id],
        ).map_err(|e| CisError::Storage(format!("Failed to update peer RTT: {}", e)))?;
        
        Ok(())
    }

    fn row_to_peer_info(row: &Row) -> std::result::Result<PeerInfo, rusqlite::Error> {
        let status_int: i32 = row.get(3)?;
        
        Ok(PeerInfo {
            node_id: row.get(0)?,
            did: row.get(1)?,
            endpoint_ws: row.get(2)?,
            status: PeerStatus::from_i32(status_int),
            last_seen: row.get(4)?,
            rtt_ms: row.get(5)?,
            public_key: row.get(6)?,
        })
    }

    // === 断线同步队列 ===

    /// 添加同步任务
    pub fn add_sync_task(&self, task: &SyncTask) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        
        self.conn.execute(
            "INSERT INTO pending_sync (target_node, room_id, since_event_id, priority, created_at, retry_count)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            rusqlite::params![task.target_node, task.room_id, task.since_event_id, task.priority, now],
        ).map_err(|e| CisError::Storage(format!("Failed to add sync task: {}", e)))?;
        
        Ok(())
    }

    /// 获取待同步任务（按优先级排序）
    pub fn get_pending_tasks(&self, limit: usize) -> Result<Vec<SyncTask>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, target_node, room_id, since_event_id, priority, retry_count
             FROM pending_sync ORDER BY priority ASC, created_at ASC LIMIT ?1"
        ).map_err(|e| CisError::Storage(format!("Failed to prepare get_pending_tasks query: {}", e)))?;

        let tasks: Result<Vec<SyncTask>> = stmt
            .query_map([limit as i64], |row| {
                Ok(SyncTask {
                    id: row.get(0)?,
                    target_node: row.get(1)?,
                    room_id: row.get(2)?,
                    since_event_id: row.get(3)?,
                    priority: row.get(4)?,
                })
            })
            .map_err(|e| CisError::Storage(format!("Failed to query pending tasks: {}", e)))?
            .map(|r| r.map_err(CisError::Database))
            .collect();

        tasks
    }

    /// 完成任务
    pub fn complete_sync_task(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM pending_sync WHERE id = ?1",
            [id],
        ).map_err(|e| CisError::Storage(format!("Failed to complete sync task: {}", e)))?;
        
        Ok(())
    }

    /// 增加重试计数
    pub fn increment_retry(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE pending_sync SET retry_count = retry_count + 1 WHERE id = ?1",
            [id],
        ).map_err(|e| CisError::Storage(format!("Failed to increment retry: {}", e)))?;
        
        Ok(())
    }

    // === 联邦日志 ===

    /// 记录日志
    pub fn log_federation(&self, log: &FederationLog) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        
        self.conn.execute(
            "INSERT INTO federation_logs (direction, node_id, event_type, event_id, size_bytes, status, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![log.direction, log.node_id, log.event_type, log.event_id, log.size_bytes, log.status, now],
        ).map_err(|e| CisError::Storage(format!("Failed to log federation: {}", e)))?;
        
        Ok(())
    }

    /// 获取日志
    pub fn get_logs(&self, node_id: Option<&str>, limit: usize) -> Result<Vec<FederationLog>> {
        let limit_i64 = limit as i64;
        let mut logs = Vec::new();
        
        if let Some(nid) = node_id {
            let mut stmt = self.conn.prepare(
                "SELECT direction, node_id, event_type, event_id, size_bytes, status
                 FROM federation_logs WHERE node_id = ?1 ORDER BY timestamp DESC LIMIT ?2"
            ).map_err(|e| CisError::Storage(format!("Failed to prepare get_logs query: {}", e)))?;
            
            let rows = stmt.query_map(rusqlite::params![nid, limit_i64], |row| {
                Ok(FederationLog {
                    direction: row.get(0)?,
                    node_id: row.get(1)?,
                    event_type: row.get(2)?,
                    event_id: row.get(3)?,
                    size_bytes: row.get(4)?,
                    status: row.get(5)?,
                })
            }).map_err(|e| CisError::Storage(format!("Failed to query logs: {}", e)))?;
            
            for row in rows {
                logs.push(row.map_err(CisError::Database)?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT direction, node_id, event_type, event_id, size_bytes, status
                 FROM federation_logs ORDER BY timestamp DESC LIMIT ?1"
            ).map_err(|e| CisError::Storage(format!("Failed to prepare get_logs query: {}", e)))?;
            
            let rows = stmt.query_map([limit_i64], |row| {
                Ok(FederationLog {
                    direction: row.get(0)?,
                    node_id: row.get(1)?,
                    event_type: row.get(2)?,
                    event_id: row.get(3)?,
                    size_bytes: row.get(4)?,
                    status: row.get(5)?,
                })
            }).map_err(|e| CisError::Storage(format!("Failed to query logs: {}", e)))?;
            
            for row in rows {
                logs.push(row.map_err(CisError::Database)?);
            }
        }
        
        Ok(logs)
    }

    /// 获取底层连接（用于复杂查询）
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 获取数据库路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 关闭连接
    pub fn close(self) -> Result<()> {
        self.conn.close()
            .map_err(|(_, e)| CisError::Storage(format!("Failed to close federation db: {}", e)))
    }
}

/// 节点信息
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: String,
    pub did: String,
    pub endpoint_ws: Option<String>,
    pub status: PeerStatus,
    pub last_seen: i64,
    pub rtt_ms: Option<i32>,
    pub public_key: String,
}

/// 节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerStatus {
    /// 离线
    Offline = 0,
    /// 在线
    Online = 1,
    /// 打洞中
    HolePunching = 2,
}

impl PeerStatus {
    /// 从 i32 转换
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => PeerStatus::Online,
            2 => PeerStatus::HolePunching,
            _ => PeerStatus::Offline,
        }
    }
}

/// 信任级别
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    /// 黑名单
    Blocked = 0,
    /// 读权限
    Read = 1,
    /// 写权限
    Write = 2,
}

impl TrustLevel {
    /// 从 i32 转换
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => TrustLevel::Read,
            2 => TrustLevel::Write,
            _ => TrustLevel::Blocked,
        }
    }
}

/// 同步任务
#[derive(Debug, Clone)]
pub struct SyncTask {
    pub id: Option<i64>,
    pub target_node: String,
    pub room_id: String,
    pub since_event_id: String,
    pub priority: i32,
}

/// 联邦日志
#[derive(Debug, Clone)]
pub struct FederationLog {
    pub direction: String, // 'in' or 'out'
    pub node_id: String,
    pub event_type: String,
    pub event_id: String,
    pub size_bytes: Option<i32>,
    pub status: String, // 'success', 'failed', 'pending'
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn setup_test_db() -> FederationDb {
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = env::temp_dir().join(format!("cis_test_federation_db_{}", counter));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("federation.db");
        FederationDb::open(&db_path).unwrap()
    }

    fn cleanup_test_db() {
        // 不再需要，因为每次测试使用独立的目录
    }

    #[test]
    fn test_trust_network() {
        let db = setup_test_db();

        // 设置信任
        db.set_trust("did:example:alice", "did:example:bob", TrustLevel::Write).unwrap();
        db.set_trust("did:example:alice", "did:example:carol", TrustLevel::Read).unwrap();
        
        // 获取信任
        let trust = db.get_trust("did:example:alice", "did:example:bob").unwrap();
        assert_eq!(trust, TrustLevel::Write);
        
        let trust = db.get_trust("did:example:alice", "did:example:carol").unwrap();
        assert_eq!(trust, TrustLevel::Read);
        
        // 未设置的信任默认为 Blocked
        let trust = db.get_trust("did:example:alice", "did:example:dave").unwrap();
        assert_eq!(trust, TrustLevel::Blocked);

        // 列出信任列表
        let trusted = db.list_trusted("did:example:alice", TrustLevel::Read).unwrap();
        assert_eq!(trusted.len(), 2);
        assert!(trusted.contains(&"did:example:bob".to_string()));
        assert!(trusted.contains(&"did:example:carol".to_string()));
    }

    #[test]
    fn test_peer_management() {
        let db = setup_test_db();

        // 添加节点
        let peer = PeerInfo {
            node_id: "node1".to_string(),
            did: "did:example:node1".to_string(),
            endpoint_ws: Some("ws://192.168.1.1:8080".to_string()),
            status: PeerStatus::Offline,
            last_seen: 0,
            rtt_ms: None,
            public_key: "pubkey1".to_string(),
        };
        db.upsert_peer(&peer).unwrap();

        // 获取节点
        let info = db.get_peer("node1").unwrap();
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.node_id, "node1");
        assert_eq!(info.did, "did:example:node1");

        // 更新状态
        db.update_peer_status("node1", PeerStatus::Online).unwrap();
        let info = db.get_peer("node1").unwrap().unwrap();
        assert_eq!(info.status, PeerStatus::Online);

        // 更新 RTT
        db.update_peer_rtt("node1", 50).unwrap();
        let info = db.get_peer("node1").unwrap().unwrap();
        assert_eq!(info.rtt_ms, Some(50));

        // 列出在线节点
        let online = db.list_online_peers().unwrap();
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].node_id, "node1");
    }

    #[test]
    fn test_sync_tasks() {
        let db = setup_test_db();

        // 添加任务
        let task = SyncTask {
            id: None,
            target_node: "node1".to_string(),
            room_id: "!room:example.com".to_string(),
            since_event_id: "$event1".to_string(),
            priority: 0,
        };
        db.add_sync_task(&task).unwrap();

        // 获取待处理任务
        let tasks = db.get_pending_tasks(10).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].target_node, "node1");

        // 增加重试
        let task_id = tasks[0].id.unwrap();
        db.increment_retry(task_id).unwrap();

        // 完成任务
        db.complete_sync_task(task_id).unwrap();
        let tasks = db.get_pending_tasks(10).unwrap();
        assert_eq!(tasks.len(), 0);
    }

    #[test]
    fn test_federation_logs() {
        let db = setup_test_db();

        // 添加日志
        let log = FederationLog {
            direction: "in".to_string(),
            node_id: "node1".to_string(),
            event_type: "m.room.message".to_string(),
            event_id: "$event1".to_string(),
            size_bytes: Some(1024),
            status: "success".to_string(),
        };
        db.log_federation(&log).unwrap();

        // 获取所有日志
        let logs = db.get_logs(None, 10).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].node_id, "node1");

        // 按节点过滤
        let logs = db.get_logs(Some("node1"), 10).unwrap();
        assert_eq!(logs.len(), 1);

        let logs = db.get_logs(Some("node2"), 10).unwrap();
        assert_eq!(logs.len(), 0);
    }
}
