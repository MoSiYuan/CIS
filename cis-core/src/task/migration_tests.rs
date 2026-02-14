//! # Task Migrator 单元测试
//!
//! 全面测试数据迁移功能，包括 TOML 解析、任务迁移、团队注册、验证和回滚。

use crate::task::migration::*;
use crate::task::models::*;
use crate::task::db::{DatabasePool, initialize_schema};
use crate::task::repository::TaskRepository;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// 辅助函数
// ============================================================================

/// 创建测试用的内存数据库
async fn create_test_db() -> (Arc<DatabasePool>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let pool = Arc::new(DatabasePool::new(db_path, 5).unwrap());

    // 初始化 schema
    let conn = pool.acquire().await.unwrap();
    initialize_schema(&conn).unwrap();

    (pool, temp_dir)
}

/// 创建测试用的 TOML 文件
fn create_test_toml_file(dir: &PathBuf, filename: &str, content: &str) -> PathBuf {
    let file_path = dir.join(filename);
    let mut file = File::create(&file_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    file_path
}

/// 有效的任务 TOML 模板
fn valid_task_toml() -> &'static str {
    r#"
[[task]]
id = "TASK-001"
name = "Test Task 1"
type = "ModuleRefactoring"
priority = "p0"
effort_person_days = 5.0
prompt = "This is a test prompt"

[[task.dependencies]]
task_id = "TASK-002"

[[task.capabilities]]
capability = "CodeReview"

[[task.capabilities]]
capability = "TestWriting"

[[task.context_files]]
path = "/path/to/file.rs"
description = "Main implementation file"

[[task]]
id = "TASK-002"
name = "Test Task 2"
type = "CodeReview"
priority = "p1"
prompt = "Review the code"
"#
}

/// 有效的团队 TOML 模板
fn valid_team_toml() -> &'static str {
    r#"
[[team]]
id = "TEAM-001"
name = "Test Team 1"
runtime = "claude"
max_concurrent_tasks = 3

[[team.capabilities]]
capability = "CodeReview"

[[team.capabilities]]
capability = "ModuleRefactoring"

[[team.members]]
name = "Agent-1"
type = "claude"

[[team.members]]
name = "Agent-2"
type = "opencode"
"#
}

/// 完整的配置 TOML（包含任务和团队）
fn full_config_toml() -> &'static str {
    r#"
[[task]]
id = "V-1"
name = "CLI Architecture Fix"
type = "ModuleRefactoring"
priority = "p0"
effort_person_days = 5.0
prompt = "Fix CLI architecture"

[[task.capabilities]]
capability = "CodeReview"

[[task]]
id = "V-2"
name = "Code Review"
type = "CodeReview"
priority = "p1"
prompt = "Review the implementation"

[[task.dependencies]]
task_id = "V-1"

[[team]]
id = "Team-V-CLI"
name = "Team V - CLI"
runtime = "claude"
max_concurrent_tasks = 3

[[team.capabilities]]
capability = "CodeReview"

[[team.members]]
name = "Claude-Agent"
type = "claude"
"#
}

// ============================================================================
// TOML 解析测试
// ============================================================================

#[test]
fn test_parse_valid_task() {
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "ModuleRefactoring"
priority = "p0"
effort_person_days = 3.5
prompt = "Test prompt"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.task.len(), 1);
    assert_eq!(config.task[0].task_id, "TASK-001");
    assert_eq!(config.task[0].name, "Test Task");
    assert_eq!(config.task[0].task_type, "ModuleRefactoring");
    assert_eq!(config.task[0].priority, "p0");
    assert_eq!(config.task[0].effort_person_days, Some(3.5));
    assert_eq!(config.task[0].prompt, "Test prompt");
}

#[test]
fn test_parse_valid_team() {
    let toml_content = r#"
[[team]]
id = "TEAM-001"
name = "Test Team"
runtime = "claude"
max_concurrent_tasks = 5
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.team.len(), 1);
    assert_eq!(config.team[0].team_id, "TEAM-001");
    assert_eq!(config.team[0].name, "Test Team");
    assert_eq!(config.team[0].runtime, "claude");
    assert_eq!(config.team[0].max_concurrent_tasks, 5);
}

