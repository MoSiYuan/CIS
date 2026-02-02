//! # Task Command
//!
//! Task management - list, create, update, etc.

use anyhow::Result;
use cis_core::scheduler::TaskDag;
use cis_core::types::{Task, TaskId, TaskPriority, TaskStatus};

/// Task store for managing tasks
pub struct TaskStore {
    tasks: Vec<Task>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }
    
    pub fn load() -> Result<Self> {
        // TODO: Load from persistent storage
        Ok(Self::new())
    }
    
    pub fn save(&self) -> Result<()> {
        // TODO: Save to persistent storage
        Ok(())
    }
    
    pub fn list_all(&self) -> &[Task] {
        &self.tasks
    }
    
    pub fn get(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }
    
    pub fn add(&mut self, task: Task) {
        self.tasks.push(task);
    }
    
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == id) {
            self.tasks.remove(pos);
            true
        } else {
            false
        }
    }
}

impl Default for TaskStore {
    fn default() -> Self {
        Self::new()
    }
}

/// List all tasks
pub fn list_tasks(status: Option<TaskStatus>) -> Result<()> {
    let store = TaskStore::load()?;
    let tasks: Vec<_> = store.list_all()
        .iter()
        .filter(|t| status.map_or(true, |s| t.status == s))
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
    
    store.add(task);
    store.save()?;
    
    println!("✅ Task created with ID: {}", id);
    
    Ok(())
}

/// Update task status
pub fn update_task_status(id: &str, status: TaskStatus) -> Result<()> {
    let mut store = TaskStore::load()?;
    
    // Find and update task
    let mut found = false;
    for task in store.list_all() {
        if task.id == id {
            found = true;
            // In real implementation, we would update the task
            break;
        }
    }
    
    if !found {
        println!("Task not found: {}", id);
        return Ok(());
    }
    
    // TODO: Implement actual update
    println!("✅ Task {} status updated to {:?}", id, status);
    
    store.save()?;
    
    Ok(())
}

/// Delete a task
pub fn delete_task(id: &str) -> Result<()> {
    let mut store = TaskStore::load()?;
    
    if store.remove(id) {
        store.save()?;
        println!("✅ Task {} deleted.", id);
    } else {
        println!("Task not found: {}", id);
    }
    
    Ok(())
}

/// Execute tasks using DAG scheduler
pub fn execute_tasks() -> Result<()> {
    let store = TaskStore::load()?;
    
    // Build DAG from tasks
    let mut dag = TaskDag::new();
    
    for task in store.list_all() {
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
    
    // TODO: Actually execute tasks
    println!("\n⚠️  Task execution is not yet fully implemented.");
    
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

/// Helper: Truncate string to max length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
