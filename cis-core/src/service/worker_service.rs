//! # Worker Service
//!
//! Worker 进程管理服务，提供统一的 Worker 操作接口。

use super::{ListOptions, PaginatedResult, ResourceStats, ResourceStatus, BatchResult};
use crate::error::{CisError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use sysinfo::System;

/// Worker 摘要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSummary {
    pub id: String,
    pub name: String,
    pub status: ResourceStatus,
    pub pid: Option<u32>,
    pub scope: String,
    pub room: String,
    pub uptime: u64,
    pub tasks_executed: u64,
    pub active_tasks: u64,
    pub created_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

/// Worker 详细信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    #[serde(flatten)]
    pub summary: WorkerSummary,
    pub parent_node: String,
    pub matrix_server: String,
    pub health_interval: u64,
    pub max_cpu: usize,
    pub max_memory_mb: usize,
    pub config: HashMap<String, serde_json::Value>,
    /// Worker 启动参数（用于重启）
    pub worker_args: Option<WorkerStartArgs>,
}

/// Worker 启动参数（用于持久化和重启）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStartArgs {
    pub room: String,
    pub scope: WorkerScope,
    pub parent_node: String,
    pub scope_id: Option<String>,
    pub health_interval: u64,
    pub max_cpu: usize,
    pub max_memory_mb: usize,
    pub matrix_server: String,
    pub matrix_token: String,
    pub env: HashMap<String, String>,
}

/// Worker 创建选项
#[derive(Debug, Clone, Default)]
pub struct WorkerCreateOptions {
    pub name: Option<String>,
    pub room: String,
    pub scope: WorkerScope,
    pub parent_node: String,
    pub scope_id: Option<String>,
    pub health_interval: u64,
    pub max_cpu: usize,
    pub max_memory_mb: usize,
    pub matrix_server: String,
    pub matrix_token: String,
    pub detach: bool,
    pub env: HashMap<String, String>,
}

/// Worker 作用域
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerScope {
    #[default]
    Global,
    Project,
    User,
    Type,
}

impl std::fmt::Display for WorkerScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerScope::Global => write!(f, "global"),
            WorkerScope::Project => write!(f, "project"),
            WorkerScope::User => write!(f, "user"),
            WorkerScope::Type => write!(f, "type"),
        }
    }
}

impl std::str::FromStr for WorkerScope {
    type Err = CisError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "global" => Ok(WorkerScope::Global),
            "project" => Ok(WorkerScope::Project),
            "user" => Ok(WorkerScope::User),
            "type" => Ok(WorkerScope::Type),
            _ => Err(CisError::invalid_input(format!("Invalid worker scope: {}", s))),
        }
    }
}

/// Worker Service
pub struct WorkerService {
    data_dir: PathBuf,
}

impl WorkerService {
    /// 创建新的 WorkerService
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("cis")
            .join("workers");
        
        std::fs::create_dir_all(&data_dir)?;
        
