//! AI Executor Skill
//!
//! 抽象的 AI Agent 执行层，兼容多种 AI 工具
//! 直接执行，不检查环境可用性（信任用户已配置）

use std::process::{Command, Stdio};

/// AI Agent 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentType {
    ClaudeCode,
    ClaudeCli,
    KimiCode,
    KimiCli,
    Aider,
    Codex,
}

impl AgentType {
    pub fn command(&self) -> &str {
        match self {
            AgentType::ClaudeCode => "claude",
            AgentType::ClaudeCli => "claude-cli",
            AgentType::KimiCode => "kimi",
            AgentType::KimiCli => "kimi-cli",
            AgentType::Aider => "aider",
            AgentType::Codex => "codex",
        }
    }
}

pub struct ExecuteRequest {
    pub agent: AgentType,
    pub prompt: String,
    pub work_dir: Option<String>,
}

pub struct ExecuteResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct AiExecutor;

impl AiExecutor {
    pub fn new() -> Self { Self }
    
    pub fn execute(&self, req: ExecuteRequest) -> Result<ExecuteResponse, String> {
        let mut cmd = Command::new(req.agent.command());
        
        if let Some(ref work_dir) = req.work_dir {
            cmd.current_dir(work_dir);
        }
        
        cmd.arg(&req.prompt)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
        
        let output = cmd.output()
            .map_err(|e| format!("Execute failed: {}", e))?;
        
        Ok(ExecuteResponse {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

impl Default for AiExecutor {
    fn default() -> Self { Self::new() }
}

// WASM 导出
#[no_mangle]
pub extern "C" fn skill_init() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn skill_execute(_input_ptr: *const u8, _input_len: usize) -> i32 {
    0
}
