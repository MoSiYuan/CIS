//! # Agent Session 仓储
//!
//! 提供 Session 管理功能，支持跨 Agent 复用。

use super::db::DatabasePool;
use super::models::{AgentEntity, AgentSessionEntity, SessionStatus};
use rusqlite::{params, Connection};
use std::sync::Arc;
use uuid::Uuid;

/// Agent Session 仓储
pub struct SessionRepository {
    db: Arc<DatabasePool>,
}

impl SessionRepository {
    /// 创建新的 Session 仓储
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    /// 创建 Session
    pub async fn create(
        &self,
        agent_id: i64,
        runtime_type: &str,
        context_capacity: i64,
        ttl_minutes: i64,
    ) -> rusqlite::Result<i64> {
        let conn = self.db.acquire().await?;

        let session_id = format!("{}-{}", agent_id, Uuid::new_v4());
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + (ttl_minutes * 60);

        let mut stmt = conn.prepare(
            "INSERT INTO agent_sessions (
                session_id, agent_id, runtime_type,
                status, context_capacity, context_used,
                created_at, last_used_at, expires_at
            ) VALUES (?1, ?2, ?3, 'active', ?4, 0, ?5, ?6, ?7)"
        )?;

        stmt.execute(params![
            &session_id,
            &agent_id,
            &runtime_type,
            &context_capacity,
            now,
            now,
            &expires_at,
        ])?;