#[test]
fn test_parse_task_with_dependencies() {
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "CodeReview"
priority = "p1"
prompt = "Review code"

[[task.dependencies]]
task_id = "TASK-002"

[[task.dependencies]]
task_id = "TASK-003"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.task[0].dependencies.len(), 2);
    assert_eq!(config.task[0].dependencies[0].task_id_ref, Some("TASK-002".to_string()));
    assert_eq!(config.task[0].dependencies[1].task_id_ref, Some("TASK-003".to_string()));
}

#[test]
fn test_parse_task_with_capabilities() {
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "ModuleRefactoring"
priority = "p0"
prompt = "Refactor module"

[[task.capabilities]]
capability = "CodeReview"

[[task.capabilities]]
capability = "TestWriting"

[[task.capabilities]]
capability = "Documentation"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.task[0].capabilities.len(), 3);
    assert_eq!(config.task[0].capabilities[0].capability, "CodeReview");
    assert_eq!(config.task[0].capabilities[1].capability, "TestWriting");
    assert_eq!(config.task[0].capabilities[2].capability, "Documentation");
}

#[test]
fn test_parse_task_with_context_files() {
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "PerformanceOptimization"
priority = "p2"
prompt = "Optimize performance"

[[task.context_files]]
path = "/src/main.rs"
description = "Main entry point"

[[task.context_files]]
path = "/src/lib.rs"
description = "Library implementation"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.task[0].context_files.len(), 2);
    assert_eq!(config.task[0].context_files[0].path, "/src/main.rs");
    assert_eq!(config.task[0].context_files[0].description, "Main entry point");
    assert_eq!(config.task[0].context_files[1].path, "/src/lib.rs");
    assert_eq!(config.task[0].context_files[1].description, "Library implementation");
}

#[test]
fn test_parse_team_with_members() {
    let toml_content = r#"
[[team]]
id = "TEAM-001"
name = "Test Team"
runtime = "claude"
max_concurrent_tasks = 3

[[team.members]]
name = "Agent-1"
type = "claude"

[[team.members]]
name = "Agent-2"
type = "opencode"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.team[0].members.len(), 2);
    assert_eq!(config.team[0].members[0].name, "Agent-1");
    assert_eq!(config.team[0].members[0].member_type, "claude");
    assert_eq!(config.team[0].members[1].name, "Agent-2");
    assert_eq!(config.team[0].members[1].member_type, "opencode");
}

#[test]
fn test_parse_multiple_tasks_and_teams() {
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Task 1"
type = "ModuleRefactoring"
priority = "p0"
prompt = "First task"

[[task]]
id = "TASK-002"
name = "Task 2"
type = "CodeReview"
priority = "p1"
prompt = "Second task"

[[team]]
id = "TEAM-001"
name = "Team 1"
runtime = "claude"
max_concurrent_tasks = 3

[[team]]
id = "TEAM-002"
name = "Team 2"
runtime = "opencode"
max_concurrent_tasks = 5
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    assert_eq!(config.task.len(), 2);
    assert_eq!(config.team.len(), 2);
}

#[test]
fn test_parse_missing_required_field() {
    // 缺少 required 字段 'prompt'
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "ModuleRefactoring"
priority = "p0"
"#;

    let result: Result<TomlConfig, _> = toml::from_str(toml_content);
    assert!(result.is_err(), "Should fail when missing required field 'prompt'");
}

#[test]
fn test_parse_invalid_task_type() {
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "InvalidType"
priority = "p0"
prompt = "Test"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();

    // 在创建迁移器时测试类型解析
    let (pool, _temp_dir) = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(create_test_db());
    let migrator = TaskMigrator::new(pool);

    let result = migrator.parse_task_type(&config.task[0].task_type);
    assert!(result.is_err(), "Should fail with unknown task type");
    assert!(result.unwrap_err().to_string().contains("未知的任务类型"));
}

#[test]
fn test_parse_invalid_priority() {
    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "ModuleRefactoring"
priority = "p5"
prompt = "Test"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();

    // 在创建迁移器时测试优先级解析
    let (pool, _temp_dir) = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(create_test_db());
    let migrator = TaskMigrator::new(pool);

    let result = migrator.parse_priority(&config.task[0].priority);
    assert!(result.is_err(), "Should fail with unknown priority");
    assert!(result.unwrap_err().to_string().contains("未知的优先级"));
}

