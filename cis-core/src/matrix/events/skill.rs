//! 强类型 Skill 消息事件
//! 
//! 命名空间: io.cis.{skill_name}.{action}

use serde::{Deserialize, Serialize};

/// io.cis.task.invoke - 任务调用
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TaskInvokeEventContent {
    pub task_id: String,
    pub skill_name: String,
    pub method: String,
    pub params: serde_json::Value,
    pub priority: i32,
}

/// io.cis.task.result - 任务结果
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TaskResultEventContent {
    pub task_id: String,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

/// io.cis.git.push - Git 推送
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct GitPushEventContent {
    pub repo: String,
    pub repo_url: String,
    pub ref_name: String,
    pub commit: String,
    pub commit_message: String,
    pub author: String,
    pub objects: Vec<String>,
    pub diff_stats: DiffStats,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DiffStats {
    pub insertions: u32,
    pub deletions: u32,
    pub files_changed: u32,
}

/// io.cis.im.message - IM 消息
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ImMessageEventContent {
    pub message_id: String,
    pub sender_id: String,
    pub content: MessageContent,
    pub reply_to: Option<String>,
    pub mentions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum MessageContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { url: String, width: u32, height: u32 },
    #[serde(rename = "file")]
    File { name: String, url: String, size: u64 },
}

/// io.cis.nav.target - 导航目标
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct NavTargetEventContent {
    pub target: String,
    pub coordinates: Option<(f64, f64)>,
    pub accuracy: f32,
}

/// io.cis.memory.update - 记忆更新
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MemoryUpdateEventContent {
    pub key: String,
    pub domain: String,  // "private" or "public"
    pub category: String,
    pub summary: String,
    pub full_content_hash: String,  // 实际内容存储在 memory.db
}

/// Skill 事件类型枚举
#[derive(Clone, Debug, PartialEq)]
pub enum SkillEvent {
    TaskInvoke(TaskInvokeEventContent),
    TaskResult(TaskResultEventContent),
    GitPush(GitPushEventContent),
    ImMessage(ImMessageEventContent),
    NavTarget(NavTargetEventContent),
    MemoryUpdate(MemoryUpdateEventContent),
}

/// 技能注册表
pub struct SkillEventRegistry;

impl SkillEventRegistry {
    /// 获取 Skill 对应的事件类型
    pub fn event_types_for_skill(skill_name: &str) -> Vec<&'static str> {
        match skill_name {
            "git" => vec!["io.cis.git.push", "io.cis.git.pull"],
            "im" => vec!["io.cis.im.message", "io.cis.im.typing"],
            "nav" => vec!["io.cis.nav.target", "io.cis.nav.arrived"],
            "task" => vec!["io.cis.task.invoke", "io.cis.task.result"],
            "memory" => vec!["io.cis.memory.update"],
            _ => vec![],
        }
    }
    
    /// 解析事件内容为具体类型
    pub fn parse_event(event_type: &str, content: &serde_json::Value) -> Result<SkillEvent, SkillEventError> {
        match event_type {
            "io.cis.task.invoke" => {
                let content: TaskInvokeEventContent = serde_json::from_value(content.clone())
                    .map_err(|e| SkillEventError::ParseError(e.to_string()))?;
                Ok(SkillEvent::TaskInvoke(content))
            }
            "io.cis.task.result" => {
                let content: TaskResultEventContent = serde_json::from_value(content.clone())
                    .map_err(|e| SkillEventError::ParseError(e.to_string()))?;
                Ok(SkillEvent::TaskResult(content))
            }
            "io.cis.git.push" => {
                let content: GitPushEventContent = serde_json::from_value(content.clone())
                    .map_err(|e| SkillEventError::ParseError(e.to_string()))?;
                Ok(SkillEvent::GitPush(content))
            }
            "io.cis.im.message" => {
                let content: ImMessageEventContent = serde_json::from_value(content.clone())
                    .map_err(|e| SkillEventError::ParseError(e.to_string()))?;
                Ok(SkillEvent::ImMessage(content))
            }
            "io.cis.nav.target" => {
                let content: NavTargetEventContent = serde_json::from_value(content.clone())
                    .map_err(|e| SkillEventError::ParseError(e.to_string()))?;
                Ok(SkillEvent::NavTarget(content))
            }
            "io.cis.memory.update" => {
                let content: MemoryUpdateEventContent = serde_json::from_value(content.clone())
                    .map_err(|e| SkillEventError::ParseError(e.to_string()))?;
                Ok(SkillEvent::MemoryUpdate(content))
            }
            _ => Err(SkillEventError::UnknownEventType(event_type.to_string())),
        }
    }
    
    /// 获取所有支持的事件类型
    pub fn all_event_types() -> Vec<&'static str> {
        vec![
            "io.cis.task.invoke",
            "io.cis.task.result",
            "io.cis.git.push",
            "io.cis.git.pull",
            "io.cis.im.message",
            "io.cis.im.typing",
            "io.cis.nav.target",
            "io.cis.nav.arrived",
            "io.cis.memory.update",
        ]
    }
}

