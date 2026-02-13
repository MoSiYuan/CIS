//! # 数据迁移工具
//!
//! 提供从 TOML 格式迁移到 SQLite 数据库的功能。

use super::models::*;
use super::repository::TaskRepository;
use super::db::DatabasePool;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn, debug};

/// TOML 任务定义
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TomlTask {
    #[serde(rename = "id")]
    pub task_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub priority: String,
    #[serde(rename = "effort_person_days")]
    pub effort_person_days: Option<f64>,
    pub prompt: String,

    #[serde(default)]
    pub dependencies: Vec<TomlDependency>,

    #[serde(default)]
    pub capabilities: Vec<TomlCapability>,

    #[serde(default)]
    pub context_files: Vec<TomlContextFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TomlDependency {
    #[serde(rename = "task_id")]
    pub task_id_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TomlCapability {
    pub capability: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TomlContextFile {
    pub path: String,
    pub description: String,
}

/// TOML Team 定义
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TomlTeam {
    #[serde(rename = "id")]
    pub team_id: String,
    pub name: String,
    pub runtime: String,
    #[serde(rename = "max_concurrent_tasks")]
    pub max_concurrent_tasks: i32,

    #[serde(default)]
    pub capabilities: Vec<TomlCapability>,

    #[serde(default)]
    pub members: Vec<TomlMember>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TomlMember {
    pub name: String,
    #[serde(rename = "type")]
    pub member_type: String,
}

/// 完整的 TOML 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TomlConfig {
    #[serde(default)]
    pub task: Vec<TomlTask>,

    #[serde(default)]
    pub team: Vec<TomlTeam>,
}

/// 迁移统计
#[derive(Debug, Clone, Default)]
pub struct MigrationStats {
    pub tasks_migrated: usize,
    pub teams_registered: usize,
    pub tasks_failed: usize,
    pub warnings: Vec<String>,
}

impl MigrationStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// 数据迁移器
pub struct TaskMigrator {
    db: Arc<DatabasePool>,
}

impl TaskMigrator {
    /// 创建新的迁移器
    pub fn new(db: Arc<DatabasePool>) -> Self {
        Self { db }
    }

    /// 从 TOML 文件迁移任务
    pub async fn migrate_from_toml_file<P: AsRef<Path>>(
        &self,
        toml_path: P,
    ) -> Result<MigrationStats> {
        let path = toml_path.as_ref();

        info!("开始从 TOML 文件迁移任务: {}", path.display());

        // 读取 TOML 文件
        let toml_content = fs::read_to_string(path)
            .with_context(|| format!("无法读取 TOML 文件: {}", path.display()))?;

        // 解析 TOML
        let config: TomlConfig = toml::from_str(&toml_content)
            .with_context(|| format!("解析 TOML 文件失败: {}", path.display()))?;

        // 执行迁移
        self.migrate_from_config(&config).await
    }

    /// 从 TOML 配置迁移
    pub async fn migrate_from_config(&self, config: &TomlConfig) -> Result<MigrationStats> {
        let mut stats = MigrationStats::new();
        let repository = TaskRepository::new(self.db.clone());

        info!("开始迁移 {} 个任务", config.task.len());

        // 迁移任务
        for toml_task in &config.task {
            match self.migrate_task(&repository, toml_task).await {
                Ok(_) => {
                    stats.tasks_migrated += 1;
                    debug!("任务迁移成功: {} ({})", toml_task.task_id, toml_task.name);
                }
                Err(e) => {
                    stats.tasks_failed += 1;
                    let warning = format!(
                        "任务 {} 迁移失败: {}",
                        toml_task.task_id,
                        e
                    );
                    warn!("{}", warning);
                    stats.add_warning(warning);
                }
            }
        }

        // 注册 Teams
        for toml_team in &config.team {
            match self.register_team(&repository, toml_team).await {
                Ok(_) => {
                    stats.teams_registered += 1;
                    debug!("Team 注册成功: {} ({})", toml_team.team_id, toml_team.name);
                }
                Err(e) => {
                    let warning = format!(
                        "Team {} 注册失败: {}",
                        toml_team.team_id,
                        e
                    );
                    warn!("{}", warning);
                    stats.add_warning(warning);
                }
            }
        }

        info!(
            "迁移完成: {} 个任务成功, {} 个任务失败, {} 个Teams注册",
            stats.tasks_migrated, stats.tasks_failed, stats.teams_registered
        );

        if !stats.warnings.is_empty() {
            warn!("迁移过程中有 {} 个警告", stats.warnings.len());
        }

        Ok(stats)
    }

    /// 迁移单个任务
    async fn migrate_task(
        &self,
        repository: &TaskRepository,
        toml_task: &TomlTask,
    ) -> Result<()> {
        // 解析任务类型
        let task_type = self.parse_task_type(&toml_task.task_type)?;

        // 解析优先级
        let priority = self.parse_priority(&toml_task.priority)?;

        // 解析依赖
        let dependencies: Vec<String> = toml_task
            .dependencies
            .iter()
            .filter_map(|d| d.task_id_ref.clone())
            .collect();

        // 解析上下文变量
        let mut context_variables = serde_json::Map::new();
        context_variables.insert(
            "prompt".to_string(),
            serde_json::Value::String(toml_task.prompt.clone()),
        );

        // 添加 capabilities
        let capabilities: Vec<String> = toml_task
            .capabilities
            .iter()
            .map(|c| c.capability.clone())
            .collect();
        context_variables.insert(
            "capabilities".to_string(),
            serde_json::Value::from(capabilities),
        );

        // 添加上下文文件路径
        let context_files: Vec<serde_json::Value> = toml_task
            .context_files
            .iter()
            .map(|f| {
                serde_json::json!({
                    "path": f.path,
                    "description": f.description
                })
            })
            .collect();
        context_variables.insert(
            "context_files".to_string(),
            serde_json::Value::from(context_files),
        );

        // 计算估计工时（转换为小时）
        let estimated_hours = toml_task.effort_person_days.map(|days| days * 8.0);

        // 创建任务实体
        let entity = TaskEntity {
            id: 0, // 数据库自动生成
            task_id: toml_task.task_id.clone(),
            name: toml_task.name.clone(),
            task_type,
            priority,
            prompt_template: toml_task.prompt.clone(),
            context_variables: serde_json::Value::Object(context_variables),
            description: Some(format!(
                "从 TOML 迁移的任务: {}",
                toml_task.name
            )),
            estimated_effort_days: toml_task.effort_person_days,
            dependencies,
            engine_type: None,
            engine_context_id: None,
            status: TaskStatus::Pending,
            assigned_team_id: None,
            assigned_agent_id: None,
            assigned_at: None,
            result: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            duration_seconds: estimated_hours.map(|h| h * 3600.0),
            metadata: Some(serde_json::json!({
                "migration_source": "toml",
                "migrated_at": chrono::Utc::now().timestamp()
            })),
            created_at_ts: chrono::Utc::now().timestamp(),
            updated_at_ts: chrono::Utc::now().timestamp(),
        };

        // 插入数据库
        repository.create(&entity).await?;

        Ok(())
    }

    /// 注册 Team
    async fn register_team(
        &self,
        repository: &TaskRepository,
        toml_team: &TomlTeam,
    ) -> Result<()> {
        // Team 通过 metadata 存储在数据库中
        // 这里我们创建一个特殊的"Team注册"任务来保存 Team 信息
        let team_data = serde_json::json!({
            "team_id": toml_team.team_id,
            "name": toml_team.name,
            "runtime": toml_team.runtime,
            "max_concurrent_tasks": toml_team.max_concurrent_tasks,
            "capabilities": toml_team.capabilities.iter().map(|c| &c.capability).collect::<Vec<_>>(),
            "members": toml_team.members
        });

        // 创建 Team 注册记录（使用特殊的任务类型）
        let entity = TaskEntity {
            id: 0,
            task_id: format!("TEAM-REGISTRATION-{}", toml_team.team_id),
            name: format!("Team Registration: {}", toml_team.name),
            task_type: TaskType::Documentation, // 使用文档类型作为注册记录
            priority: TaskPriority::P3,
            prompt_template: String::new(),
            context_variables: team_data,
            description: Some(format!("Team 注册记录: {}", toml_team.name)),
            estimated_effort_days: None,
            dependencies: Vec::new(),
            engine_type: None,
            engine_context_id: None,
            status: TaskStatus::Completed,
            assigned_team_id: Some(toml_team.team_id.clone()),
            assigned_agent_id: None,
            assigned_at: Some(chrono::Utc::now().timestamp()),
            result: None,
            error_message: None,
            started_at: Some(chrono::Utc::now().timestamp()),
            completed_at: Some(chrono::Utc::now().timestamp()),
            duration_seconds: Some(0.0),
            metadata: Some(serde_json::json!({
                "registration_type": "team",
                "migrated_at": chrono::Utc::now().timestamp()
            })),
            created_at_ts: chrono::Utc::now().timestamp(),
            updated_at_ts: chrono::Utc::now().timestamp(),
        };

        repository.create(&entity).await?;

        Ok(())
    }

    /// 解析任务类型
    fn parse_task_type(&self, type_str: &str) -> Result<TaskType> {
        match type_str {
            "ModuleRefactoring" => Ok(TaskType::ModuleRefactoring),
            "EngineCodeInjection" => Ok(TaskType::EngineCodeInjection),
            "PerformanceOptimization" => Ok(TaskType::PerformanceOptimization),
            "CodeReview" => Ok(TaskType::CodeReview),
            "TestWriting" => Ok(TaskType::TestWriting),
            "Documentation" => Ok(TaskType::Documentation),
            _ => Err(anyhow::anyhow!("未知的任务类型: {}", type_str)),
        }
    }

    /// 解析优先级
    fn parse_priority(&self, priority_str: &str) -> Result<TaskPriority> {
        match priority_str {
            "p0" | "P0" => Ok(TaskPriority::P0),
            "p1" | "P1" => Ok(TaskPriority::P1),
            "p2" | "P2" => Ok(TaskPriority::P2),
            "p3" | "P3" => Ok(TaskPriority::P3),
            _ => Err(anyhow::anyhow!("未知的优先级: {}", priority_str)),
        }
    }

    /// 从目录批量迁移 TOML 文件
    pub async fn migrate_from_directory<P: AsRef<Path>>(
        &self,
        dir_path: P,
    ) -> Result<MigrationStats> {
        let dir = dir_path.as_ref();
        info!("开始从目录迁移 TOML 文件: {}", dir.display());

        let mut total_stats = MigrationStats::new();

        // 遍历目录中的所有 .toml 文件
        let entries = fs::read_dir(dir)
            .with_context(|| format!("无法读取目录: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // 只处理 .toml 文件
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                info!("处理文件: {}", path.display());

                match self.migrate_from_toml_file(&path).await {
                    Ok(stats) => {
                        total_stats.tasks_migrated += stats.tasks_migrated;
                        total_stats.teams_registered += stats.teams_registered;
                        total_stats.tasks_failed += stats.tasks_failed;
                        total_stats.warnings.extend(stats.warnings);
                    }
                    Err(e) => {
                        let warning = format!("文件 {} 迁移失败: {}", path.display(), e);
                        warn!("{}", warning);
                        total_stats.add_warning(warning);
                    }
                }
            }
        }

        info!(
            "目录迁移完成: {} 个任务成功, {} 个任务失败, {} 个Teams注册",
            total_stats.tasks_migrated,
            total_stats.tasks_failed,
            total_stats.teams_registered
        );

        Ok(total_stats)
    }

    /// 验证迁移结果
    pub async fn verify_migration(&self) -> Result<MigrationVerification> {
        let repository = TaskRepository::new(self.db.clone());

        // 获取所有任务统计
        let total_tasks = repository.count(TaskFilter::default()).await? as usize;
        let pending_tasks = repository.count(
            TaskFilter {
                status: Some(TaskStatus::Pending),
                ..Default::default()
            }
        ).await? as usize;

        // 简化的统计信息
        let mut tasks_by_type = HashMap::new();
        let mut tasks_by_priority = HashMap::new();
        let mut tasks_by_status = HashMap::new();

        // 查询所有任务以获取统计
        let all_tasks = repository.query(TaskFilter::default()).await?;
        for task in &all_tasks {
            *tasks_by_type.entry(format!("{:?}", task.task_type)).or_insert(0) += 1;
            *tasks_by_priority.entry(format!("{:?}", task.priority)).or_insert(0) += 1;
            *tasks_by_status.entry(format!("{:?}", task.status)).or_insert(0) += 1;
        }

        let total_estimated_hours: Option<f64> = None;

        let verification = MigrationVerification {
            total_tasks,
            pending_tasks,
            tasks_by_type,
            tasks_by_priority,
            tasks_by_status,
            total_estimated_hours,
        };

        info!("迁移验证: {:?}", verification);

        Ok(verification)
    }

    /// 回滚迁移（删除所有迁移的任务）
    pub async fn rollback_migration(&self, before_timestamp: i64) -> Result<usize> {
        use rusqlite::params;

        let conn = self.db.acquire().await?;

        let count = conn.execute(
            "DELETE FROM tasks WHERE created_at >= ?1",
            params![before_timestamp],
        )?;

        info!("回滚迁移: 删除了 {} 个任务", count);

        Ok(count)
    }
}

/// 迁移验证结果
#[derive(Debug, Clone)]
pub struct MigrationVerification {
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub tasks_by_type: HashMap<String, usize>,
    pub tasks_by_priority: HashMap<String, usize>,
    pub tasks_by_status: HashMap<String, usize>,
    pub total_estimated_hours: Option<f64>,
}

/// CLI 辅助函数
impl TaskMigrator {
    /// 打印迁移报告
    pub fn print_report(stats: &MigrationStats, verification: &MigrationVerification) {
        use colored::*;

        println!("\n{}", "═════════════════════════════════════════".dimmed());
        println!("{}", "           迁移报告".bold().cyan());
        println!("{}", "═════════════════════════════════════════".dimmed());

        // 迁移统计
        println!("\n{}", "迁移统计:".bold());
        println!("  {} 成功迁移 {} 个任务",
            "✓".green(),
            stats.tasks_migrated.to_string().green()
        );
        println!("  {} 注册 {} 个 Teams",
            "✓".green(),
            stats.teams_registered.to_string().green()
        );

        if stats.tasks_failed > 0 {
            println!("  {} {} 个任务迁移失败",
                "✗".red(),
                stats.tasks_failed.to_string().red()
            );
        }

        // 数据库统计
        println!("\n{}", "数据库统计:".bold());
        println!("  总任务数: {}", verification.total_tasks);
        println!("  待处理任务: {}", verification.pending_tasks);

        if let Some(hours) = verification.total_estimated_hours {
            println!("  总估计工时: {:.1} 小时 ({:.1} 人天)",
                hours,
                hours / 8.0
            );
        }

        // 任务类型分布
        if !verification.tasks_by_type.is_empty() {
            println!("\n{}", "任务类型分布:".bold());
            for (task_type, count) in &verification.tasks_by_type {
                println!("  {}: {}", task_type, count);
            }
        }

        // 优先级分布
        if !verification.tasks_by_priority.is_empty() {
            println!("\n{}", "优先级分布:".bold());
            for (priority, count) in &verification.tasks_by_priority {
                println!("  {}: {}", priority, count);
            }
        }

        // 状态分布
        if !verification.tasks_by_status.is_empty() {
            println!("\n{}", "任务状态分布:".bold());
            for (status, count) in &verification.tasks_by_status {
                println!("  {}: {}", status, count);
            }
        }

        // 警告
        if !stats.warnings.is_empty() {
            println!("\n{}", "警告:".bold().yellow());
            for warning in &stats.warnings {
                println!("  {} {}", "⚠".yellow(), warning);
            }
        }

        println!("{}", "\n═════════════════════════════════════════".dimmed());
    }
}

// Tests are in a separate module to keep the file organized
#[cfg(test)]
mod tests {
    // Include the comprehensive test suite from migration_tests.rs
    // This module is kept for compatibility but tests are now in migration_tests.rs
}