#[test]
fn test_parse_priority_case_insensitive() {
    let (pool, _temp_dir) = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(create_test_db());
    let migrator = TaskMigrator::new(pool);

    // 测试大小写不敏感
    assert!(matches!(migrator.parse_priority("p0").unwrap(), TaskPriority::P0));
    assert!(matches!(migrator.parse_priority("P0").unwrap(), TaskPriority::P0));
    assert!(matches!(migrator.parse_priority("p1").unwrap(), TaskPriority::P1));
    assert!(matches!(migrator.parse_priority("P1").unwrap(), TaskPriority::P1));
    assert!(matches!(migrator.parse_priority("p2").unwrap(), TaskPriority::P2));
    assert!(matches!(migrator.parse_priority("P2").unwrap(), TaskPriority::P2));
    assert!(matches!(migrator.parse_priority("p3").unwrap(), TaskPriority::P3));
    assert!(matches!(migrator.parse_priority("P3").unwrap(), TaskPriority::P3));
}

// ============================================================================
// 任务迁移测试
// ============================================================================

#[tokio::test]
async fn test_migrate_single_task() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Single Task"
type = "ModuleRefactoring"
priority = "p0"
effort_person_days = 2.5
prompt = "Migrate this task"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 1);
    assert_eq!(result.tasks_failed, 0);
    assert_eq!(result.teams_registered, 0);
    assert!(result.warnings.is_empty());

    // 验证数据库中的任务
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_id, "TASK-001");
    assert_eq!(tasks[0].name, "Single Task");
    assert!(matches!(tasks[0].task_type, TaskType::ModuleRefactoring));
    assert!(matches!(tasks[0].priority, TaskPriority::P0));
    assert_eq!(tasks[0].estimated_effort_days, Some(2.5));
    assert_eq!(tasks[0].status, TaskStatus::Pending);
}

#[tokio::test]
async fn test_migrate_multiple_tasks() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let config: TomlConfig = toml::from_str(valid_task_toml()).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 2);
    assert_eq!(result.tasks_failed, 0);

    // 验证数据库
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 2);
}

#[tokio::test]
async fn test_migrate_task_with_dependencies() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Task with deps"
type = "CodeReview"
priority = "p1"
prompt = "Review code"

[[task.dependencies]]
task_id = "TASK-002"

[[task.dependencies]]
task_id = "TASK-003"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 1);

    // 验证依赖关系
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks[0].dependencies.len(), 2);
    assert_eq!(tasks[0].dependencies[0], "TASK-002");
    assert_eq!(tasks[0].dependencies[1], "TASK-003");
}

#[tokio::test]
async fn test_migrate_task_with_capabilities() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Task with capabilities"
type = "TestWriting"
priority = "p2"
prompt = "Write tests"

[[task.capabilities]]
capability = "CodeReview"

[[task.capabilities]]
capability = "TestWriting"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 1);

    // 验证 capabilities 存储在 context_variables 中
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    let context_vars = tasks[0].context_variables.as_object().unwrap();
    assert!(context_vars.contains_key("capabilities"));

    let capabilities = context_vars.get("capabilities").unwrap().as_array().unwrap();
    assert_eq!(capabilities.len(), 2);
    assert_eq!(capabilities[0], "CodeReview");
    assert_eq!(capabilities[1], "TestWriting");
}

#[tokio::test]
async fn test_migrate_task_with_context_files() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Task with context files"
type = "Documentation"
priority = "p3"
prompt = "Write documentation"

[[task.context_files]]
path = "/src/lib.rs"
description = "Library file"

[[task.context_files]]
path = "/README.md"
description = "Project README"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 1);

    // 验证 context_files 存储在 context_variables 中
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    let context_vars = tasks[0].context_variables.as_object().unwrap();
    assert!(context_vars.contains_key("context_files"));

    let context_files = context_vars.get("context_files").unwrap().as_array().unwrap();
    assert_eq!(context_files.len(), 2);
    assert_eq!(context_files[0]["path"], "/src/lib.rs");
    assert_eq!(context_files[0]["description"], "Library file");
    assert_eq!(context_files[1]["path"], "/README.md");
    assert_eq!(context_files[1]["description"], "Project README");
}

