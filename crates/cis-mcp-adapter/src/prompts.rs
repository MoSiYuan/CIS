//! MCP Prompts Module
//!
//! Implements prompt template management and rendering
//! following MCP specification for prompts.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, info};

/// Prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<PromptArgument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<PromptMetadata>,
}

/// Prompt argument definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
}

/// Prompt metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<PromptExample>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptExample {
    pub description: String,
    pub arguments: HashMap<String, Value>,
    pub expected_output: String,
}

/// Prompt template for rendering
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    pub name: String,
    pub template: String,
}

/// Rendered prompt result
#[derive(Debug, Clone, Serialize)]
pub struct RenderedPrompt {
    pub name: String,
    pub description: String,
    pub messages: Vec<PromptMessage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "role")]
pub enum PromptMessage {
    #[serde(rename = "user")]
    User { content: Content },
    #[serde(rename = "assistant")]
    Assistant { content: Content },
    #[serde(rename = "system")]
    System { content: Content },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { data: String, mime_type: String },
    #[serde(rename = "resource")]
    Resource { uri: String, text: Option<String> },
}

/// Prompt store for managing prompts
pub struct PromptStore {
    prompts: HashMap<String, Prompt>,
    templates: HashMap<String, PromptTemplate>,
}

impl Default for PromptStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptStore {
    pub fn new() -> Self {
        let mut store = Self {
            prompts: HashMap::new(),
            templates: HashMap::new(),
        };

        // Register built-in prompts
        if let Err(e) = store.register_builtin_prompts() {
            tracing::error!("Failed to register built-in prompts: {}", e);
        }

        store
    }

