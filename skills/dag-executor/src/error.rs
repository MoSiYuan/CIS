//! DAG Executor 错误类型

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DagExecutorError {
    #[error("Worker spawn failed: {0}")]
    SpawnFailed(String),

    #[error("Task dispatch failed: {0}")]
    DispatchFailed(String),

    #[error("Worker not found: {0}")]
    WorkerNotFound(String),

    #[error("Worker died: {0}")]
    WorkerDied(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("CIS error: {0}")]
    Cis(#[from] cis_core::error::CisError),

    #[error("Matrix room error: {0}")]
    MatrixRoom(String),
}

pub type Result<T> = std::result::Result<T, DagExecutorError>;
