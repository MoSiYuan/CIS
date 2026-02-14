//! # Task 模块
//!
//! 提供任务管理、数据库存储、DAG 编排、向量索引等功能。
//!
//! ## 主要组件
//!
//! - `db`: 数据库连接池和 Schema 管理
//! - `models`: 任务相关的数据模型
//! - `repository`: 任务仓储（CRUD 操作）
//! - `session`: Agent Session 仓储（支持复用）
//! - `dag`: DAG 构建器和依赖解析
//! - `vector`: Task 向量索引，支持多字段语义搜索
//! - `migration`: 数据迁移工具（TOML → SQLite）

pub mod db;
pub mod models;
pub mod repository;
pub mod session;
pub mod dag;
pub mod vector;
pub mod manager;
pub mod migration;
#[cfg(test)]
pub mod migration_tests;

pub use db::{create_database_pool, DatabasePool, DatabaseStats, initialize_schema, vacuum_database};
pub use models::*;
pub use repository::TaskRepository;
pub use session::{AgentRepository, SessionRepository};
pub use dag::{Dag, DagBuilder, DagNode, DagError};
pub use manager::{
    TaskManager, TaskAssignment, LevelAssignment, ExecutionPlan,
    TaskOrchestrationResult, OrchestrationStatus, TeamRules, TaskQueryFilter,
    TaskStatistics,
};
pub use migration::{
    TaskMigrator, MigrationStats, MigrationVerification,
    TomlTask, TomlTeam, TomlConfig,
};