        conn.last_insert_rowid()
    }

    /// 获取可复用的 Session
    pub async fn acquire_session(
        &self,
        agent_id: i64,
        min_capacity: i64,
    ) -> rusqlite::Result<Option<AgentSessionEntity>> {
        let conn = self.db.acquire().await?;

        let now = chrono::Utc::now().timestamp();

        // 查找该 Agent 的可用 Session
        let mut stmt = conn.prepare(
            "SELECT * FROM agent_sessions
             WHERE agent_id = ?1
             AND status = 'active'
             AND context_capacity - context_used >= ?2
             AND expires_at > ?3
             ORDER BY last_used_at ASC
             LIMIT 1"
        )?;

        let session = stmt.query_row(params![agent_id, min_capacity, now], |row| {
            Ok(AgentSessionEntity {
                id: row.get(0)?,
                session_id: row.get(1)?,
                agent_id: row.get(2)?,
                runtime_type: row.get(3)?,
                status: row.get(4)?,
                context_capacity: row.get(5)?,
                context_used: row.get(6)?,
                created_at: row.get(7)?,
                last_used_at: row.get(8)?,
                expires_at: row.get(9)?,
            })
        }).optional()?.unwrap_or(None);

        // 如果找到可用 Session，更新为 idle 状态
        if let Some(ref session) = session {
            conn.execute(
                "UPDATE agent_sessions SET status = 'idle', last_used_at = ?2
                 WHERE id = ?1",
                params![session.id, now],
            )?;
        }

        Ok(session)
    }

    /// 归还 Session（标记为 active，可复用）
    pub async fn release_session(&self, session_id: i64) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE agent_sessions
             SET status = 'active',
                 context_used = context_used + 1,
                 last_used_at = ?2
             WHERE id = ?1",
            params![session_id, now],
        )?;

        Ok(())
    }

    /// 标记 Session 为 expired
    pub async fn expire_session(&self, session_id: i64) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;

        conn.execute(
            "UPDATE agent_sessions SET status = 'expired' WHERE id = ?1",
            params![session_id],
        )?;

        Ok(())
    }

    /// 清理过期的 Sessions
    pub async fn cleanup_expired(&self) -> rusqlite::Result<usize> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();

        let count = conn.execute(
            "UPDATE agent_sessions SET status = 'expired'
             WHERE status != 'expired' AND expires_at < ?1",
            params![now],
        )?;

        Ok(count)
    }

    /// 获取 Session 详情
    pub async fn get_by_id(&self, id: i64) -> rusqlite::Result<Option<AgentSessionEntity>> {
        let conn = self.db.acquire().await?;

        let mut stmt = conn.prepare("SELECT * FROM agent_sessions WHERE id = ?1")?;

        let session = stmt.query_row(params![id], |row| {
            Ok(AgentSessionEntity {
                id: row.get(0)?,
                session_id: row.get(1)?,
                agent_id: row.get(2)?,
                runtime_type: row.get(3)?,
                status: row.get(4)?,
                context_capacity: row.get(5)?,
                context_used: row.get(6)?,
                created_at: row.get(7)?,
                last_used_at: row.get(8)?,
                expires_at: row.get(9)?,
            })
        }).optional()?.unwrap_or(None);

        Ok(session)
    }

    /// 根据 session_id 获取 Session
    pub async fn get_by_session_id(&self, session_id: &str) -> rusqlite::Result<Option<AgentSessionEntity>> {
        let conn = self.db.acquire().await?;

        let mut stmt = conn.prepare("SELECT * FROM agent_sessions WHERE session_id = ?1")?;

        let session = stmt.query_row(params![session_id], |row| {
            Ok(AgentSessionEntity {
                id: row.get(0)?,
                session_id: row.get(1)?,
                agent_id: row.get(2)?,
                runtime_type: row.get(3)?,
                status: row.get(4)?,
                context_capacity: row.get(5)?,
                context_used: row.get(6)?,
                created_at: row.get(7)?,
                last_used_at: row.get(8)?,
                expires_at: row.get(9)?,
            })
        }).optional()?.unwrap_or(None);

        Ok(session)
    }

    /// 列出 Agent 的所有 Sessions
    pub async fn list_by_agent(
        &self,
        agent_id: i64,
        status: Option<SessionStatus>,
    ) -> rusqlite::Result<Vec<AgentSessionEntity>> {
        let conn = self.db.acquire().await?;

        let mut sql = String::from(
            "SELECT * FROM agent_sessions WHERE agent_id = ?1"
        );

        if let Some(s) = status {
            sql.push_str(&format!(" AND status = '{}'", match s {
                SessionStatus::Active => "active",
                SessionStatus::Idle => "idle",
                SessionStatus::Expired => "expired",
                SessionStatus::Released => "released",
            }));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&sql)?;

        let sessions = stmt.query_map(params![agent_id], |row| {
            Ok(AgentSessionEntity {
                id: row.get(0)?,
                session_id: row.get(1)?,
                agent_id: row.get(2)?,
                runtime_type: row.get(3)?,
                status: row.get(4)?,
                context_capacity: row.get(5)?,
                context_used: row.get(6)?,
                created_at: row.get(7)?,
                last_used_at: row.get(8)?,
                expires_at: row.get(9)?,
            })
        })?.collect();

        sessions
    }

    /// 更新 Session 使用量
    pub async fn update_usage(
        &self,
        session_id: i64,
        tokens_used: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE agent_sessions
             SET context_used = context_used + ?2,
                 last_used_at = ?3
             WHERE id = ?1",
            params![session_id, tokens_used, now],
        )?;

        Ok(())
    }

    /// 删除 Session
    pub async fn delete(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        conn.execute("DELETE FROM agent_sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// 批量删除过期的 Sessions
    pub async fn delete_expired(&self, older_than_days: i64) -> rusqlite::Result<usize> {
        self.db.transaction(|conn| {
            let cutoff = chrono::Utc::now().timestamp() - (older_than_days * 86400);

            let count = conn.execute(
                "DELETE FROM agent_sessions
                 WHERE status = 'expired' AND expires_at < ?1",
                params![cutoff],
            )?;

            Ok(count)
        }).await
    }

    /// 统计活跃 Sessions
    pub async fn count_active(&self) -> rusqlite::Result<i64> {
        let conn = self.db.acquire().await?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE status = 'active'",
            [],
            |row| row.get(0),
        )?;

        Ok(count)
    }

    /// 统计 Agent 的 Sessions
    pub async fn count_by_agent(&self, agent_id: i64) -> rusqlite::Result<i64> {
        let conn = self.db.acquire().await?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_sessions WHERE agent_id = ?1",
            params![agent_id],
            |row| row.get(0),
        )?;

        Ok(count)
    }
}

/// Agent 仓储（用于获取 Agent 信息）
pub struct AgentRepository {
    db: Arc<DatabasePool>,
}

