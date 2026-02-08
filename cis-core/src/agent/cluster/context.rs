//! # Context Store
//!
//! Manages task output storage and upstream context injection for DAG execution.
//! Uses SQLite for persistence with memory caching.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;

use tracing::{debug, info, warn};

use crate::error::{CisError, Result};

/// Context entry for a task output
#[derive(Debug, Clone)]
pub struct ContextEntry {
    /// DAG run ID
    pub run_id: String,
    /// Task ID
    pub task_id: String,
    /// Output content
    pub output: String,
    /// Exit code (if completed)
    pub exit_code: Option<i32>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Output format (text, json, markdown)
    pub format: OutputFormat,
}

/// Output format type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum OutputFormat {
    /// Plain text
    #[default]
    Text,
    /// JSON
    Json,
    /// Markdown
    Markdown,
    /// Shell output
    Shell,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Markdown => write!(f, "markdown"),
            OutputFormat::Shell => write!(f, "shell"),
        }
    }
}


/// ContextStore manages task outputs and provides upstream context injection
#[derive(Debug, Clone)]
pub struct ContextStore {
    /// In-memory cache
    cache: Arc<DashMap<String, ContextEntry>>,
    /// SQLite database path
    db_path: std::path::PathBuf,
    /// Max cache size per entry (bytes)
    max_entry_size: usize,
}