#[tokio::test]
async fn test_migrate_from_toml_file() {
    let (pool, temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 创建临时 TOML 文件
    let toml_path = create_test_toml_file(temp_dir.path(), "test.toml", valid_task_toml());

    let result = migrator.migrate_from_toml_file(&toml_path).await.unwrap();

    assert_eq!(result.tasks_migrated, 2);
    assert_eq!(result.tasks_failed, 0);

    // 验证数据库
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 2);
}

#[tokio::test]
async fn test_migrate_from_nonexistent_file() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool);

    let result = migrator.migrate_from_toml_file("/nonexistent/file.toml").await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("无法读取 TOML 文件"));
}

#[tokio::test]
async fn test_migrate_invalid_toml_syntax() {
    let (pool, temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 创建语法错误的 TOML 文件
    let invalid_toml = r#"
[[task]
id = "TASK-001"
name = "Invalid TOML
"#; // 缺少结束引号

    let toml_path = create_test_toml_file(temp_dir.path(), "invalid.toml", invalid_toml);

    let result = migrator.migrate_from_toml_file(&toml_path).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("解析 TOML 文件失败"));
}

// ============================================================================
// 团队注册测试
// ============================================================================

#[tokio::test]
async fn test_register_single_team() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let config: TomlConfig = toml::from_str(valid_team_toml()).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 0);
    assert_eq!(result.teams_registered, 1);

    // 验证团队注册记录
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_id, "TEAM-REGISTRATION-TEAM-001");
    assert_eq!(tasks[0].name, "Team Registration: Test Team 1");
    assert!(matches!(tasks[0].task_type, TaskType::Documentation));
    assert_eq!(tasks[0].status, TaskStatus::Completed);
    assert_eq!(tasks[0].assigned_team_id, Some("TEAM-001".to_string()));
}

#[tokio::test]
async fn test_register_team_with_capabilities() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[team]]
id = "TEAM-001"
name = "Test Team"
runtime = "claude"
max_concurrent_tasks = 3

[[team.capabilities]]
capability = "CodeReview"

[[team.capabilities]]
capability = "ModuleRefactoring"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.teams_registered, 1);

    // 验证 capabilities 存储在 context_variables 中
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    let team_data = tasks[0].context_variables.as_object().unwrap();
    assert_eq!(team_data.get("team_id").unwrap(), "TEAM-001");
    assert_eq!(team_data.get("name").unwrap(), "Test Team");
    assert_eq!(team_data.get("runtime").unwrap(), "claude");
    assert_eq!(team_data.get("max_concurrent_tasks").unwrap(), 3);

    let capabilities = team_data.get("capabilities").unwrap().as_array().unwrap();
    assert_eq!(capabilities.len(), 2);
}

#[tokio::test]
async fn test_register_team_with_members() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[team]]
id = "TEAM-001"
name = "Test Team"
runtime = "claude"
max_concurrent_tasks = 3

[[team.members]]
name = "Agent-1"
type = "claude"

[[team.members]]
name = "Agent-2"
type = "opencode"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.teams_registered, 1);

    // 验证 members 存储
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    let team_data = tasks[0].context_variables.as_object().unwrap();
    let members = team_data.get("members").unwrap().as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["name"], "Agent-1");
    assert_eq!(members[0]["type"], "claude");
    assert_eq!(members[1]["name"], "Agent-2");
    assert_eq!(members[1]["type"], "opencode");
}

// ============================================================================
// 批量迁移测试
// ============================================================================

#[tokio::test]
async fn test_migrate_from_directory() {
    let (pool, temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 创建多个 TOML 文件
    create_test_toml_file(temp_dir.path(), "tasks1.toml", valid_task_toml());
    create_test_toml_file(temp_dir.path(), "teams1.toml", valid_team_toml());
    create_test_toml_file(temp_dir.path(), "full.toml", full_config_toml());

    let result = migrator.migrate_from_directory(temp_dir.path()).await.unwrap();

    assert_eq!(result.tasks_migrated, 6); // 2 + 2 + 2
    assert_eq!(result.teams_registered, 3); // 1 + 1 + 1
    assert_eq!(result.tasks_failed, 0);

    // 验证数据库
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 9); // 6 tasks + 3 team registrations
}