/// Skill 事件错误
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum SkillEventError {
    #[error("Unknown event type: {0}")]
    UnknownEventType(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_invoke_serialization() {
        let event = TaskInvokeEventContent {
            task_id: "task-123".to_string(),
            skill_name: "git".to_string(),
            method: "push".to_string(),
            params: serde_json::json!({"branch": "main"}),
            priority: 1,
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: TaskInvokeEventContent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_git_push_event() {
        let event = GitPushEventContent {
            repo: "my-repo".to_string(),
            repo_url: "https://github.com/user/repo".to_string(),
            ref_name: "refs/heads/main".to_string(),
            commit: "abc123".to_string(),
            commit_message: "Initial commit".to_string(),
            author: "user@example.com".to_string(),
            objects: vec!["obj1".to_string(), "obj2".to_string()],
            diff_stats: DiffStats {
                insertions: 10,
                deletions: 5,
                files_changed: 3,
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: GitPushEventContent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_im_message_content() {
        let text_content = MessageContent::Text {
            text: "Hello, world!".to_string(),
        };

        let json = serde_json::to_string(&text_content).unwrap();
        assert!(json.contains("text"));
        
        let decoded: MessageContent = serde_json::from_str(&json).unwrap();
        assert_eq!(text_content, decoded);
    }

    #[test]
    fn test_event_types_for_skill() {
        let git_events = SkillEventRegistry::event_types_for_skill("git");
        assert!(git_events.contains(&"io.cis.git.push"));
        assert!(git_events.contains(&"io.cis.git.pull"));

        let task_events = SkillEventRegistry::event_types_for_skill("task");
        assert!(task_events.contains(&"io.cis.task.invoke"));
        assert!(task_events.contains(&"io.cis.task.result"));
    }

    #[test]
    fn test_parse_event() {
        let content = serde_json::json!({
            "task_id": "task-123",
            "skill_name": "git",
            "method": "push",
            "params": {},
            "priority": 1
        });

        let result = SkillEventRegistry::parse_event("io.cis.task.invoke", &content);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), SkillEvent::TaskInvoke(_)));
    }

    #[test]
    fn test_parse_unknown_event() {
        let content = serde_json::json!({});
        let result = SkillEventRegistry::parse_event("io.cis.unknown.event", &content);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SkillEventError::UnknownEventType(_)));
    }

    #[test]
    fn test_memory_update_event() {
        let event = MemoryUpdateEventContent {
            key: "user-preference".to_string(),
            domain: "private".to_string(),
            category: "settings".to_string(),
            summary: "Dark mode enabled".to_string(),
            full_content_hash: "sha256:abc123".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: MemoryUpdateEventContent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, decoded);
    }
}
