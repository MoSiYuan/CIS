//! # Schema Discovery Module
//!
//! 命令自描述和 Schema 发现，支持 AI 自动识别 CLI 能力

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 命令 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSchema {
    /// 命令名
    pub name: String,
    /// 描述
    pub description: String,
    /// 使用示例
    pub examples: Vec<Example>,
    /// 参数
    pub parameters: Vec<ParameterSchema>,
    /// 子命令
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subcommands: Option<Vec<CommandSchema>>,
    /// 输出 Schema
    pub output: TypeSchema,
    /// 相关命令
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub related: Vec<String>,
}

/// 使用示例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    /// 描述
    pub description: String,
    /// 命令
    pub command: String,
    /// 预期输出
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// 参数 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterSchema {
    /// 参数名
    pub name: String,
    /// 描述
    pub description: String,
    /// 参数类型
    pub param_type: TypeSchema,
    /// 是否必需
    pub required: bool,
    /// 默认值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// 可能的值（枚举）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    /// 是否位置参数
    pub positional: bool,
    /// 短选项
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<char>,
    /// 长选项
    pub long: String,
}

/// 类型 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TypeSchema {
    /// 字符串
    #[serde(rename = "string")]
    String {
        #[serde(skip_serializing_if = "Option::is_none")]
        pattern: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_length: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_length: Option<usize>,
    },
    /// 整数
    #[serde(rename = "integer")]
    Integer {
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum: Option<i64>,
    },
    /// 数字
    #[serde(rename = "number")]
    Number,
    /// 布尔值
    #[serde(rename = "boolean")]
    Boolean,
    /// 数组
    #[serde(rename = "array")]
    Array {
        items: Box<TypeSchema>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_items: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_items: Option<usize>,
    },
    /// 对象
    #[serde(rename = "object")]
    Object {
        #[serde(skip_serializing_if = "Option::is_none")]
        properties: Option<HashMap<String, TypeSchema>>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        required: Vec<String>,
    },
    /// 枚举
    #[serde(rename = "enum")]
    Enum { values: Vec<String> },
    /// 联合类型
    #[serde(rename = "union")]
    Union { types: Vec<TypeSchema> },
    /// 可选类型
    #[serde(rename = "optional")]
    Optional { inner: Box<TypeSchema> },
    /// 文件路径
    #[serde(rename = "path")]
    Path {
        #[serde(rename = "path_type")]
        path_type: PathType,
    },
}

/// 路径类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathType {
    /// 文件
    File,
    /// 目录
    Directory,
    /// 任意路径
    Any,
}

/// 命令注册表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRegistry {
    /// 版本
    pub version: String,
    /// 所有命令
    pub commands: Vec<CommandSchema>,
    /// 组合模式
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub compositions: Vec<CompositionPattern>,
}

impl CommandRegistry {
    /// 创建新的注册表
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            commands: Vec::new(),
            compositions: Vec::new(),
        }
    }

    /// 注册命令
    pub fn register(&mut self, command: CommandSchema) {
        self.commands.push(command);
    }

    /// 注册组合模式
    pub fn register_composition(&mut self, pattern: CompositionPattern) {
        self.compositions.push(pattern);
    }

    /// 获取命令 Schema
    pub fn get_command(&self, name: &str) -> Option<&CommandSchema> {
        self.find_command(name, &self.commands)
    }

    fn find_command<'a>(
        &'a self,
        name: &str,
        commands: &'a [CommandSchema],
    ) -> Option<&'a CommandSchema> {
        for cmd in commands {
            if cmd.name == name {
                return Some(cmd);
            }
            if let Some(ref subs) = cmd.subcommands {
                if let Some(found) = self.find_command(name, subs) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// 获取所有命令名称（扁平化）
    pub fn list_all_commands(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.collect_command_names(&self.commands, "", &mut names);
        names
    }

    fn collect_command_names(&self, commands: &[CommandSchema], prefix: &str, names: &mut Vec<String>) {
        for cmd in commands {
            let full_name = if prefix.is_empty() {
                cmd.name.clone()
            } else {
                format!("{} {}", prefix, cmd.name)
            };
            names.push(full_name.clone());

            if let Some(ref subs) = cmd.subcommands {
                self.collect_command_names(subs, &full_name, names);
            }
        }
    }

    /// 获取 JSON Schema
    pub fn to_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "CIS CLI Schema",
            "description": "Schema for CIS CLI commands and parameters",
            "version": self.version,
            "commands": self.commands,
            "compositions": self.compositions,
        })
    }
}

