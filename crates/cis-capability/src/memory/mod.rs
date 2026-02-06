//! Memory service for persisting project preferences and execution history

use crate::types::{MemoryEntry, MemoryScope, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

pub struct MemoryService {
    conn: Connection,
}

impl MemoryService {
    /// Open memory database
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let service = Self { conn };
        service.init_schema()?;
        Ok(service)
    }

    /// Open with default path
    pub fn open_default() -> Result<Self> {
        let path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("cis")
            .join("memory.db");
        
        std::fs::create_dir_all(path.parent().unwrap())?;
        Self::open(path)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                scope TEXT NOT NULL,
                project_path TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            
            CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key);
            CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope);
            CREATE INDEX IF NOT EXISTS idx_memories_project ON memories(project_path);
            
            PRAGMA journal_mode = WAL;"
        )?;
        Ok(())
    }

    /// Store a memory
    pub fn store(&self, key: impl Into<String>, value: impl Into<String>, scope: MemoryScope, project_path: Option<&Path>) -> Result<MemoryEntry> {
        let entry = MemoryEntry {
            id: Uuid::new_v4().to_string(),
            key: key.into(),
            value: value.into(),
            scope,
            project_path: project_path.map(|p| p.to_path_buf()),
            created_at: Utc::now(),
        };

        self.conn.execute(
            "INSERT OR REPLACE INTO memories 
             (id, key, value, scope, project_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            rusqlite::params![
                &entry.id,
                &entry.key,
                &entry.value,
                format!("{:?}", entry.scope).to_lowercase(),
                entry.project_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                entry.created_at.timestamp()
            ],
        )?;

        Ok(entry)
    }

    /// Recall a memory by key
    pub fn recall(&self, key: &str, project_path: Option<&Path>) -> Result<Option<String>> {
        // First try project-specific
        if let Some(project) = project_path {
            let result: Option<String> = self.conn.query_row(
                "SELECT value FROM memories 
                 WHERE key = ?1 AND scope = 'project' AND project_path = ?2
                 ORDER BY updated_at DESC LIMIT 1",
                rusqlite::params![key, project.to_string_lossy().to_string()],
                |row| row.get(0)
            ).optional()?;

            if result.is_some() {
                return Ok(result);
            }
        }

        // Then try global
        let result: Option<String> = self.conn.query_row(
            "SELECT value FROM memories 
             WHERE key = ?1 AND scope = 'global'
             ORDER BY updated_at DESC LIMIT 1",
            [key],
            |row| row.get(0)
        ).optional()?;

        Ok(result)
    }

    /// Search memories by prefix
    pub fn search(&self, prefix: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, key, value, scope, project_path, created_at 
             FROM memories 
             WHERE key LIKE ?1
             ORDER BY updated_at DESC 
             LIMIT ?2"
        )?;

        let pattern = format!("{}%", prefix);
        let rows = stmt.query_map(
            rusqlite::params![pattern, limit as i64],
            |row| {
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    scope: parse_scope(&row.get::<_, String>(3)?),
                    project_path: row.get::<_, Option<String>>(4)?.map(|p| p.into()),
                    created_at: chrono::DateTime::from_timestamp(row.get(5)?, 0)
                        .unwrap_or_else(|| Utc::now()),
                })
            }
        )?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Get all memories for a project
    pub fn get_project_memories(&self, project_path: &Path) -> Result<Vec<MemoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, key, value, scope, project_path, created_at 
             FROM memories 
             WHERE project_path = ?1 OR scope = 'global'
             ORDER BY updated_at DESC"
        )?;

        let rows = stmt.query_map(
            [project_path.to_string_lossy().to_string()],
            |row| {
                Ok(MemoryEntry {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    scope: parse_scope(&row.get::<_, String>(3)?),
                    project_path: row.get::<_, Option<String>>(4)?.map(|p| p.into()),
                    created_at: chrono::DateTime::from_timestamp(row.get(5)?, 0)
                        .unwrap_or_else(|| Utc::now()),
                })
            }
        )?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Delete a memory
    pub fn forget(&self, key: &str, project_path: Option<&Path>) -> Result<bool> {
        let affected = if let Some(project) = project_path {
            self.conn.execute(
                "DELETE FROM memories WHERE key = ?1 AND project_path = ?2",
                rusqlite::params![key, project.to_string_lossy().to_string()]
            )?
        } else {
            self.conn.execute(
                "DELETE FROM memories WHERE key = ?1",
                [key]
            )?
        };

        Ok(affected > 0)
    }
}

fn parse_scope(s: &str) -> MemoryScope {
    match s {
        "global" => MemoryScope::Global,
        "project" => MemoryScope::Project,
        _ => MemoryScope::Session,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_memory_store_and_recall() {
        let temp = TempDir::new().unwrap();
        let service = MemoryService::open(temp.path().join("test.db")).unwrap();

        service.store("test_cmd", "cargo test", MemoryScope::Global, None).unwrap();
        
        let value = service.recall("test_cmd", None).unwrap();
        assert_eq!(value, Some("cargo test".to_string()));
    }

    #[test]
    fn test_project_memory_priority() {
        let temp = TempDir::new().unwrap();
        let service = MemoryService::open(temp.path().join("test.db")).unwrap();
        let project = temp.path().join("myproject");

        // Store global
        service.store("cmd", "global_cmd", MemoryScope::Global, None).unwrap();
        // Store project-specific
        service.store("cmd", "project_cmd", MemoryScope::Project, Some(&project)).unwrap();

        // Should get project-specific
        let value = service.recall("cmd", Some(&project)).unwrap();
        assert_eq!(value, Some("project_cmd".to_string()));

        // Different project should get global
        let other = temp.path().join("other");
        let value = service.recall("cmd", Some(&other)).unwrap();
        assert_eq!(value, Some("global_cmd".to_string()));
    }
}
