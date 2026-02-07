//! # Task Command
//!
//! Task management - list, create, update, etc.

use anyhow::Result;
use cis_core::scheduler::{TaskDag, LocalExecutor};
use cis_core::scheduler::persistence::DagPersistence;
use cis_core::types::{Task, TaskId, TaskPriority, TaskStatus};
use std::path::PathBuf;

/// Task store for managing tasks - 使用 DAG SQLite 数据库
pub struct TaskStore {
    persistence: DagPersistence,
}

impl TaskStore {
    /// 获取数据库存储路径
    fn db_path() -> PathBuf {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cis");
        
        if !data_dir.exists() {
            let _ = std::fs::create_dir_all(&data_dir);
        }
        
        data_dir.join("cis.db")
    }
    
    pub fn new() -> Result<Self> {
        let persistence = DagPersistence::new(
            Self::db_path().to_str().unwrap_or("cis.db")
        )?;
        
        Ok(Self { persistence })
    }
    
    pub fn load() -> Result<Self> {
        Self::new()
    }
    
    pub fn save(&self) -> Result<()> {
        // persistence 自动保存，无需额外操作
        Ok(())
    }
    
    pub fn list_all(&self) -> Vec<Task> {
        self.persistence.list_tasks(None).unwrap_or_default()
    }
    
    pub fn get(&self, id: &str) -> Option<Task> {
        self.persistence.load_task(id).ok().flatten()
    }
    
    pub fn add(&mut self, task: Task) -> Result<()> {
        self.persistence.save_task(&task)?;
        Ok(())
    }
    
    pub fn remove(&mut self, id: &str) -> Result<bool> {
        let existed = self.persistence.load_task(id)?.is_some();
        self.persistence.delete_task(id)?;
        Ok(existed)
    }
}

impl Default for TaskStore {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| panic!("Failed to create TaskStore"))
    }
}

/// List all tasks
pub fn list_tasks(status: Option<TaskStatus>) -> Result<()> {
    let store = TaskStore::load()?;
    let tasks = store.list_all();
    
    let tasks: Vec<_> = tasks
        .iter()
        .filter(|t| status.map_or(true, |s| s == t.status))
        .collect();
    
    if tasks.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }
    
    println!("Tasks:");
    println!(
        "{:<12} {:<20} {:<10} {:<10} {}",
        "ID", "Title", "Status", "Priority", "Created"
    );
    println!("{}", "-".repeat(90));
    
    for task in tasks {
        let created = task.created_at.format("%Y-%m-%d %H:%M");
        println!(
            "{:<12} {:<20} {:<10} {:<10} {}",
            task.id,
            truncate(&task.title, 20),
            format!("{:?}", task.status).to_lowercase(),
            format!("{:?}", task.priority).to_lowercase(),
            created
        );
    }
    
    Ok(())
}

/// Get task details
pub fn task_details(id: &str) -> Result<()> {
    let store = TaskStore::load()?;
    
    let task = match store.get(id) {
        Some(t) => t,
        None => {
            println!("Task not found: {}", id);
            return Ok(());
        }
    };
    
    println!("Task Details:");
    println!("  ID:                 {}", task.id);
    println!("  Title:              {}", task.title);
    
    if let Some(desc) = &task.description {
        println!("  Description:        {}", desc);
    }
    
    println!("  Group:              {}", task.group_name);
    println!("  Status:             {:?}", task.status);
    println!("  Priority:           {:?}", task.priority);
    
    if let Some(criteria) = &task.completion_criteria {
        println!("  Completion Criteria: {}", criteria);
    }
    
    if !task.dependencies.is_empty() {
        println!("  Dependencies:       {}", task.dependencies.join(", "));
    }
    
    if let Some(result) = &task.result {
        println!("  Result:             {}", result);
    }
    
    if let Some(error) = &task.error {
        println!("  Error:              {}", error);
    }
    
    println!("  Created:            {}", task.created_at.to_rfc3339());
    
    if let Some(started) = task.started_at {
        println!("  Started:            {}", started.to_rfc3339());
    }
    
    if let Some(completed) = task.completed_at {
        println!("  Completed:          {}", completed.to_rfc3339());
    }
    
    println!("  Sandboxed:          {}", task.sandboxed);
    println!("  Allow Network:      {}", task.allow_network);
    
    Ok(())
}