impl AgentRepository {
    /// 创建新的 Agent 仓储
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    /// 注册 Agent
    pub async fn register(
        &self,
        agent_type: &str,
        display_name: &str,
        config: &serde_json::Value,
        capabilities: &[String],
    ) -> rusqlite::Result<i64> {
        let conn = self.db.acquire().await?;

        let config_json = serde_json::to_string(config)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let capabilities_json = serde_json::to_string(capabilities)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn.prepare(
            "INSERT INTO agents (
                agent_type, display_name, enabled, config_json, capabilities_json,
                created_at, updated_at
            ) VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6)
            ON CONFLICT(agent_type) DO UPDATE SET
                display_name = ?2,
                config_json = ?3,
                capabilities_json = ?4,
                updated_at = ?5"
        )?;

        stmt.execute(params![
            agent_type,
            display_name,
            &config_json,
            &capabilities_json,
            now,
            now,
        ])?;

        conn.last_insert_rowid()
    }

    /// 获取 Agent（根据类型）
    pub async fn get_by_type(&self, agent_type: &str) -> rusqlite::Result<Option<AgentEntity>> {
        let conn = self.db.acquire().await?;

        let mut stmt = conn.prepare("SELECT * FROM agents WHERE agent_type = ?1")?;

        let agent = stmt.query_row(params![agent_type], |row| {
            Ok(AgentEntity {
                id: row.get(0)?,
                agent_type: row.get(1)?,
                display_name: row.get(2)?,
                enabled: row.get(3)?,
                config: serde_json::from_str(row.get::<_, String>(4)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(Box::new(e).into()))?,
                capabilities: serde_json::from_str(row.get::<_, String>(5)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(Box::new(e).into()))?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        }).optional()?.unwrap_or(None);

        Ok(agent)
    }

    /// 列出所有启用的 Agents
    pub async fn list_enabled(&self) -> rusqlite::Result<Vec<AgentEntity>> {
        let conn = self.db.acquire().await?;

        let mut stmt = conn.prepare("SELECT * FROM agents WHERE enabled = 1 ORDER BY created_at ASC")?;

        let agents = stmt.query_map([], |row| {
            Ok(AgentEntity {
                id: row.get(0)?,
                agent_type: row.get(1)?,
                display_name: row.get(2)?,
                enabled: row.get(3)?,
                config: serde_json::from_str(row.get::<_, String>(4)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(Box::new(e).into()))?,
                capabilities: serde_json::from_str(row.get::<_, String>(5)?)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(Box::new(e).into()))?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?.collect();

        agents
    }

    /// 启用/禁用 Agent
    pub async fn set_enabled(&self, agent_id: i64, enabled: bool) -> rusqlite::Result<()> {
        let conn = self.db.acquire().await?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE agents SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
            params![agent_id, enabled, now],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::db::create_database_pool;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_and_acquire_session() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_database_pool(Some(db_path), 5).await;
        let session_repo = SessionRepository::new(pool.clone());

        // 先注册一个 Agent
        let agent_repo = AgentRepository::new(pool);
        let agent_id = agent_repo.register(
            "claude",
            "Claude AI",
            &serde_json::json!({"model": "claude-3"}),
            &vec!["code_review".to_string(), "module_refactoring".to_string()],
        ).await.unwrap();

        // 创建 Session
        let session_id = session_repo.create(
            agent_id,
            "claude",
            100000,  // 100K tokens
            60,      // 60 分钟 TTL
        ).await.unwrap();

        assert!(session_id > 0);

        // 获取 Session
        let session = session_repo.get_by_id(session_id).await.unwrap().unwrap();
        assert_eq!(session.agent_id, agent_id);
        assert_eq!(session.runtime_type, "claude");
        assert_eq!(session.context_capacity, 100000);
        assert_eq!(session.context_used, 0);
        assert_eq!(session.status, SessionStatus::Active);

        // 复用 Session
        let acquired = session_repo.acquire_session(agent_id, 50000).await.unwrap();
        assert!(acquired.is_some());
        assert_eq!(acquired.unwrap().id, session_id);
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_database_pool(Some(db_path), 5).await;
        let session_repo = SessionRepository::new(pool.clone());

        // 注册 Agent
        let agent_repo = AgentRepository::new(pool);
        let agent_id = agent_repo.register(
            "claude",
            "Claude AI",
            &serde_json::json!({}),
            &vec![],
        ).await.unwrap();

        // 创建短期 Session（1 秒 TTL）
        let session_id = session_repo.create(
            agent_id,
            "claude",
            100000,
            0,  // 0 秒 TTL
        ).await.unwrap();

        // 清理过期 Sessions
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let expired_count = session_repo.cleanup_expired().await.unwrap();
        assert!(expired_count >= 1);
    }
}