/// 组合模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionPattern {
    /// 模式名
    pub name: String,
    /// 描述
    pub description: String,
    /// 示例
    pub example: String,
    /// 输入命令
    pub input: String,
    /// 输出命令
    pub output: String,
    /// 数据流描述
    pub data_flow: String,
}

/// 内置的 CIS 命令 Schema
pub fn build_cis_schema() -> CommandRegistry {
    use TypeSchema::*;

    let mut registry = CommandRegistry::new(env!("CARGO_PKG_VERSION"));

    // ========== System Commands ==========
    registry.register(CommandSchema {
        name: "system".to_string(),
        description: "System management and diagnostics".to_string(),
        examples: vec![
            Example {
                description: "Check system status".to_string(),
                command: "cis system status".to_string(),
                output: Some("System health: healthy".to_string()),
            },
            Example {
                description: "Initialize CIS directories".to_string(),
                command: "cis system init".to_string(),
                output: None,
            },
        ],
        parameters: vec![],
        subcommands: Some(vec![
            CommandSchema {
                name: "status".to_string(),
                description: "Show system status and health".to_string(),
                examples: vec![],
                parameters: vec![],
                subcommands: None,
                output: Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert("health".to_string(), Enum {
                            values: vec!["healthy".to_string(), "degraded".to_string(), "unhealthy".to_string()],
                        });
                        props.insert("version".to_string(), String { pattern: None, min_length: None, max_length: None });
                        props.insert("data_dir".to_string(), Path { path_type: PathType::Directory });
                        props
                    }),
                    required: vec!["health".to_string(), "version".to_string()],
                },
                related: vec!["check".to_string(), "init".to_string()],
            },
            CommandSchema {
                name: "init".to_string(),
                description: "Initialize CIS system directories".to_string(),
                examples: vec![],
                parameters: vec![
                    ParameterSchema {
                        name: "force".to_string(),
                        description: "Force reinitialization".to_string(),
                        param_type: Boolean,
                        required: false,
                        default: Some(serde_json::Value::Bool(false)),
                        enum_values: None,
                        positional: false,
                        short: Some('f'),
                        long: "force".to_string(),
                    },
                ],
                subcommands: None,
                output: Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert("initialized".to_string(), Boolean);
                        props.insert("data_dir".to_string(), Path { path_type: PathType::Directory });
                        props
                    }),
                    required: vec!["initialized".to_string()],
                },
                related: vec!["migrate".to_string()],
            },
            CommandSchema {
                name: "migrate".to_string(),
                description: "Migrate from legacy directory structure".to_string(),
                examples: vec![],
                parameters: vec![],
                subcommands: None,
                output: Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert("migrated".to_string(), Boolean);
                        props.insert("source".to_string(), Path { path_type: PathType::Directory });
                        props.insert("target".to_string(), Path { path_type: PathType::Directory });
                        props
                    }),
                    required: vec!["migrated".to_string()],
                },
                related: vec!["init".to_string()],
            },
        ]),
        output: Object {
            properties: None,
            required: vec![],
        },
        related: vec![],
    });

    // ========== DAG Commands ==========
    registry.register(CommandSchema {
        name: "dag".to_string(),
        description: "DAG (Directed Acyclic Graph) workflow management".to_string(),
        examples: vec![
            Example {
                description: "List all DAGs".to_string(),
                command: "cis dag list".to_string(),
                output: None,
            },
            Example {
                description: "Run a DAG".to_string(),
                command: "cis dag run --id workflow-123".to_string(),
                output: None,
            },
        ],
        parameters: vec![],
        subcommands: Some(vec![
            CommandSchema {
                name: "list".to_string(),
                description: "List all DAG definitions".to_string(),
                examples: vec![],
                parameters: vec![
                    ParameterSchema {
                        name: "format".to_string(),
                        description: "Output format".to_string(),
                        param_type: Enum {
                            values: vec!["table".to_string(), "json".to_string()],
                        },
                        required: false,
                        default: Some(serde_json::Value::String("table".to_string())),
                        enum_values: Some(vec!["table".to_string(), "json".to_string()]),
                        positional: false,
                        short: Some('f'),
                        long: "format".to_string(),
                    },
                ],
                subcommands: None,
                output: Array {
                    items: Box::new(Object {
                        properties: Some({
                            let mut props = HashMap::new();
                            props.insert("id".to_string(), String { pattern: None, min_length: None, max_length: None });
                            props.insert("name".to_string(), String { pattern: None, min_length: None, max_length: None });
                            props.insert("status".to_string(), Enum {
                                values: vec!["active".to_string(), "inactive".to_string()],
                            });
                            props
                        }),
                        required: vec!["id".to_string(), "name".to_string()],
                    }),
                    min_items: None,
                    max_items: None,
                },
                related: vec!["run".to_string()],
            },
            CommandSchema {
                name: "run".to_string(),
                description: "Run a DAG workflow".to_string(),
                examples: vec![],
                parameters: vec![
                    ParameterSchema {
                        name: "id".to_string(),
                        description: "DAG definition ID".to_string(),
                        param_type: String { pattern: None, min_length: Some(1), max_length: None },
                        required: true,
                        default: None,
                        enum_values: None,
                        positional: true,
                        short: Some('i'),
                        long: "id".to_string(),
                    },
                    ParameterSchema {
                        name: "vars".to_string(),
                        description: "Runtime variables".to_string(),
                        param_type: Array {
                            items: Box::new(String { pattern: None, min_length: None, max_length: None }),
                            min_items: None,
                            max_items: None,
                        },
                        required: false,
                        default: Some(serde_json::Value::Array(vec![])),
                        enum_values: None,
                        positional: false,
                        short: Some('v'),
                        long: "vars".to_string(),
                    },
                    ParameterSchema {
                        name: "watch".to_string(),
                        description: "Watch execution in real-time".to_string(),
                        param_type: Boolean,
                        required: false,
                        default: Some(serde_json::Value::Bool(false)),
                        enum_values: None,
                        positional: false,
                        short: Some('w'),
                        long: "watch".to_string(),
                    },
                ],
                subcommands: None,
                output: Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert("run_id".to_string(), String { pattern: None, min_length: None, max_length: None });
                        props.insert("status".to_string(), Enum {
                            values: vec!["running".to_string(), "completed".to_string(), "failed".to_string()],
                        });
                        props
                    }),
                    required: vec!["run_id".to_string()],
                },
                related: vec!["list".to_string()],
            },
            CommandSchema {
                name: "submit".to_string(),
                description: "Submit a new DAG (accepts stdin)".to_string(),
                examples: vec![
                    Example {
                        description: "Submit from file".to_string(),
                        command: "cat workflow.json | cis dag submit".to_string(),
                        output: None,
                    },
                ],
                parameters: vec![],
                subcommands: None,
                output: Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert("dag_id".to_string(), String { pattern: None, min_length: None, max_length: None });
                        props
                    }),
                    required: vec!["dag_id".to_string()],
                },
                related: vec!["run".to_string()],
            },
        ]),
        output: Object {
            properties: None,
            required: vec![],
        },
        related: vec![],
    });

    // ========== Skill Commands ==========
    registry.register(CommandSchema {
        name: "skill".to_string(),
        description: "Skill management and execution".to_string(),
        examples: vec![],
        parameters: vec![],
        subcommands: Some(vec![
            CommandSchema {
                name: "list".to_string(),
                description: "List installed skills".to_string(),
                examples: vec![],
                parameters: vec![],
                subcommands: None,
                output: Array {
                    items: Box::new(Object {
                        properties: Some({
                            let mut props = HashMap::new();
                            props.insert("name".to_string(), String { pattern: None, min_length: None, max_length: None });
                            props.insert("version".to_string(), String { pattern: None, min_length: None, max_length: None });
                            props.insert("enabled".to_string(), Boolean);
                            props
                        }),
                        required: vec!["name".to_string(), "version".to_string()],
                    }),
                    min_items: None,
                    max_items: None,
                },
                related: vec!["invoke".to_string()],
            },
            CommandSchema {
                name: "invoke".to_string(),
                description: "Invoke a skill with parameters".to_string(),
                examples: vec![
                    Example {
                        description: "Invoke a skill".to_string(),
                        command: "cis skill invoke my-skill --input '{\"key\": \"value\"}'".to_string(),
                        output: None,
                    },
                ],
                parameters: vec![
                    ParameterSchema {
                        name: "name".to_string(),
                        description: "Skill name".to_string(),
                        param_type: String { pattern: None, min_length: Some(1), max_length: None },
                        required: true,
                        default: None,
                        enum_values: None,
                        positional: true,
                        short: None,
                        long: "name".to_string(),
                    },
                    ParameterSchema {
                        name: "input".to_string(),
                        description: "Input parameters (JSON)".to_string(),
                        param_type: String { pattern: None, min_length: None, max_length: None },
                        required: false,
                        default: Some(serde_json::Value::String("{}".to_string())),
                        enum_values: None,
                        positional: false,
                        short: Some('i'),
                        long: "input".to_string(),
                    },
                    ParameterSchema {
                        name: "raw".to_string(),
                        description: "Output raw result only".to_string(),
                        param_type: Boolean,
                        required: false,
                        default: Some(serde_json::Value::Bool(false)),
                        enum_values: None,
                        positional: false,
                        short: Some('r'),
                        long: "raw".to_string(),
                    },
                ],
                subcommands: None,
                output: Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert("result".to_string(), Object { properties: None, required: vec![] });
                        props
                    }),
                    required: vec![],
                },
                related: vec!["list".to_string()],
            },
        ]),
        output: Object {
            properties: None,
            required: vec![],
        },
        related: vec![],
    });

    // ========== Memory Commands ==========
    registry.register(CommandSchema {
        name: "memory".to_string(),
        description: "Memory/vector storage operations".to_string(),
        examples: vec![],
        parameters: vec![],
        subcommands: Some(vec![
            CommandSchema {
                name: "store".to_string(),
                description: "Store content in memory".to_string(),
                examples: vec![],
                parameters: vec![
                    ParameterSchema {
                        name: "content".to_string(),
                        description: "Content to store".to_string(),
                        param_type: String { pattern: None, min_length: None, max_length: None },
                        required: true,
                        default: None,
                        enum_values: None,
                        positional: true,
                        short: None,
                        long: "content".to_string(),
                    },
                    ParameterSchema {
                        name: "tag".to_string(),
                        description: "Tags for categorization".to_string(),
                        param_type: Array {
                            items: Box::new(String { pattern: None, min_length: None, max_length: None }),
                            min_items: None,
                            max_items: None,
                        },
                        required: false,
                        default: Some(serde_json::Value::Array(vec![])),
                        enum_values: None,
                        positional: false,
                        short: Some('t'),
                        long: "tag".to_string(),
                    },
                ],
                subcommands: None,
                output: Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert("memory_id".to_string(), String { pattern: None, min_length: None, max_length: None });
                        props
                    }),
                    required: vec!["memory_id".to_string()],
                },
                related: vec!["query".to_string()],
            },
            CommandSchema {
                name: "query".to_string(),
                description: "Query memory with semantic search".to_string(),
                examples: vec![
                    Example {
                        description: "Search memories".to_string(),
                        command: "cis memory query --q \"deployment issues\" --top-k 5".to_string(),
                        output: None,
                    },
                ],
                parameters: vec![
                    ParameterSchema {
                        name: "query".to_string(),
                        description: "Search query".to_string(),
                        param_type: String { pattern: None, min_length: Some(1), max_length: None },
                        required: true,
                        default: None,
                        enum_values: None,
                        positional: false,
                        short: Some('q'),
                        long: "query".to_string(),
                    },
                    ParameterSchema {
                        name: "top_k".to_string(),
                        description: "Number of results".to_string(),
                        param_type: Integer { minimum: Some(1), maximum: Some(100) },
                        required: false,
                        default: Some(serde_json::Value::Number(10.into())),
                        enum_values: None,
                        positional: false,
                        short: Some('k'),
                        long: "top-k".to_string(),
                    },
                    ParameterSchema {
                        name: "tag".to_string(),
                        description: "Filter by tag".to_string(),
                        param_type: Optional {
                            inner: Box::new(String { pattern: None, min_length: None, max_length: None }),
                        },
                        required: false,
                        default: None,
                        enum_values: None,
                        positional: false,
                        short: Some('t'),
                        long: "tag".to_string(),
                    },
                ],
                subcommands: None,
                output: Array {
                    items: Box::new(Object {
                        properties: Some({
                            let mut props = HashMap::new();
                            props.insert("content".to_string(), String { pattern: None, min_length: None, max_length: None });
                            props.insert("score".to_string(), Number);
                            props.insert("tags".to_string(), Array {
                                items: Box::new(String { pattern: None, min_length: None, max_length: None }),
                                min_items: None,
                                max_items: None,
                            });
                            props
                        }),
                        required: vec!["content".to_string(), "score".to_string()],
                    }),
                    min_items: None,
                    max_items: None,
                },
                related: vec!["store".to_string()],
            },
        ]),
        output: Object {
            properties: None,
            required: vec![],
        },
        related: vec![],
    });

    // ========== Compositions ==========
    registry.register_composition(CompositionPattern {
        name: "skill_to_dag".to_string(),
        description: "Invoke skill and pipe result to DAG".to_string(),
        example: "cis skill invoke generator --raw | cis dag submit".to_string(),
        input: "cis skill invoke <name> [--raw]".to_string(),
        output: "cis dag submit".to_string(),
        data_flow: "Skill output → DAG definition JSON → DAG execution".to_string(),
    });

    registry.register_composition(CompositionPattern {
        name: "dag_to_memory".to_string(),
        description: "Store DAG execution results in memory".to_string(),
        example: "cis dag run --id <dag-id> --raw | cis memory store --tag dag-result".to_string(),
        input: "cis dag run <id> [--raw]".to_string(),
        output: "cis memory store [--tag <tag>]".to_string(),
        data_flow: "DAG output → JSON content → Memory storage".to_string(),
    });

    registry.register_composition(CompositionPattern {
        name: "memory_to_dag".to_string(),
        description: "Query memory and use results in DAG".to_string(),
        example: "cis memory query --q <query> | jq '.results[].content' | cis dag submit".to_string(),
        input: "cis memory query [--q <query>]".to_string(),
        output: "cis dag submit".to_string(),
        data_flow: "Memory query results → Content extraction → DAG input".to_string(),
    });

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_registry() {
        let registry = build_cis_schema();

        assert!(!registry.commands.is_empty());
        assert!(!registry.compositions.is_empty());

        // Test getting a command
        let system_cmd = registry.get_command("system");
        assert!(system_cmd.is_some());

        // Test listing all commands
        let all_commands = registry.list_all_commands();
        assert!(all_commands.contains(&"system".to_string()));
        assert!(all_commands.contains(&"dag run".to_string()));
    }

    #[test]
    fn test_json_schema_output() {
        let registry = build_cis_schema();
        let schema = registry.to_json_schema();

        assert!(schema.get("$schema").is_some());
        assert!(schema.get("commands").is_some());
    }
}
