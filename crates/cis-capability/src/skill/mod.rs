//! Skill execution engine

use crate::types::{ExecutionRequest, ExecutionResult, SkillMatch, SkillMetadata};
use crate::types::{CapabilityError, Result};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use std::time::Instant;

/// Skill engine for executing commands
pub struct SkillEngine {
    registry: SkillRegistry,
}

impl SkillEngine {
    pub fn new() -> Self {
        Self {
            registry: SkillRegistry::new(),
        }
    }

    /// Execute a skill by name
    pub async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult> {
        let start = Instant::now();
        
        // Find the skill
        let skill = self.registry.get(&request.skill_name)
            .ok_or_else(|| CapabilityError::SkillNotFound(request.skill_name.clone()))?;

        // Determine work directory
        let work_dir = request.context.project_root
            .clone()
            .or_else(|| Some(std::env::current_dir().unwrap_or_default()))
            .unwrap();

        // Execute based on skill type
        let result = match skill.skill_type {
            SkillType::Shell => {
                self.execute_shell(&skill, &request.params, &work_dir).await?
            }
            SkillType::Builtin => {
                self.execute_builtin(&skill, &request, &work_dir).await?
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            success: result.success,
            output: result.output,
            exit_code: result.exit_code,
            work_dir: work_dir.clone(),
            duration_ms,
            metadata: result.metadata,
        })
    }

    /// Find matching skills for a description
    pub async fn discover(&self, description: &str, _context: &crate::types::ProjectContext) -> Result<Vec<SkillMatch>> {
        let mut matches = Vec::new();
        
        // Simple keyword matching (can be enhanced with vector search)
        let keywords = description.to_lowercase();
        
        for (name, skill) in &self.registry.skills {
            let score = calculate_match_score(&keywords, skill);
            if score > 0.3 {
                matches.push(SkillMatch {
                    skill_name: name.clone(),
                    confidence: score,
                    reason: format!("匹配关键词: {}", skill.metadata.description),
                    suggested_params: None,
                });
            }
        }
        
        // Sort by confidence
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        
        Ok(matches.into_iter().take(5).collect())
    }

    /// List all available skills
    pub fn list_skills(&self) -> Vec<&SkillMetadata> {
        self.registry.skills.values().map(|s| &s.metadata).collect()
    }

    /// Execute shell command
    async fn execute_shell(&self, skill: &SkillDef, params: &serde_json::Value, work_dir: &std::path::Path) -> Result<ExecutionResult> {
        // Build command
        let cmd_template = skill.command.as_ref()
            .ok_or_else(|| CapabilityError::ExecutionFailed("No command defined".to_string()))?;
        
        let command_str = render_template(cmd_template, params)?;
        
        // Parse command
        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return Err(CapabilityError::ExecutionFailed("Empty command".to_string()));
        }

        let program = parts[0];
        let args = &parts[1..];

        // Execute
        let output = Command::new(program)
            .args(args)
            .current_dir(work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        let success = output.status.success();
        let exit_code = output.status.code();
        
        let result_output = if success {
            stdout.to_string()
        } else {
            format!("Error: {}\n{}", stderr, stdout)
        };

        Ok(ExecutionResult {
            success,
            output: result_output,
            exit_code,
            work_dir: work_dir.to_path_buf(),
            duration_ms: 0,
            metadata: HashMap::new(),
        })
    }

    /// Execute builtin skill
    async fn execute_builtin(&self, skill: &SkillDef, request: &ExecutionRequest, _work_dir: &std::path::Path) -> Result<ExecutionResult> {
        match skill.name.as_str() {
            "context-extract" => {
                // Return context as JSON
                let context_json = serde_json::to_string_pretty(&request.context)?;
                Ok(ExecutionResult::success(context_json))
            }
            "memory-store" => {
                // This would need memory service injected
                Ok(ExecutionResult::success("Memory stored"))
            }
            _ => Err(CapabilityError::SkillNotFound(skill.name.clone()))
        }
    }
}

impl Default for SkillEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Skill registry
struct SkillRegistry {
    skills: HashMap<String, SkillDef>,
}

impl SkillRegistry {
    fn new() -> Self {
        let mut registry = Self {
            skills: HashMap::new(),
        };
        registry.register_builtin();
        registry
    }

    fn get(&self, name: &str) -> Option<&SkillDef> {
        self.skills.get(name)
    }

    fn register_builtin(&mut self) {
        // Register git-commit
        self.skills.insert("git-commit".to_string(), SkillDef {
            name: "git-commit".to_string(),
            metadata: SkillMetadata {
                name: "git-commit".to_string(),
                description: "提交 git 更改".to_string(),
                category: "git".to_string(),
                parameters: vec![
                    crate::types::SkillParameter {
                        name: "message".to_string(),
                        description: "提交信息".to_string(),
                        required: true,
                        param_type: "string".to_string(),
                        default_value: None,
                    }
                ],
                examples: vec!["git-commit -m 'fix bug'".to_string()],
            },
            skill_type: SkillType::Shell,
            command: Some("git commit -m '{{message}}'".to_string()),
        });

        // Register git-status
        self.skills.insert("git-status".to_string(), SkillDef {
            name: "git-status".to_string(),
            metadata: SkillMetadata {
                name: "git-status".to_string(),
                description: "查看 git 状态".to_string(),
                category: "git".to_string(),
                parameters: vec![],
                examples: vec!["git-status".to_string()],
            },
            skill_type: SkillType::Shell,
            command: Some("git status".to_string()),
        });

        // Register context-extract
        self.skills.insert("context-extract".to_string(), SkillDef {
            name: "context-extract".to_string(),
            metadata: SkillMetadata {
                name: "context-extract".to_string(),
                description: "提取项目上下文".to_string(),
                category: "builtin".to_string(),
                parameters: vec![],
                examples: vec![],
            },
            skill_type: SkillType::Builtin,
            command: None,
        });
    }
}

/// Skill definition
struct SkillDef {
    name: String,
    metadata: SkillMetadata,
    skill_type: SkillType,
    command: Option<String>,
}

#[derive(Clone, Copy)]
enum SkillType {
    Shell,
    Builtin,
}

/// Calculate simple match score
fn calculate_match_score(keywords: &str, skill: &SkillDef) -> f32 {
    let desc = skill.metadata.description.to_lowercase();
    let name = skill.metadata.name.to_lowercase();
    
    let mut score: f32 = 0.0;
    
    // Name exact match
    if keywords.contains(&name) {
        score += 0.5;
    }
    
    // Description contains keywords
    for word in keywords.split_whitespace() {
        if desc.contains(word) {
            score += 0.1;
        }
    }
    
    score.min(1.0)
}

/// Simple template rendering
fn render_template(template: &str, params: &serde_json::Value) -> Result<String> {
    let mut result = template.to_string();
    
    if let serde_json::Value::Object(map) = params {
        for (key, value) in map {
            let placeholder = format!("{{{{{}}}}}", key);
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }
    }
    
    // Check for unreplaced placeholders
    if result.contains("{{") {
        return Err(CapabilityError::ExecutionFailed(
            format!("Missing parameters in template: {}", result)
        ));
    }
    
    Ok(result)
}
