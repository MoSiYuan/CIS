//! Agent 进程检测器
//!
//! 检测 Claude, OpenCode, Kimi 等 Agent 进程

use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::{debug, info, warn};

/// Agent 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentType {
    Claude,
    OpenCode,
    Kimi,
}

impl AgentType {
    pub fn process_name(&self) -> &'static str {
        match self {
            AgentType::Claude => "claude",
            AgentType::OpenCode => "opencode",
            AgentType::Kimi => "kimi",
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentType::Claude => "Claude",
            AgentType::OpenCode => "OpenCode",
            AgentType::Kimi => "Kimi",
        }
    }
}

/// Agent 进程信息
#[derive(Debug, Clone)]
pub struct AgentProcessInfo {
    pub pid: u32,
    pub agent_type: AgentType,
    pub command: String,
    pub working_dir: PathBuf,
    pub start_time: SystemTime,
    pub port: Option<u16>,
}

/// Agent Session 信息
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub session_id: String,
    pub agent_type: AgentType,
    pub pid: u32,
    pub created_at: SystemTime,
    pub last_active_at: SystemTime,
    pub total_tasks: u32,
    pub work_dir: PathBuf,
}

/// Agent 进程检测器
pub struct AgentProcessDetector;

impl AgentProcessDetector {
    /// 检测指定类型的 Agent 进程
    pub fn detect(agent_type: AgentType) -> Vec<AgentProcessInfo> {
        let mut processes = Vec::new();
        let name = agent_type.process_name();
        
        // 使用 sysinfo 获取系统进程
        let mut system = sysinfo::System::new_all();
        system.refresh_all();
        
        for (pid, process) in system.processes() {
            let cmd = process.cmd().join(" ");
            let exe = process.name();
            
            // 检查进程名或命令行
            if exe.to_lowercase().contains(name) 
                || cmd.to_lowercase().contains(&format!("{} ", name))
                || cmd.to_lowercase().contains(&format!("/{}", name)) {
                
                let working_dir = process.cwd().map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::temp_dir());
                
                // 尝试检测端口
                let port = Self::detect_port(pid.as_u32());
                
                processes.push(AgentProcessInfo {
                    pid: pid.as_u32(),
                    agent_type,
                    command: cmd,
                    working_dir,
                    start_time: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(process.start_time()),
                    port,
                });
            }
        }
        
        processes
    }
    
    /// 通过 PID 检查 Agent 是否运行
    pub fn is_running(pid: u32) -> bool {
        let mut system = sysinfo::System::new();
        system.refresh_processes();
        system.process(sysinfo::Pid::from_u32(pid)).is_some()
    }
    
    /// 获取 Agent 的活跃会话
    pub fn get_sessions(agent_type: AgentType) -> Vec<AgentSession> {
        let processes = Self::detect(agent_type);
        let mut sessions = Vec::new();
        
        for proc in processes {
            // 尝试从工作目录恢复 session_id
            let session_id = Self::extract_session_id(&proc.working_dir);
            
            // 尝试从 session 文件加载统计信息
            let (last_active, total_tasks) = Self::load_session_stats(&proc.working_dir);
            
            sessions.push(AgentSession {
                session_id,
                agent_type: proc.agent_type,
                pid: proc.pid,
                created_at: proc.start_time,
                last_active_at: last_active.unwrap_or(proc.start_time),
                total_tasks,
                work_dir: proc.working_dir,
            });
        }
        
        sessions
    }
    
    /// 从 session 目录加载统计信息
    fn load_session_stats(work_dir: &PathBuf) -> (Option<SystemTime>, u32) {
        let stats_file = work_dir.join(".session_stats.json");
        
        if let Ok(content) = std::fs::read_to_string(&stats_file) {
            if let Ok(stats) = serde_json::from_str::<serde_json::Value>(&content) {
                let last_active = stats.get("last_active")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(|ts| std::time::UNIX_EPOCH + std::time::Duration::from_secs(ts));
                
                let total_tasks = stats.get("total_tasks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                
                return (last_active, total_tasks);
            }
        }
        
        (None, 0)
    }
    
    /// 保存 session 统计信息
    pub fn save_session_stats(work_dir: &PathBuf, total_tasks: u32) -> Result<(), std::io::Error> {
        let stats_file = work_dir.join(".session_stats.json");
        let stats = serde_json::json!({
            "last_active": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "total_tasks": total_tasks,
        });
        
        std::fs::write(&stats_file, serde_json::to_string(&stats)?)
    }
    
    /// 检测进程是否监听端口
    fn detect_port(pid: u32) -> Option<u16> {
        // 尝试通过 lsof 或 /proc/net/tcp 检测
        // 简化的实现，实际应该扫描网络连接
        None
    }
    
    /// 从工作目录提取 session ID
    fn extract_session_id(work_dir: &PathBuf) -> String {
        work_dir.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    
    /// 检测所有类型的 Agent
    pub fn detect_all() -> HashMap<AgentType, Vec<AgentProcessInfo>> {
        let mut result = HashMap::new();
        
        for agent_type in [AgentType::Claude, AgentType::OpenCode, AgentType::Kimi] {
            let processes = Self::detect(agent_type);
            if !processes.is_empty() {
                result.insert(agent_type, processes);
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_agent_type_display() {
        assert_eq!(AgentType::Claude.display_name(), "Claude");
        assert_eq!(AgentType::OpenCode.display_name(), "OpenCode");
    }
    
    #[test]
    fn test_detect_self() {
        // 应该能检测到当前测试进程
        let current_pid = std::process::id();
        assert!(AgentProcessDetector::is_running(current_pid));
    }
}