#[tokio::test]
async fn test_migrate_from_directory_with_invalid_files() {
    let (pool, temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 创建有效和无效的文件
    create_test_toml_file(temp_dir.path(), "valid.toml", valid_task_toml());
    create_test_toml_file(temp_dir.path(), "invalid.toml", "invalid toml content {");
    create_test_toml_file(temp_dir.path(), "not_toml.txt", "not a toml file");

    let result = migrator.migrate_from_directory(temp_dir.path()).await.unwrap();

    assert_eq!(result.tasks_migrated, 2);
    assert_eq!(result.tasks_failed, 0);
    assert_eq!(result.warnings.len(), 1); // invalid.toml 产生警告
    assert!(result.warnings[0].contains("invalid.toml"));
}

#[tokio::test]
async fn test_migrate_full_config() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let config: TomlConfig = toml::from_str(full_config_toml()).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 2);
    assert_eq!(result.teams_registered, 1);
    assert_eq!(result.tasks_failed, 0);
    assert!(result.warnings.is_empty());

    // 验证任务和团队都正确迁移
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 3); // 2 tasks + 1 team registration
}

// ============================================================================
// 验证测试
// ============================================================================

#[tokio::test]
async fn test_verify_migration_statistics() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 迁移数据
    let config: TomlConfig = toml::from_str(full_config_toml()).unwrap();
    migrator.migrate_from_config(&config).await.unwrap();

    // 验证
    let verification = migrator.verify_migration().await.unwrap();

    assert_eq!(verification.total_tasks, 3); // 2 tasks + 1 team registration
    assert_eq!(verification.pending_tasks, 2); // 只有 2 个普通任务是 Pending

    // 验证任务类型分布
    assert!(verification.tasks_by_type.contains_key("ModuleRefactoring"));
    assert!(verification.tasks_by_type.contains_key("CodeReview"));
    assert!(verification.tasks_by_type.contains_key("Documentation"));

    // 验证优先级分布
    assert!(verification.tasks_by_priority.contains_key("P0"));
    assert!(verification.tasks_by_priority.contains_key("P1"));

    // 验证状态分布
    assert!(verification.tasks_by_status.contains_key("Pending"));
    assert!(verification.tasks_by_status.contains_key("Completed"));
}

#[tokio::test]
async fn test_verify_task_type_distribution() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Refactoring"
type = "ModuleRefactoring"
priority = "p0"
prompt = "Refactor"

[[task]]
id = "TASK-002"
name = "Review 1"
type = "CodeReview"
priority = "p1"
prompt = "Review 1"

[[task]]
id = "TASK-003"
name = "Review 2"
type = "CodeReview"
priority = "p1"
prompt = "Review 2"

[[task]]
id = "TASK-004"
name = "Tests"
type = "TestWriting"
priority = "p2"
prompt = "Write tests"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    migrator.migrate_from_config(&config).await.unwrap();

    let verification = migrator.verify_migration().await.unwrap();

    assert_eq!(verification.total_tasks, 4);
    assert_eq!(
        *verification.tasks_by_type.get("ModuleRefactoring").unwrap_or(&0),
        1
    );
    assert_eq!(
        *verification.tasks_by_type.get("CodeReview").unwrap_or(&0),
        2
    );
    assert_eq!(
        *verification.tasks_by_type.get("TestWriting").unwrap_or(&0),
        1
    );
}

#[tokio::test]
async fn test_verify_priority_distribution() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Task 1"
type = "ModuleRefactoring"
priority = "p0"
prompt = "T1"

[[task]]
id = "TASK-002"
name = "Task 2"
type = "CodeReview"
priority = "p1"
prompt = "T2"

[[task]]
id = "TASK-003"
name = "Task 3"
type = "TestWriting"
priority = "p1"
prompt = "T3"

[[task]]
id = "TASK-004"
name = "Task 4"
type = "Documentation"
priority = "p2"
prompt = "T4"

[[task]]
id = "TASK-005"
name = "Task 5"
type = "Documentation"
priority = "p3"
prompt = "T5"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    migrator.migrate_from_config(&config).await.unwrap();

    let verification = migrator.verify_migration().await.unwrap();

    assert_eq!(verification.total_tasks, 5);
    assert_eq!(*verification.tasks_by_priority.get("P0").unwrap_or(&0), 1);
    assert_eq!(*verification.tasks_by_priority.get("P1").unwrap_or(&0), 2);
    assert_eq!(*verification.tasks_by_priority.get("P2").unwrap_or(&0), 1);
    assert_eq!(*verification.tasks_by_priority.get("P3").unwrap_or(&0), 1);
}

