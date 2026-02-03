//! Skill 兼容性数据库操作
//!
//! 支持 CVI-010: Skill Chain Orchestrator
//! 管理 skill_compatibility 表的创建和查询

use rusqlite::{Connection, Row};

use crate::error::{CisError, Result};
use crate::skill::chain::SkillCompatibilityRecord;

/// Skill 兼容性数据库操作
pub struct SkillCompatibilityDb<'a> {
    conn: &'a Connection,
}

impl<'a> SkillCompatibilityDb<'a> {
    /// 创建新的兼容性数据库操作实例
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// 初始化 skill_compatibility 表
    pub fn init_table(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS skill_compatibility (
                source_skill_id TEXT NOT NULL,
                target_skill_id TEXT NOT NULL,
                compatibility_score REAL NOT NULL,
                data_flow_types TEXT NOT NULL,
                discovered_at INTEGER NOT NULL,
                PRIMARY KEY (source_skill_id, target_skill_id)
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create skill_compatibility table: {}", e)))?;

        // 创建索引
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_compat_source ON skill_compatibility(source_skill_id)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create source index: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_compat_target ON skill_compatibility(target_skill_id)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create target index: {}", e)))?;

        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_compat_score ON skill_compatibility(compatibility_score)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create score index: {}", e)))?;

        Ok(())
    }

    /// 插入或更新兼容性记录
    pub fn upsert(&self, record: &SkillCompatibilityRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO skill_compatibility 
             (source_skill_id, target_skill_id, compatibility_score, data_flow_types, discovered_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source_skill_id, target_skill_id) DO UPDATE SET
             compatibility_score = excluded.compatibility_score,
             data_flow_types = excluded.data_flow_types,
             discovered_at = excluded.discovered_at",
            rusqlite::params![
                record.source_skill_id,
                record.target_skill_id,
                record.compatibility_score,
                record.data_flow_types,
                record.discovered_at
            ],
        ).map_err(|e| CisError::storage(format!("Failed to upsert compatibility: {}", e)))?;

        Ok(())
    }

    /// 获取兼容性记录
    pub fn get(&self, source_id: &str, target_id: &str) -> Result<Option<SkillCompatibilityRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_skill_id, target_skill_id, compatibility_score, data_flow_types, discovered_at
             FROM skill_compatibility
             WHERE source_skill_id = ?1 AND target_skill_id = ?2"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let result = stmt.query_row(
            rusqlite::params![source_id, target_id],
            Self::row_to_record,
        );

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::storage(format!("Failed to query compatibility: {}", e))),
        }
    }

    /// 获取源技能的所有兼容目标
    pub fn find_compatible_targets(&self, source_id: &str, min_score: f64) -> Result<Vec<SkillCompatibilityRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_skill_id, target_skill_id, compatibility_score, data_flow_types, discovered_at
             FROM skill_compatibility
             WHERE source_skill_id = ?1 AND compatibility_score >= ?2
             ORDER BY compatibility_score DESC"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map(
            rusqlite::params![source_id, min_score],
            Self::row_to_record,
        ).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?);
        }

        Ok(results)
    }

    /// 获取目标技能的所有兼容源
    pub fn find_compatible_sources(&self, target_id: &str, min_score: f64) -> Result<Vec<SkillCompatibilityRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_skill_id, target_skill_id, compatibility_score, data_flow_types, discovered_at
             FROM skill_compatibility
             WHERE target_skill_id = ?1 AND compatibility_score >= ?2
             ORDER BY compatibility_score DESC"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map(
            rusqlite::params![target_id, min_score],
            Self::row_to_record,
        ).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?);
        }

        Ok(results)
    }

    /// 删除兼容性记录
    pub fn delete(&self, source_id: &str, target_id: &str) -> Result<bool> {
        let rows = self.conn.execute(
            "DELETE FROM skill_compatibility WHERE source_skill_id = ?1 AND target_skill_id = ?2",
            rusqlite::params![source_id, target_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete compatibility: {}", e)))?;

        Ok(rows > 0)
    }

    /// 获取所有兼容性记录
    pub fn list_all(&self, limit: usize) -> Result<Vec<SkillCompatibilityRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_skill_id, target_skill_id, compatibility_score, data_flow_types, discovered_at
             FROM skill_compatibility
             ORDER BY compatibility_score DESC
             LIMIT ?1"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map(
            rusqlite::params![limit as i64],
            Self::row_to_record,
        ).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?);
        }

        Ok(results)
    }

    /// 获取记录数量
    pub fn count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM skill_compatibility",
            [],
            |row| row.get(0),
        ).map_err(|e| CisError::storage(format!("Failed to count: {}", e)))?;

        Ok(count)
    }

    /// 清空所有记录
    pub fn clear(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM skill_compatibility",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to clear: {}", e)))?;

        Ok(())
    }

    /// 行转换函数
    fn row_to_record(row: &Row) -> rusqlite::Result<SkillCompatibilityRecord> {
        Ok(SkillCompatibilityRecord {
            source_skill_id: row.get(0)?,
            target_skill_id: row.get(1)?,
            compatibility_score: row.get(2)?,
            data_flow_types: row.get(3)?,
            discovered_at: row.get(4)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_record(source: &str, target: &str, score: f64) -> SkillCompatibilityRecord {
        SkillCompatibilityRecord {
            source_skill_id: source.to_string(),
            target_skill_id: target.to_string(),
            compatibility_score: score,
            data_flow_types: "json,csv".to_string(),
            discovered_at: chrono::Utc::now().timestamp(),
        }
    }

    #[test]
    fn test_compatibility_db() {
        let conn = Connection::open_in_memory().unwrap();
        let db = SkillCompatibilityDb::new(&conn);
        
        // 初始化表
        db.init_table().unwrap();
        
        // 插入记录
        let record1 = create_test_record("skill-a", "skill-b", 0.85);
        db.upsert(&record1).unwrap();
        
        let record2 = create_test_record("skill-a", "skill-c", 0.75);
        db.upsert(&record2).unwrap();
        
        // 查询
        let fetched = db.get("skill-a", "skill-b").unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().compatibility_score, 0.85);
        
        // 查找兼容目标
        let targets = db.find_compatible_targets("skill-a", 0.7).unwrap();
        assert_eq!(targets.len(), 2);
        
        // 查找兼容源
        let sources = db.find_compatible_sources("skill-b", 0.7).unwrap();
        assert_eq!(sources.len(), 1);
        
        // 删除
        assert!(db.delete("skill-a", "skill-b").unwrap());
        assert!(!db.delete("skill-a", "skill-b").unwrap());
        
        // 统计
        let count = db.count().unwrap();
        assert_eq!(count, 1);
    }
}