    /// Register built-in prompt templates
    fn register_builtin_prompts(&mut self) -> Result<()> {
        // Code summarization prompt
        self.register_prompt(Prompt {
            name: "summarize_code".to_string(),
            description: "Summarize the given code snippet".to_string(),
            arguments: vec![
                PromptArgument {
                    name: "code".to_string(),
                    description: Some("The code to summarize".to_string()),
                    required: true,
                    default_value: None,
                },
                PromptArgument {
                    name: "language".to_string(),
                    description: Some("Programming language".to_string()),
                    required: false,
                    default_value: Some(Value::String("auto".to_string())),
                },
            ],
            metadata: Some(PromptMetadata {
                category: Some("code-analysis".to_string()),
                tags: Some(vec!["summary".to_string(), "code".to_string()]),
                examples: None,
            }),
        })?;

        self.register_template(PromptTemplate {
            name: "summarize_code".to_string(),
            template: r#"Please provide a concise summary of the following {{language}} code:

```{{language}}
{{code}}
```

Focus on:
1. Overall purpose
2. Key functions/methods
3. Main algorithms or patterns used
4. Any notable observations"#
                .to_string(),
        })?;

        // Code review prompt
        self.register_prompt(Prompt {
            name: "review_code".to_string(),
            description: "Review code for potential issues and improvements".to_string(),
            arguments: vec![
                PromptArgument {
                    name: "code".to_string(),
                    description: Some("The code to review".to_string()),
                    required: true,
                    default_value: None,
                },
                PromptArgument {
                    name: "focus_areas".to_string(),
                    description: Some("Specific areas to focus on (e.g., security, performance, readability)".to_string()),
                    required: false,
                    default_value: None,
                },
            ],
            metadata: Some(PromptMetadata {
                category: Some("code-review".to_string()),
                tags: Some(vec!["review".to_string(), "quality".to_string()]),
                examples: None,
            }),
        })?;

        self.register_template(PromptTemplate {
            name: "review_code".to_string(),
            template: r#"Please review the following code:

```
{{code}}
```

{{#if focus_areas}}
Focus areas: {{focus_areas}}
{{/if}}

Provide feedback on:
1. **Correctness**: Any bugs or logic errors?
2. **Security**: Potential security vulnerabilities?
3. **Performance**: Performance bottlenecks?
4. **Readability**: Code clarity and maintainability?
5. **Best Practices**: Adherence to idioms and patterns?

Format your response as a structured review with specific line references when applicable."#
                .to_string(),
        })?;

        // DAG execution prompt
        self.register_prompt(Prompt {
            name: "dag_execution_plan".to_string(),
            description: "Create an execution plan for a DAG workflow".to_string(),
            arguments: vec![
                PromptArgument {
                    name: "dag_definition".to_string(),
                    description: Some("The DAG definition".to_string()),
                    required: true,
                    default_value: None,
                },
                PromptArgument {
                    name: "context".to_string(),
                    description: Some("Additional context about the execution environment".to_string()),
                    required: false,
                    default_value: None,
                },
            ],
            metadata: Some(PromptMetadata {
                category: Some("workflow".to_string()),
                tags: Some(vec!["dag".to_string(), "planning".to_string()]),
                examples: None,
            }),
        })?;

        self.register_template(PromptTemplate {
            name: "dag_execution_plan".to_string(),
            template: r#"Based on the following DAG definition:

```toml
{{dag_definition}}
```

{{#if context}}
Context: {{context}}
{{/if}}

Create a detailed execution plan that includes:
1. **Task Dependency Analysis**: Execution order and parallelization opportunities
2. **Resource Requirements**: Estimated resources needed for each task
3. **Risk Assessment**: Potential failure points and mitigation strategies
4. **Optimization Suggestions**: Ways to improve efficiency
5. **Monitoring Plan**: Key metrics to track during execution"#
                .to_string(),
        })?;

        Ok(())
    }

    /// Register a new prompt
    pub fn register_prompt(&mut self, prompt: Prompt) -> Result<()> {
        if self.prompts.contains_key(&prompt.name) {
            return Err(anyhow!("Prompt already exists: {}", prompt.name));
        }
        self.prompts.insert(prompt.name.clone(), prompt);
        info!("Registered prompt: {}", prompt.name);
        Ok(())
    }

    /// Register a prompt template
    pub fn register_template(&mut self, template: PromptTemplate) -> Result<()> {
        if !self.prompts.contains_key(&template.name) {
            return Err(anyhow!(
                "Cannot register template for non-existent prompt: {}",
                template.name
            ));
        }
        self.templates.insert(template.name.clone(), template);
        debug!("Registered template for: {}", template.name);
        Ok(())
    }

    /// List all prompts
    pub fn list_prompts(&self) -> Vec<Prompt> {
        self.prompts.values().cloned().collect()
    }

    /// Get a specific prompt
    pub fn get_prompt(&self, name: &str) -> Option<&Prompt> {
        self.prompts.get(name)
    }

    /// Render a prompt with arguments
    pub fn render_prompt(&self, name: &str, arguments: &HashMap<String, Value>) -> Result<RenderedPrompt> {
        let prompt = self
            .prompts
            .get(name)
            .ok_or_else(|| anyhow!("Prompt not found: {}", name))?;

        // Validate required arguments
        for arg in &prompt.arguments {
            if arg.required && !arguments.contains_key(&arg.name) {
                return Err(anyhow!(
                    "Missing required argument: {} for prompt: {}",
                    arg.name,
                    name
                ));
            }
        }

        let template = self
            .templates
            .get(name)
            .ok_or_else(|| anyhow!("Template not found for prompt: {}", name))?;

        // Simple template rendering (replace {{key}} with values)
        let mut rendered = template.template.clone();
        for (key, value) in arguments {
            let placeholder = format!("{{{{{}}}}}", key);
            let value_str = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => serde_json::to_string(value).unwrap_or_default(),
            };
            rendered = rendered.replace(&placeholder, &value_str);
        }

        // Remove any unreplaced placeholders (optional arguments)
        rendered = Self::remove_conditionals(&rendered);

        Ok(RenderedPrompt {
            name: prompt.name.clone(),
            description: prompt.description.clone(),
            messages: vec![PromptMessage::User {
                content: Content::Text { text: rendered },
            }],
        })
    }

    /// Remove conditional blocks that weren't rendered
    fn remove_conditionals(template: &str) -> String {
        let mut result = template.to_string();

        // Remove {{#if key}}...{{/if}} blocks where key is not set
        let re = regex::Regex::new(r"\{\{#if\s+(\w+)\}\}.*?\{\{/if\}\}").unwrap();
        result = re.replace_all(&result, "").to_string();

        result
    }

    /// Search prompts by metadata
    pub fn search_prompts(&self, category: Option<&str>, tags: Option<&[String]>) -> Vec<Prompt> {
        self.prompts
            .values()
            .filter(|prompt| {
                if let Some(prompt_meta) = &prompt.metadata {
                    if let Some(cat) = category {
                        if prompt_meta.category.as_deref() != Some(cat) {
                            return false;
                        }
                    }
                    if let Some(search_tags) = tags {
                        if let Some(prompt_tags) = &prompt_meta.tags {
                            if !search_tags.iter().any(|t| prompt_tags.contains(t)) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                }
                true
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_registration() {
        let store = PromptStore::new();
        assert!(store.get_prompt("summarize_code").is_some());
    }

    #[test]
    fn test_prompt_rendering() {
        let store = PromptStore::new();
        let mut args = HashMap::new();
        args.insert("code".to_string(), Value::String("fn test() {}".to_string()));
        args.insert("language".to_string(), Value::String("rust".to_string()));

        let result = store.render_prompt("summarize_code", &args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "summarize_code");
    }

    #[test]
    fn test_missing_required_argument() {
        let store = PromptStore::new();
        let args = HashMap::new();
        let result = store.render_prompt("summarize_code", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_prompts() {
        let store = PromptStore::new();
        let results = store.search_prompts(Some("code-analysis"), None);
        assert!(!results.is_empty());
    }
}