// ============================================================================
// 回滚测试
// ============================================================================

#[tokio::test]
async fn test_rollback_migration_by_timestamp() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 迁移任务
    let config: TomlConfig = toml::from_str(valid_task_toml()).unwrap();
    let before_migration = chrono::Utc::now().timestamp() - 1;
    migrator.migrate_from_config(&config).await.unwrap();

    // 验证任务已迁移
    let repository = TaskRepository::new(pool.clone());
    let tasks_before = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks_before.len(), 2);

    // 回滚
    let deleted_count = migrator.rollback_migration(before_migration).await.unwrap();
    assert_eq!(deleted_count, 2);

    // 验证任务已删除
    let tasks_after = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks_after.len(), 0);
}

#[tokio::test]
async fn test_partial_rollback() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 第一次迁移
    let config1: TomlConfig = toml::from_str(
        r#"
[[task]]
id = "TASK-001"
name = "First Task"
type = "ModuleRefactoring"
priority = "p0"
prompt = "First"
"#
    ).unwrap();

    migrator.migrate_from_config(&config1).await.unwrap();
    let midpoint = chrono::Utc::now().timestamp();

    // 等待至少 1 秒以确保时间戳不同
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // 第二次迁移
    let config2: TomlConfig = toml::from_str(
        r#"
[[task]]
id = "TASK-002"
name = "Second Task"
type = "CodeReview"
priority = "p1"
prompt = "Second"
"#
    ).unwrap();

    migrator.migrate_from_config(&config2).await.unwrap();

    // 验证所有任务存在
    let repository = TaskRepository::new(pool.clone());
    let tasks_before = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks_before.len(), 2);

    // 只回滚第二次迁移
    let deleted_count = migrator.rollback_migration(midpoint).await.unwrap();
    assert_eq!(deleted_count, 1);

    // 验证只有第二个任务被删除
    let tasks_after = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks_after.len(), 1);
    assert_eq!(tasks_after[0].task_id, "TASK-001");
}

#[tokio::test]
async fn test_rollback_nonexistent_tasks() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 尝试回滚到未来时间戳（不会有任务被删除）
    let future_timestamp = chrono::Utc::now().timestamp() + 1000;
    let deleted_count = migrator.rollback_migration(future_timestamp).await.unwrap();

    assert_eq!(deleted_count, 0);
}

#[tokio::test]
async fn test_rollback_all_tasks() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 迁移多个任务
    let config: TomlConfig = toml::from_str(full_config_toml()).unwrap();
    migrator.migrate_from_config(&config).await.unwrap();

    let repository = TaskRepository::new(pool.clone());
    let tasks_before = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks_before.len(), 3); // 2 tasks + 1 team registration

    // 回滚所有任务（使用很早的时间戳）
    let ancient_timestamp = 0;
    let deleted_count = migrator.rollback_migration(ancient_timestamp).await.unwrap();
    assert_eq!(deleted_count, 3);

    let tasks_after = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks_after.len(), 0);
}

// ============================================================================
// 错误处理测试
// ============================================================================

#[tokio::test]
async fn test_migrate_with_duplicate_task_id() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    // 第一次迁移
    let config1: TomlConfig = toml::from_str(
        r#"
[[task]]
id = "TASK-001"
name = "First"
type = "ModuleRefactoring"
priority = "p0"
prompt = "First"
"#
    ).unwrap();

    migrator.migrate_from_config(&config1).await.unwrap();

    // 第二次迁移，使用相同的 task_id
    let config2: TomlConfig = toml::from_str(
        r#"
[[task]]
id = "TASK-001"
name = "Duplicate"
type = "CodeReview"
priority = "p1"
prompt = "Duplicate"
"#
    ).unwrap();

    let result = migrator.migrate_from_config(&config2).await.unwrap();

    // 应该失败
    assert_eq!(result.tasks_migrated, 0);
    assert_eq!(result.tasks_failed, 1);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("TASK-001"));
}

#[tokio::test]
async fn test_migrate_mixed_valid_invalid_tasks() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "Valid Task"
type = "ModuleRefactoring"
priority = "p0"
prompt = "Valid task"

[[task]]
id = "TASK-002"
name = "Invalid Type"
type = "InvalidType"
priority = "p1"
prompt = "Invalid type"

