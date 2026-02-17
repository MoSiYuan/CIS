//! # CIS Error Types
//!
//! Centralized error handling for CIS core library.

use thiserror::Error;

/// Result type alias for CIS operations
pub type Result<T> = std::result::Result<T, CisError>;

/// Core error types for CIS
#[derive(Error, Debug)]
pub enum CisError {
    /// Sandbox-related errors
    #[error("Sandbox error: {0}")]
    Sandbox(#[from] crate::sandbox::SandboxError),

    /// Scheduler-related errors
    #[error("Scheduler error: {0}")]
    Scheduler(String),

    /// Memory-related errors
    #[error("Memory error: {0}")]
    Memory(String),

    /// Task execution errors
    #[error("Execution error: {0}")]
    Execution(String),

    /// P2P communication errors
    #[error("P2P error: {0}")]
    P2P(String),

    /// Identity/DID errors
    #[error("Identity error: {0}")]
    Identity(String),

    /// Database errors
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// I/O errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Already exists errors
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    /// Invalid input errors
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Storage errors
    #[error("Storage error: {0}")]
    Storage(String),

    /// Skill errors
    #[error("Skill error: {0}")]
    Skill(String),

    /// Vector/Embedding errors
    #[error("Vector error: {0}")]
    Vector(String),

    /// Conversation errors
    #[error("Conversation error: {0}")]
    Conversation(String),

    /// Intent errors
    #[error("Intent error: {0}")]
    Intent(String),

    /// Telemetry errors
    #[error("Telemetry error: {0}")]
    Telemetry(String),

    /// Cloud Anchor errors
    #[error("Cloud Anchor error: {0}")]
    CloudAnchor(String),

    /// Skill not found errors
    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    /// AI/LLM errors
    #[error("AI error: {0}")]
    Ai(String),

    /// WASM runtime errors
    #[error("WASM error: {0}")]
    Wasm(String),

    /// Matrix federation errors
    #[error("Matrix error: {0}")]
    Matrix(String),

    /// Federation errors
    #[error("Federation error: {0}")]
    Federation(String),

    /// Encryption/E2EE errors
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// Resource exhausted errors (P0-6: memory limits, etc.)
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    /// Generic errors with context
    #[error("{0}")]
    Other(String),
}

impl CisError {
    /// Create a new scheduler error
    pub fn scheduler(msg: impl Into<String>) -> Self {
        Self::Scheduler(msg.into())
    }

    /// Create a new memory error
    pub fn memory(msg: impl Into<String>) -> Self {
        Self::Memory(msg.into())
    }

    /// Create a new execution error
    pub fn execution(msg: impl Into<String>) -> Self {
        Self::Execution(msg.into())
    }

    /// Create a new P2P error
    pub fn p2p(msg: impl Into<String>) -> Self {
        Self::P2P(msg.into())
    }

    /// Create a new identity error
    pub fn identity(msg: impl Into<String>) -> Self {
        Self::Identity(msg.into())
    }

    /// Create a new not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a new already exists error
    pub fn already_exists(msg: impl Into<String>) -> Self {
        Self::AlreadyExists(msg.into())
    }

    /// Create a new IO error
    pub fn io(msg: impl Into<String>) -> Self {
        Self::Other(format!("IO error: {}", msg.into()))
    }

    /// Create a new invalid input error
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create a new configuration error
    pub fn configuration(msg: impl Into<String>) -> Self {
        Self::Configuration(msg.into())
    }

    /// Create a new storage error
    pub fn storage(msg: impl Into<String>) -> Self {
        Self::Storage(msg.into())
    }

    /// Create a new skill error
    pub fn skill(msg: impl Into<String>) -> Self {
        Self::Skill(msg.into())
    }

    /// Create a new resource exhausted error (P0-6)
    pub fn resource_exhausted(msg: impl Into<String>) -> Self {
        Self::ResourceExhausted(msg.into())
    }

    /// Create a new vector error
    pub fn vector(msg: impl Into<String>) -> Self {
        Self::Vector(msg.into())
    }

    /// Create a new conversation error
    pub fn conversation(msg: impl Into<String>) -> Self {
        Self::Conversation(msg.into())
    }

    /// Create a new intent error
    pub fn intent(msg: impl Into<String>) -> Self {
        Self::Intent(msg.into())
    }

    /// Create a new telemetry error
    pub fn telemetry(msg: impl Into<String>) -> Self {
        Self::Telemetry(msg.into())
    }

    /// Create a new Cloud Anchor error
    pub fn cloud_anchor(msg: impl Into<String>) -> Self {
        Self::CloudAnchor(msg.into())
    }

    /// Create a new skill not found error
    pub fn skill_not_found(msg: impl Into<String>) -> Self {
        Self::SkillNotFound(msg.into())
    }

    /// Create a new AI error
    pub fn ai(msg: impl Into<String>) -> Self {
        Self::Ai(msg.into())
    }

    /// Create a new WASM error
    pub fn wasm(msg: impl Into<String>) -> Self {
        Self::Wasm(msg.into())
    }

    /// Create a new Matrix error
    pub fn matrix(msg: impl Into<String>) -> Self {
        Self::Matrix(msg.into())
    }

    /// Create a new federation error
    pub fn federation(msg: impl Into<String>) -> Self {
        Self::Federation(msg.into())
    }

    /// Create a new generic/other error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Create a new network error (alias for P2P)
    pub fn network(msg: impl Into<String>) -> Self {
        Self::P2P(msg.into())
    }

    /// Create a new crypto error
    pub fn crypto(msg: impl Into<String>) -> Self {
        Self::Other(format!("Crypto error: {}", msg.into()))
    }

    /// Create a new serialization error (manual)
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Other(format!("Serialization error: {}", msg.into()))
    }

    /// Create a new invalid state error
    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::Other(format!("Invalid state: {}", msg.into()))
    }

    /// Create a new internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Other(format!("Internal error: {}", msg.into()))
    }

    /// Create a new encryption error
    pub fn encryption(msg: impl Into<String>) -> Self {
        Self::Encryption(msg.into())
    }
}