impl ContextStore {
    /// Create new ContextStore
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CisError::execution(format!("Failed to create context store dir: {}", e)))?;
        }
        
        let store = Self {
            cache: Arc::new(DashMap::new()),
            db_path,
            max_entry_size: 1024 * 1024, // 1MB default
        };
        
        // Initialize database
        store.init_db()?;
        
        info!("ContextStore initialized at {:?}", store.db_path);
        Ok(store)
    }
    
    /// Create default ContextStore in CIS data directory
    pub fn default_store() -> Result<Self> {
        let data_dir = crate::storage::Paths::data_dir();
        let db_path = data_dir.join("context_store.db");
        Self::new(db_path)
    }
    
    /// Initialize SQLite database
    fn init_db(&self) -> Result<()> {
        use rusqlite::Connection;
        
        let conn = Connection::open(&self.db_path)
            .map_err(|e| CisError::storage(format!("Failed to open context DB: {}", e)))?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS task_outputs (
                run_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                output TEXT NOT NULL,
                exit_code INTEGER,
                created_at TEXT NOT NULL,
                format TEXT NOT NULL DEFAULT 'text',
                PRIMARY KEY (run_id, task_id)
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create table: {}", e)))?;
        
        // Create index for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_run_id ON task_outputs(run_id)",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;
        
        Ok(())
    }
    
    /// Generate cache key
    fn cache_key(run_id: &str, task_id: &str) -> String {
        format!("{}:{}", run_id, task_id)
    }
    
    /// Save task output
    pub async fn save(
        &self,
        run_id: &str,
        task_id: &str,
        output: &str,
        exit_code: Option<i32>,
    ) -> Result<()> {
        let entry = ContextEntry {
            run_id: run_id.to_string(),
            task_id: task_id.to_string(),
            output: output.to_string(),
            exit_code,
            created_at: Utc::now(),
            format: OutputFormat::default(),
        };
        
        // Save to cache
        let key = Self::cache_key(run_id, task_id);
        self.cache.insert(key.clone(), entry.clone());
        
        // Save to database (truncate if too large)
        let output_to_save = if output.len() > self.max_entry_size {
            warn!("Output for {}:{} exceeds max size, truncating", run_id, task_id);
            &output[..self.max_entry_size]
        } else {
            output
        };
        
        self.save_to_db(run_id, task_id, output_to_save, exit_code).await?;
        
        debug!("Saved context for {}:{}", run_id, task_id);
        Ok(())
    }
    
    /// Save to database
    async fn save_to_db(
        &self,
        run_id: &str,
        task_id: &str,
        output: &str,
        exit_code: Option<i32>,
    ) -> Result<()> {
        use rusqlite::params;
        
        let db_path = self.db_path.clone();
        let run_id = run_id.to_string();
        let task_id = task_id.to_string();
        let output = output.to_string();
        
        let db_result = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| CisError::storage(format!("Failed to open DB: {}", e)))?;
            
            conn.execute(
                "INSERT OR REPLACE INTO task_outputs 
                 (run_id, task_id, output, exit_code, created_at, format)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    run_id,
                    task_id,
                    output,
                    exit_code,
                    Utc::now().to_rfc3339(),
                    "text"
                ],
            ).map_err(|e| CisError::storage(format!("Failed to insert: {}", e)))?;
            
            Ok::<(), CisError>(())
        })
        .await;
        
        match db_result {
            Ok(inner) => inner,
            Err(e) => Err(CisError::execution(format!("DB task failed: {}", e))),
        }
    }
    
    /// Load task output
    pub async fn load(&self, run_id: &str, task_id: &str) -> Result<String> {
        let key = Self::cache_key(run_id, task_id);
        
        // Try cache first
        if let Some(entry) = self.cache.get(&key) {
            debug!("Cache hit for {}:{}", run_id, task_id);
            return Ok(entry.output.clone());
        }
        
        // Load from database
        let output = self.load_from_db(run_id, task_id).await?;
        
        // Populate cache
        if let Ok(ref out) = output {
            let entry = ContextEntry {
                run_id: run_id.to_string(),
                task_id: task_id.to_string(),
                output: out.clone(),
                exit_code: None,
                created_at: Utc::now(),
                format: OutputFormat::default(),
            };
            self.cache.insert(key, entry);
        }
        
        output
    }
    
    /// Load from database
    async fn load_from_db(&self, run_id: &str, task_id: &str) -> Result<Result<String>> {
        use rusqlite::params;
        
        let db_path = self.db_path.clone();
        let run_id = run_id.to_string();
        let task_id = task_id.to_string();
        
        let result = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| CisError::storage(format!("Failed to open DB: {}", e)))?;
            
            let mut stmt = conn.prepare(
                "SELECT output FROM task_outputs WHERE run_id = ?1 AND task_id = ?2"
            ).map_err(|e| CisError::storage(format!("Failed to prepare: {}", e)))?;
            
            let output: Result<String> = stmt.query_row(
                params![run_id, task_id],
                |row| row.get(0),
            ).map_err(|_| CisError::not_found(format!("Output for {}:{} not found", run_id, task_id)));
            
            Ok(output)
        })
        .await
        .map_err(|e| CisError::execution(format!("DB task failed: {}", e)))?;
        
        result
    }
    
    /// Load multiple task outputs for upstream context injection
    pub async fn load_many(&self, run_id: &str, task_ids: &[String]) -> HashMap<String, String> {
        let mut result = HashMap::new();
        
        for task_id in task_ids {
            if let Ok(output) = self.load(run_id, task_id).await {
                result.insert(task_id.clone(), output);
            }
        }
        
        result
    }
    
    /// Prepare upstream context for a task (combines all dependency outputs)
    pub async fn prepare_upstream_context(
        &self,
        run_id: &str,
        task_id: &str,
        dependencies: &[String],
    ) -> String {
        if dependencies.is_empty() {
            return String::new();
        }
        
        let mut context = String::new();
        context.push_str(&format!("\n## Upstream Task Outputs for {}\n\n", task_id));
        
        let outputs = self.load_many(run_id, dependencies).await;
        
        for (dep_id, output) in outputs {
            context.push_str(&format!("### Output from {}\n\n", dep_id));
            context.push_str(&format_output(&output));
            context.push_str("\n---\n\n");
        }
        
        context
    }
    
    /// Clear cache for a specific run
    pub fn clear_run_cache(&self, run_id: &str) {
        let keys_to_remove: Vec<String> = self
            .cache
            .iter()
            .filter(|entry| entry.key().starts_with(&format!("{}:", run_id)))
            .map(|entry| entry.key().clone())
            .collect();
        
        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        
        info!("Cleared cache for run {}", run_id);
    }
    
    /// Get cache stats
    pub fn cache_stats(&self) -> (usize, usize) {
        (self.cache.len(), self.cache.len()) // (entries, approximate size)
    }
}

/// Format output for inclusion in prompt (truncate if too long)
fn format_output(output: &str) -> String {
    const MAX_LEN: usize = 10000;
    
    if output.len() > MAX_LEN {
        format!(
            "{}\n\n[... truncated, total length: {} characters ...]",
            &output[..MAX_LEN],
            output.len()
        )
    } else {
        output.to_string()
    }
}

/// Build full prompt with upstream context
pub fn build_task_prompt(base_prompt: &str, upstream_context: &str) -> String {
    if upstream_context.is_empty() {
        base_prompt.to_string()
    } else {
        format!(
            "{}\n\n{}\n",
            base_prompt,
            upstream_context
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_context_store_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContextStore::new(temp_dir.path().join("test.db")).unwrap();
        
        // Save
        store.save("run-1", "task-1", "output content", Some(0)).await.unwrap();
        
        // Load from cache
        let output = store.load("run-1", "task-1").await.unwrap();
        assert_eq!(output, "output content");
    }

    #[test]
    fn test_build_task_prompt() {
        let base = "Analyze the code";
        let upstream = "\n## Upstream\n### Output from dep\nresult";
        
        let prompt = build_task_prompt(base, upstream);
        assert!(prompt.contains("Analyze the code"));
        assert!(prompt.contains("Upstream"));
    }
}