/// Create a new task
pub fn create_task(
    title: &str,
    description: Option<&str>,
    group: Option<&str>,
    priority: Option<TaskPriority>,
    criteria: Option<&str>,
) -> Result<()> {
    let mut store = TaskStore::load()?;
    
    let id = generate_task_id();
    let group = group.unwrap_or("default");
    
    let mut task = Task::new(id.clone(), title.to_string(), group.to_string());
    
    if let Some(desc) = description {
        task.description = Some(desc.to_string());
    }
    
    if let Some(p) = priority {
        task.priority = p;
    }
    
    if let Some(c) = criteria {
        task.completion_criteria = Some(c.to_string());
    }
    
    println!("Creating task '{}'...", title);
    
    store.add(task)?;
    
    println!("✅ Task created with ID: {}", id);
    
    Ok(())
}

/// Update task status
pub fn update_task_status(id: &str, status: TaskStatus) -> Result<()> {
    let store = TaskStore::load()?;
    
    let updated = store.persistence.update_task_status(id, status)?;
    
    if updated {
        println!("✅ Task {} status updated to {:?}", id, status);
    } else {
        println!("Task not found: {}", id);
    }
    
    Ok(())
}

/// Delete a task
pub fn delete_task(id: &str) -> Result<()> {
    let mut store = TaskStore::load()?;
    
    if store.remove(id)? {
        println!("✅ Task {} deleted.", id);
    } else {
        println!("Task not found: {}", id);
    }
    
    Ok(())
}

/// Execute tasks using DAG scheduler
pub async fn execute_tasks() -> Result<()> {
    let store = TaskStore::load()?;
    let tasks = store.list_all();
    
    if tasks.is_empty() {
        println!("No tasks to execute.");
        return Ok(());
    }
    
    // Build DAG from tasks
    let mut dag = TaskDag::new();
    
    for task in &tasks {
        let deps: Vec<String> = task.dependencies.clone();
        dag.add_node(task.id.clone(), deps)?;
    }
    
    // Validate no cycles
    dag.validate()?;
    
    // Get execution order
    dag.initialize();
    let levels = dag.get_execution_order()?;
    
    println!("Task execution order ({} levels):", levels.len());
    for (i, level) in levels.iter().enumerate() {
        println!("  Level {}: {}", i + 1, level.join(", "));
    }
    
    // Execute tasks using LocalExecutor
    println!("\n🚀 Starting task execution...");
    
    let node_id = format!("cis-node-{}", std::process::id());
    let worker_binary = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("cis-node"));
    let default_room = format!("!worker-default:{}", node_id);
    
    let executor = LocalExecutor::new(
        node_id,
        worker_binary.to_string_lossy().to_string(),
        default_room,
    );
    
    // Convert tasks to DagTaskSpec
    let task_specs: Vec<cis_core::scheduler::DagTaskSpec> = tasks.iter().map(|task| {
        cis_core::scheduler::DagTaskSpec {
            id: task.id.clone(),
            task_type: "command".to_string(),
            command: task.title.clone(),
            depends_on: task.dependencies.clone(),
            env: std::collections::HashMap::new(),
        }
    }).collect();
    
    // Create DAG specification
    let dag_spec = cis_core::scheduler::DagSpec::new(
        format!("task-dag-{}", uuid::Uuid::new_v4()),
        task_specs,
    );
    
    // Execute the DAG
    match executor.execute(&dag_spec).await {
        Ok(run_id) => {
            println!("✅ Tasks dispatched successfully");
            println!("   Run ID: {}", run_id);
            println!("   Total tasks: {}", tasks.len());
            println!("\n📊 Execution started. Use 'cis task list' to check status.");
        }
        Err(e) => {
            eprintln!("❌ Failed to execute tasks: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

/// Helper: Generate a simple task ID
fn generate_task_id() -> TaskId {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let chars: String = (0..8)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    format!("task-{}", chars.to_lowercase())
}

/// Helper: Truncate string to max length (处理中文字符)
fn truncate(s: &str, max_len: usize) -> String {
    let chars: Vec<_> = s.chars().collect();
    if chars.len() > max_len {
        let truncated: String = chars.into_iter().take(max_len - 3).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}