[[task]]
id = "TASK-003"
name = "Another Valid"
type = "CodeReview"
priority = "p2"
prompt = "Also valid"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    let result = migrator.migrate_from_config(&config).await.unwrap();

    assert_eq!(result.tasks_migrated, 2);
    assert_eq!(result.tasks_failed, 1);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("TASK-002"));

    // 验证只有有效任务被迁移
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 2);
}

#[tokio::test]
async fn test_migrate_empty_config() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let empty_config = TomlConfig {
        task: vec![],
        team: vec![],
    };

    let result = migrator.migrate_from_config(&empty_config).await.unwrap();

    assert_eq!(result.tasks_migrated, 0);
    assert_eq!(result.tasks_failed, 0);
    assert_eq!(result.teams_registered, 0);
    assert!(result.warnings.is_empty());

    // 验证数据库为空
    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 0);
}

#[tokio::test]
async fn test_migrate_from_empty_directory() {
    let (pool, temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let result = migrator.migrate_from_directory(temp_dir.path()).await.unwrap();

    assert_eq!(result.tasks_migrated, 0);
    assert_eq!(result.tasks_failed, 0);
    assert!(result.warnings.is_empty());
}

// ============================================================================
// 元数据测试
// ============================================================================

#[tokio::test]
async fn test_task_metadata_after_migration() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let config: TomlConfig = toml::from_str(
        r#"
[[task]]
id = "TASK-001"
name = "Test Task"
type = "ModuleRefactoring"
priority = "p0"
prompt = "Test"
"#
    ).unwrap();

    migrator.migrate_from_config(&config).await.unwrap();

    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    // 验证元数据
    assert!(tasks[0].metadata.is_some());
    let metadata = tasks[0].metadata.as_ref().unwrap().as_object().unwrap();
    assert_eq!(metadata.get("migration_source").unwrap(), "toml");
    assert!(metadata.contains_key("migrated_at"));

    // 验证描述
    assert_eq!(tasks[0].description, Some("从 TOML 迁移的任务: Test Task".to_string()));
}

#[tokio::test]
async fn test_team_registration_metadata() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let config: TomlConfig = toml::from_str(valid_team_toml()).unwrap();
    migrator.migrate_from_config(&config).await.unwrap();

    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    let task = &tasks[0];

    // 验证任务状态
    assert_eq!(task.status, TaskStatus::Completed);
    assert!(task.assigned_team_id.is_some());
    assert!(task.assigned_at.is_some());
    assert!(task.started_at.is_some());
    assert!(task.completed_at.is_some());
    assert_eq!(task.duration_seconds, Some(0.0));

    // 验证元数据
    assert!(task.metadata.is_some());
    let metadata = task.metadata.as_ref().unwrap().as_object().unwrap();
    assert_eq!(metadata.get("registration_type").unwrap(), "team");
    assert!(metadata.contains_key("migrated_at"));
}

// ============================================================================
// 工时计算测试
// ============================================================================

#[tokio::test]
async fn test_effort_conversion_to_hours() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "5 Day Task"
type = "ModuleRefactoring"
priority = "p0"
effort_person_days = 5.0
prompt = "Test"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    migrator.migrate_from_config(&config).await.unwrap();

    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    // 5 人天 = 40 小时 = 144000 秒
    assert_eq!(tasks[0].estimated_effort_days, Some(5.0));
    assert_eq!(tasks[0].duration_seconds, Some(40.0 * 3600.0));
}

#[tokio::test]
async fn test_task_without_effort_estimate() {
    let (pool, _temp_dir) = create_test_db().await;
    let migrator = TaskMigrator::new(pool.clone());

    let toml_content = r#"
[[task]]
id = "TASK-001"
name = "No Effort Task"
type = "CodeReview"
priority = "p1"
prompt = "Review"
"#;

    let config: TomlConfig = toml::from_str(toml_content).unwrap();
    migrator.migrate_from_config(&config).await.unwrap();

    let repository = TaskRepository::new(pool);
    let tasks = repository.query(TaskFilter::default()).await.unwrap();
    assert_eq!(tasks.len(), 1);

    // 没有 effort_person_days，duration_seconds 应该为 None
    assert_eq!(tasks[0].estimated_effort_days, None);
    assert_eq!(tasks[0].duration_seconds, None);
}
