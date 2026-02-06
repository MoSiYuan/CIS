//! Shared types for capability layer

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Caller type for tracking invocation source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallerType {
    Skill,
    Mcp,
    Http,
    Cli,
}

/// Unified execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub skill_name: String,
    pub params: serde_json::Value,
    pub context: ProjectContext,
    pub caller: CallerType,
}

impl ExecutionRequest {
    pub fn new(skill_name: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            skill_name: skill_name.into(),
            params,
            context: ProjectContext::default(),
            caller: CallerType::Skill,
        }
    }

    pub fn with_context(mut self, context: ProjectContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_caller(mut self, caller: CallerType) -> Self {
        self.caller = caller;
        self
    }
}

/// Unified execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub output: String,
    pub exit_code: Option<i32>,
    pub work_dir: PathBuf,
    pub duration_ms: u64,
    pub metadata: HashMap<String, String>,
}

impl ExecutionResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            exit_code: Some(0),
            work_dir: std::env::current_dir().unwrap_or_default(),
            duration_ms: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn error(output: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            exit_code: Some(1),
            work_dir: std::env::current_dir().unwrap_or_default(),
            duration_ms: 0,
            metadata: HashMap::new(),
        }
    }
}

/// Project context information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project_root: Option<PathBuf>,
    pub project_type: Option<String>,
    pub package_manager: Option<String>,
    pub git_branch: Option<String>,
    pub git_status: Option<GitStatus>,
    pub detected_files: Vec<String>,
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub ahead: i32,
    pub behind: i32,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
}

/// Memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub value: String,
    pub scope: MemoryScope,
    pub project_path: Option<PathBuf>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryScope {
    Global,
    Project,
    Session,
}

/// Skill metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub category: String,
    pub parameters: Vec<SkillParameter>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub param_type: String,
    pub default_value: Option<serde_json::Value>,
}

/// Skill match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMatch {
    pub skill_name: String,
    pub confidence: f32,
    pub reason: String,
    pub suggested_params: Option<serde_json::Value>,
}

/// Capability error
#[derive(thiserror::Error, Debug)]
pub enum CapabilityError {
    #[error("Skill not found: {0}")]
    SkillNotFound(String),
    
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Memory error: {0}")]
    MemoryError(String),
    
    #[error("Context detection failed: {0}")]
    ContextError(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    
    #[error("Other: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CapabilityError>;