        Ok(Self { data_dir })
    }

    /// 创建 Worker
    pub async fn create(&self, options: WorkerCreateOptions) -> Result<WorkerInfo> {
        let id = options.name.unwrap_or_else(|| generate_worker_id());
        
        // 构建启动参数
        let worker_args = WorkerStartArgs {
            room: options.room.clone(),
            scope: options.scope,
            parent_node: options.parent_node.clone(),
            scope_id: options.scope_id.clone(),
            health_interval: options.health_interval,
            max_cpu: options.max_cpu,
            max_memory_mb: options.max_memory_mb,
            matrix_server: options.matrix_server.clone(),
            matrix_token: options.matrix_token.clone(),
            env: options.env.clone(),
        };
        
        // 启动 worker 进程
        let pid = self.spawn_worker_process(&id, &worker_args, options.detach).await?;
        
        let info = WorkerInfo {
            summary: WorkerSummary {
                id: id.clone(),
                name: id.clone(),
                status: if options.detach {
                    ResourceStatus::Running
                } else {
                    ResourceStatus::Creating
                },
                pid: Some(pid),
                scope: options.scope.to_string(),
                room: options.room.clone(),
                uptime: 0,
                tasks_executed: 0,
                active_tasks: 0,
                created_at: Utc::now(),
                last_heartbeat: None,
            },
            parent_node: options.parent_node,
            matrix_server: options.matrix_server,
            health_interval: options.health_interval,
            max_cpu: options.max_cpu,
            max_memory_mb: options.max_memory_mb,
            config: HashMap::new(),
            worker_args: Some(worker_args),
        };
        
        self.save_worker(&info)?;
        
        Ok(info)
    }

    /// 启动 worker 进程
    async fn spawn_worker_process(&self, worker_id: &str, args: &WorkerStartArgs, detach: bool) -> Result<u32> {
        // 获取当前可执行文件路径
        let current_exe = std::env::current_exe()
            .map_err(|e| CisError::execution(format!("Failed to get current executable: {}", e)))?;
        
        // 构建命令参数
        let mut cmd = Command::new(&current_exe);
        cmd.arg("worker")
            .arg("run")
            .arg("--worker-id")
            .arg(worker_id)
            .arg("--room")
            .arg(&args.room)
            .arg("--scope")
            .arg(args.scope.to_string())
            .arg("--parent-node")
            .arg(&args.parent_node)
            .arg("--health-interval")
            .arg(args.health_interval.to_string())
            .arg("--max-cpu")
            .arg(args.max_cpu.to_string())
            .arg("--max-memory-mb")
            .arg(args.max_memory_mb.to_string())
            .arg("--matrix-server")
            .arg(&args.matrix_server);
        
        // 添加 scope-id（如果有）
        if let Some(ref scope_id) = args.scope_id {
            cmd.arg("--scope-id").arg(scope_id);
        }
        
        // 添加 Matrix token（如果有）
        if !args.matrix_token.is_empty() {
            cmd.arg("--matrix-token").arg(&args.matrix_token);
        }
        
        // 添加 detach 模式
        if detach {
            cmd.arg("--detach");
        }
        
        // 设置环境变量
        for (key, value) in &args.env {
            cmd.env(key, value);
        }
        
        // 设置日志文件路径
        let log_file = self.data_dir.join(format!("{}.log", worker_id));
        let log_file = std::fs::File::create(log_file)
            .map_err(|e| CisError::execution(format!("Failed to create log file: {}", e)))?;
        
        cmd.stdout(std::process::Stdio::from(log_file.try_clone()?))
            .stderr(std::process::Stdio::from(log_file));
        
        // 启动进程
        let mut child = cmd.spawn()
            .map_err(|e| CisError::execution(format!("Failed to spawn worker process: {}", e)))?;
        
        let pid = child.id()
            .ok_or_else(|| CisError::execution("Failed to get worker process PID".to_string()))?;
        
        // 如果是 detach 模式，等待一小会儿确认进程成功启动
        if detach {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            // 检查进程是否还在运行
            match child.try_wait() {
                Ok(Some(status)) => {
                    return Err(CisError::execution(
                        format!("Worker process exited immediately with status: {}", status)
                    ));
                }
                Ok(None) => {
                    // 进程还在运行，成功
                }
                Err(e) => {
                    return Err(CisError::execution(
                        format!("Failed to check worker process status: {}", e)
                    ));
                }
            }
        }
        
        Ok(pid)
    }

    /// 列出 Workers
    pub async fn list(&self, options: ListOptions) -> Result<PaginatedResult<WorkerSummary>> {
        let workers = self.load_all_workers()?;
        let mut items: Vec<WorkerSummary> = workers.into_iter()
            .map(|w| w.summary)
            .collect();
        
        // 更新状态（检查进程是否还在运行）
        for item in &mut items {
            if let Some(pid) = item.pid {
                if !Self::is_process_running(pid) {
                    item.status = ResourceStatus::Stopped;
                }
            }
        }
        
        // 应用过滤器
        if !options.all {
            items.retain(|w| matches!(w.status, ResourceStatus::Running));
        }
        
        for (key, value) in &options.filters {
            match key.as_str() {
                "status" => {
                    items.retain(|w| w.status.to_string() == value.to_lowercase());
                }
                "scope" => {
                    items.retain(|w| w.scope == *value);
                }
                "room" => {
                    items.retain(|w| w.room.contains(value));
                }
                _ => {}
            }
        }
        
        // 应用排序
        if let Some(sort_by) = options.sort_by {
            match sort_by.as_str() {
                "created" => {
                    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                }
                "uptime" => {
                    items.sort_by(|a, b| b.uptime.cmp(&a.uptime));
                }
                "tasks" => {
                    items.sort_by(|a, b| b.tasks_executed.cmp(&a.tasks_executed));
                }
                _ => {}
            }
        }
        
        let total = items.len();
        
        // 应用限制
        if let Some(limit) = options.limit {
            items.truncate(limit);
        }
        
        Ok(PaginatedResult::new(items, total))
    }

    /// 获取 Worker 详情
    pub async fn inspect(&self, id: &str) -> Result<WorkerInfo> {
        let mut info = self.find_worker(id)?
            .ok_or_else(|| CisError::not_found(format!("Worker '{}' not found", id)))?;
        
        // 更新状态（检查进程是否还在运行）
        if let Some(pid) = info.summary.pid {
            if !Self::is_process_running(pid) && matches!(info.summary.status, ResourceStatus::Running) {
                info.summary.status = ResourceStatus::Stopped;
                // 更新保存的状态
                let _ = self.save_worker(&info);
            }
        }
        
        Ok(info)
    }

    /// 检查 Worker 是否存在
    pub async fn exists(&self, id: &str) -> Result<bool> {
        Ok(self.find_worker(id)?.is_some())
    }

    /// 停止 Worker
    pub async fn stop(&self, id: &str, force: bool, timeout: u64) -> Result<()> {
        let info = self.find_worker(id)?
            .ok_or_else(|| CisError::not_found(format!("Worker '{}' not found", id)))?;
        
        if !matches!(info.summary.status, ResourceStatus::Running) {
            return Err(CisError::invalid_input(format!("Worker '{}' is not running", id)));
        }
        
        let pid = info.summary.pid
            .ok_or_else(|| CisError::invalid_input(format!("Worker '{}' has no PID", id)))?;
        
        // 发送停止信号
        #[cfg(unix)]
        {
            use libc::{kill, SIGTERM, SIGKILL};
            
            let pid_i32 = pid as i32;
            
            if force {
                // 强制停止：发送 SIGKILL
                unsafe {
                    let result = kill(pid_i32, SIGKILL);
                    if result != 0 {
                        return Err(CisError::execution(
                            format!("Failed to send SIGKILL to worker '{}': {}", id, std::io::Error::last_os_error())
                        ));
                    }
                }
            } else {
                // 优雅停止：发送 SIGTERM
                unsafe {
                    let result = kill(pid_i32, SIGTERM);
                    if result != 0 {
                        return Err(CisError::execution(
                            format!("Failed to send SIGTERM to worker '{}': {}", id, std::io::Error::last_os_error())
                        ));
                    }
                }
                
                // 等待进程终止
                let mut stopped = false;
                for _ in 0..timeout {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    if !Self::is_process_running(pid) {
                        stopped = true;
                        break;
                    }
                }
                
                // 如果还在运行，强制杀死
                if !stopped {
                    unsafe {
                        let _ = kill(pid_i32, SIGKILL);
                    }
                }
            }
        }
        
        #[cfg(windows)]
        {
            // Windows 下使用 taskkill 命令
            let output = if force {
                Command::new("taskkill")
                    .args(&["/F", "/PID", &pid.to_string()])
                    .output()
                    .await
            } else {
                Command::new("taskkill")
                    .args(&["/PID", &pid.to_string()])
                    .output()
                    .await
            };
            
            match output {
                Ok(result) => {
                    if !result.status.success() && force {
                        let error = String::from_utf8_lossy(&result.stderr);
                        return Err(CisError::internal_error(
                            format!("Failed to stop worker '{}': {}", id, error)
                        ));
                    }
                }
                Err(e) => {
                    return Err(CisError::execution(
                        format!("Failed to execute taskkill for worker '{}': {}", id, e)
                    ));
                }
            }
        }
        
        // 更新 worker 状态
        let mut updated_info = info;
        updated_info.summary.status = ResourceStatus::Stopped;
        updated_info.summary.pid = None;
        self.save_worker(&updated_info)?;
        
        Ok(())
    }

    /// 启动已停止的 Worker
    pub async fn start(&self, id: &str) -> Result<()> {
        let mut info = self.find_worker(id)?
            .ok_or_else(|| CisError::not_found(format!("Worker '{}' not found", id)))?;
        
        if matches!(info.summary.status, ResourceStatus::Running) {
            return Err(CisError::invalid_input(format!("Worker '{}' is already running", id)));
        }
        
        // 检查是否有保存的启动参数
        let worker_args = info.worker_args.clone()
            .ok_or_else(|| CisError::invalid_input(
                format!("Worker '{}' has no saved start arguments. Please create a new worker.", id)
            ))?;
        
        // 重新启动 worker 进程
        let pid = self.spawn_worker_process(id, &worker_args, true).await?;
        
        // 更新 worker 信息
        info.summary.pid = Some(pid);
        info.summary.status = ResourceStatus::Running;
        info.summary.created_at = Utc::now();
        info.summary.last_heartbeat = None;
        self.save_worker(&info)?;
        
        Ok(())
    }

    /// 重启 Worker
    pub async fn restart(&self, id: &str, timeout: u64) -> Result<()> {
        // 先停止 worker
        let info = self.find_worker(id)?
            .ok_or_else(|| CisError::not_found(format!("Worker '{}' not found", id)))?;
        
        if matches!(info.summary.status, ResourceStatus::Running) {
            self.stop(id, false, timeout).await?;
        }
        
        // 等待一小会儿确保进程完全停止
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // 重新启动
        self.start(id).await
    }

    /// 删除 Worker
    pub async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let info = self.find_worker(id)?
            .ok_or_else(|| CisError::not_found(format!("Worker '{}' not found", id)))?;
        
        if matches!(info.summary.status, ResourceStatus::Running) && !force {
            return Err(CisError::invalid_input(
                format!("Worker '{}' is running. Use --force to remove.", id)
            ));
        }
        
        if matches!(info.summary.status, ResourceStatus::Running) {
            self.stop(id, true, 0).await?;
        }
        
        let worker_file = self.data_dir.join(format!("{}.json", info.summary.id));
        if worker_file.exists() {
            std::fs::remove_file(worker_file)?;
        }
        
        // 同时删除日志文件
        let log_file = self.data_dir.join(format!("{}.log", info.summary.id));
        if log_file.exists() {
            let _ = std::fs::remove_file(log_file);
        }
        
        Ok(())
    }

    /// 批量删除 Workers
    pub async fn remove_batch(&self, ids: &[String], force: bool) -> Result<BatchResult> {
        let mut result = BatchResult::new();
        
        for id in ids {
            match self.remove(id, force).await {
                Ok(()) => result.add_success(id),
                Err(e) => result.add_failure(id, e.to_string()),
            }
        }
        
        Ok(result)
    }

    /// 获取 Worker 统计
    pub async fn stats(&self, id: &str) -> Result<ResourceStats> {
        let info = self.find_worker(id)?
            .ok_or_else(|| CisError::not_found(format!("Worker '{}' not found", id)))?;
        
        let pid = info.summary.pid
            .ok_or_else(|| CisError::invalid_input(format!("Worker '{}' has no PID", id)))?;
        
        // 使用 sysinfo 获取进程资源使用
        let mut system = System::new_all();
        system.refresh_all();
        
        let mut stats = ResourceStats {
            cpu_percent: 0.0,
            memory_usage: 0,
            memory_limit: info.max_memory_mb as u64 * 1024 * 1024,
            memory_percent: 0.0,
            io_read_bytes: 0,
            io_write_bytes: 0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            pids: 1,
        };
        
        // 查找进程并获取资源使用
        if let Some(process) = system.process(sysinfo::Pid::from(pid as usize)) {
            // CPU 使用率
            stats.cpu_percent = process.cpu_usage() as f64;
            
            // 内存使用
            stats.memory_usage = process.memory();
            
            // 内存使用率
            let total_memory = system.total_memory() * 1024; // 转换为 bytes
            if total_memory > 0 {
                stats.memory_percent = (stats.memory_usage as f64 / total_memory as f64) * 100.0;
            }
            
            // 磁盘 I/O
            let disk_usage = process.disk_usage();
            stats.io_read_bytes = disk_usage.read_bytes;
            stats.io_write_bytes = disk_usage.written_bytes;
            
            // 统计子进程数量
            let process_pids: Vec<_> = system.processes().keys().collect();
            stats.pids = process_pids.len() as u32;
        }
        
        Ok(stats)
    }

    /// 清理已停止的 Workers
    pub async fn prune(&self) -> Result<Vec<String>> {
        let workers = self.load_all_workers()?;
        let mut removed = Vec::new();
        
        for worker in workers {
            if !matches!(worker.summary.status, ResourceStatus::Running) {
                self.remove(&worker.summary.id, false).await?;
                removed.push(worker.summary.id);
            }
        }
        
        Ok(removed)
    }

    /// 获取 Worker 日志
    ///
    /// # 参数
    /// - `id`: Worker ID
    /// - `tail`: 返回最后多少行
    /// - `follow`: 是否持续跟踪新日志（会阻塞直到超时或进程结束）
    ///
    /// # 说明
    /// 当 `follow` 为 true 时，函数会持续读取新追加的日志行，
    /// 直到 Worker 进程结束或达到 30 秒超时。
    pub async fn logs(&self, id: &str, tail: usize, follow: bool) -> Result<Vec<String>> {
        let info = self.find_worker(id)?
            .ok_or_else(|| CisError::not_found(format!("Worker '{}' not found", id)))?;
        
        let log_file = self.data_dir.join(format!("{}.log", info.summary.id));
        
        if !log_file.exists() {
            return Ok(Vec::new());
        }
        
        if follow {
            // Follow 模式：持续读取新日志
            let file = tokio::fs::File::open(&log_file).await
                .map_err(|e| CisError::io(format!("Failed to open log file: {}", e)))?;
            
            let reader = BufReader::new(file);
            let mut lines_iter = reader.lines();
            
            // 跳过已存在的行，只保留最后 `tail` 行
            let mut all_lines: Vec<String> = Vec::new();
            while let Ok(Some(line)) = lines_iter.next_line().await {
                all_lines.push(line);
            }
            
            let start = if all_lines.len() > tail { all_lines.len() - tail } else { 0 };
            let mut lines: Vec<String> = all_lines[start..].to_vec();
            
            // 检查 Worker 进程是否仍在运行
            let pid = info.summary.pid;
            let mut last_size = lines.len();
            let timeout = tokio::time::Duration::from_secs(30);
            let start_time = tokio::time::Instant::now();
            
            // 持续监控文件变化，直到进程结束或超时
            loop {
                // 检查是否超时
                if start_time.elapsed() > timeout {
                    tracing::debug!("Log follow timeout reached for worker {}", id);
                    break;
                }
                
                // 检查进程是否仍在运行
                if let Some(p) = pid {
                    if !Self::is_process_running(p) {
                        tracing::debug!("Worker {} process ended, stopping log follow", id);
                        break;
                    }
                }
                
                // 等待一小段时间
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                // 重新读取文件
                match tokio::fs::read_to_string(&log_file).await {
                    Ok(contents) => {
                        let new_lines: Vec<String> = contents.lines().map(String::from).collect();
                        if new_lines.len() > last_size {
                            // 添加新行
                            let new_entries = &new_lines[last_size..];
                            lines.extend(new_entries.iter().cloned());
                            last_size = new_lines.len();
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read log file: {}", e);
                        break;
                    }
                }
            }
            
            Ok(lines)
        } else {
            // 非 follow 模式：直接读取文件
            let contents = tokio::fs::read_to_string(log_file).await?;
            let all_lines: Vec<String> = contents.lines().map(String::from).collect();
            
            let start = if all_lines.len() > tail { all_lines.len() - tail } else { 0 };
            let lines: Vec<String> = all_lines[start..].to_vec();
            Ok(lines)
        }
    }

    /// 通过 ID 或前缀查找 Worker
    fn find_worker(&self, id: &str) -> Result<Option<WorkerInfo>> {
        // 先尝试精确匹配
        let exact_file = self.data_dir.join(format!("{}.json", id));
        if exact_file.exists() {
            return self.load_worker(&exact_file);
        }
        
        // 前缀匹配
        let matches = self.find_by_prefix(id)?;
        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches.into_iter().next().unwrap())),
            _ => Err(CisError::invalid_input(format!(
                "Multiple workers match prefix '{}': {}",
                id,
                matches.iter().map(|w| w.summary.id.clone()).collect::<Vec<_>>().join(", ")
            ))),
        }
    }

    /// 前缀查找
    fn find_by_prefix(&self, prefix: &str) -> Result<Vec<WorkerInfo>> {
        let all = self.load_all_workers()?;
        Ok(all.into_iter()
            .filter(|w| w.summary.id.starts_with(prefix))
            .collect())
    }

    /// 加载所有 Workers
    fn load_all_workers(&self) -> Result<Vec<WorkerInfo>> {
        let mut workers = Vec::new();
        
        for entry in std::fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(worker) = self.load_worker(&path)? {
                    workers.push(worker);
                }
            }
        }
        
        Ok(workers)
    }

    /// 加载单个 Worker
    fn load_worker(&self, path: &PathBuf) -> Result<Option<WorkerInfo>> {
        let json = std::fs::read_to_string(path)?;
        let mut info: WorkerInfo = serde_json::from_str(&json)?;
        
        // 检查进程状态并更新
        if let Some(pid) = info.summary.pid {
            if !Self::is_process_running(pid) && matches!(info.summary.status, ResourceStatus::Running) {
                info.summary.status = ResourceStatus::Stopped;
                // 可选：自动保存更新后的状态
                let _ = self.save_worker(&info);
            }
        }
        
        Ok(Some(info))
    }

    /// 保存 Worker
    fn save_worker(&self, info: &WorkerInfo) -> Result<()> {
        let worker_file = self.data_dir.join(format!("{}.json", info.summary.id));
        let json = serde_json::to_string_pretty(info)?;
        std::fs::write(worker_file, json)?;
        Ok(())
    }

    /// 检查进程是否还在运行
    fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            // 在 Unix 上，发送信号 0 检查进程是否存在
            unsafe {
                libc::kill(pid as i32, 0) == 0
            }
        }
        #[cfg(windows)]
        {
            // 在 Windows 上，使用 sysinfo 检查
            let mut system = System::new_all();
            system.refresh_all();
            system.process(sysinfo::Pid::from(pid as usize)).is_some()
        }
    }
}

impl Default for WorkerService {
    fn default() -> Self {
        Self::new().expect("Failed to create WorkerService")
    }
}

/// 生成 Worker ID
fn generate_worker_id() -> String {
    format!(
        "wkr-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}
