//! MCP Server implementation

use crate::mcp_protocol::*;
use cis_capability::{CapabilityLayer, CallerType};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, info};

pub struct CisMcpServer {
    capability: Arc<CapabilityLayer>,
}

impl CisMcpServer {
    pub fn new(capability: Arc<CapabilityLayer>) -> Self {
        Self { capability }
    }

    pub async fn run_stdio(&self) -> anyhow::Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        info!("CIS MCP Server started (stdio mode)");

        let stdin = BufReader::new(tokio::io::stdin());
        let mut lines = stdin.lines();
        let mut stdout = tokio::io::stdout();

        while let Some(line) = lines.next_line().await? {
            debug!("Received: {}", line);

            match self.handle_request(&line).await {
                Ok(response) => {
                    let response_json = serde_json::to_string(&response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    debug!("Sent: {}", response_json);
                }
                Err(e) => {
                    error!("Error handling request: {}", e);
                    let error_response = McpResponse::error(
                        None,
                        error_codes::INTERNAL_ERROR,
                        e.to_string(),
                    );
                    let response_json = serde_json::to_string(&error_response)?;
                    stdout.write_all(response_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_request(&self, line: &str) -> anyhow::Result<McpResponse> {
        // Parse JSON-RPC request
        let request: serde_json::Value = serde_json::from_str(line)?;
        let id = request.get("id").cloned();

        // Extract method
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing method"))?;

        debug!("Handling method: {}", method);

        match method {
            "initialize" => self.handle_initialize(id, &request).await,
            "tools/list" => self.handle_tools_list(id).await,
            "tools/call" => self.handle_tool_call(id, &request).await,
            "resources/list" => self.handle_resources_list(id).await,
            "ping" => Ok(McpResponse::success(id, json!({}))),
            _ => Ok(McpResponse::error(
                id,
                error_codes::METHOD_NOT_FOUND,
                format!("Unknown method: {}", method),
            )),
        }
    }

    async fn handle_initialize(
        &self,
        id: Option<serde_json::Value>,
        _request: &serde_json::Value,
    ) -> anyhow::Result<McpResponse> {
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: false }),
                resources: Some(ResourcesCapability {
                    subscribe: false,
                    list_changed: false,
                }),
            },
            server_info: ServerInfo {
                name: "cis-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        Ok(McpResponse::success(id, serde_json::to_value(result)?))
    }

    async fn handle_tools_list(&self, id: Option<serde_json::Value>) -> anyhow::Result<McpResponse> {
        let tools = vec![
            // DAG Tools
            Tool {
                name: "dag_create_run".to_string(),
                description: "Create a new DAG run from a DAG definition file".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "dag_file": {
                            "type": "string",
                            "description": "Path to DAG definition file (.toml or .json)"
                        },
                        "run_id": {
                            "type": "string",
                            "description": "Optional custom run ID"
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["global", "project", "user"],
                            "description": "Execution scope for worker isolation"
                        },
                        "target_node": {
                            "type": "string",
                            "description": "Target node ID (for distributed execution)"
                        }
                    },
                    "required": ["dag_file"]
                }),
            },
            Tool {
                name: "dag_get_status".to_string(),
                description: "Get DAG run status and TODO list".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "run_id": {
                            "type": "string",
                            "description": "DAG run ID"
                        },
                        "include_todo": {
                            "type": "boolean",
                            "description": "Include TODO list in response",
                            "default": true
                        }
                    },
                    "required": ["run_id"]
                }),
            },
            Tool {
                name: "dag_control".to_string(),
                description: "Control DAG run (pause/resume/abort)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "run_id": {
                            "type": "string",
                            "description": "DAG run ID"
                        },
                        "action": {
                            "type": "string",
                            "enum": ["pause", "resume", "abort"],
                            "description": "Control action"
                        }
                    },
                    "required": ["run_id", "action"]
                }),
            },
            Tool {
                name: "dag_list".to_string(),
                description: "List DAG runs with filtering".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "status": {
                            "type": "string",
                            "enum": ["running", "paused", "completed", "failed"]
                        },
                        "scope": {
                            "type": "string",
                            "description": "Filter by scope"
                        },
                        "limit": {
                            "type": "number",
                            "default": 10
                        }
                    }
                }),
            },
            Tool {
                name: "dag_todo_propose".to_string(),
                description: "Agent proposes TODO list changes to Worker (requires Worker approval)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "run_id": {
                            "type": "string",
                            "description": "DAG run ID"
                        },
                        "changes": {
                            "type": "object",
                            "description": "TODO changes",
                            "properties": {
                                "add": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string" },
                                            "description": { "type": "string" },
                                            "priority": { "type": "number" }
                                        }
                                    }
                                },
                                "modify": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string" },
                                            "description": { "type": "string" },
                                            "priority": { "type": "number" },
                                            "status": { "type": "string" }
                                        }
                                    }
                                },
                                "remove": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                }
                            }
                        },
                        "reason": {
                            "type": "string",
                            "description": "Reason for the changes"
                        }
                    },
                    "required": ["run_id", "changes", "reason"]
                }),
            },
            Tool {
                name: "dag_worker_list".to_string(),
                description: "List active DAG workers".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "description": "Filter by scope"
                        }
                    }
                }),
            },
            // Skill Tools
            Tool {
                name: "skill_execute".to_string(),
                description: "Execute a CIS skill with context awareness".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Skill name or natural language command"
                        },
                        "params": {
                            "type": "object",
                            "description": "Skill parameters"
                        }
                    },
                    "required": ["command"]
                }),
            },
            // Memory Tools
            Tool {
                name: "memory_store".to_string(),
                description: "Store a memory for later recall".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Memory key (e.g., 'test_command')"
                        },
                        "value": {
                            "type": "string",
                            "description": "Memory value"
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["global", "project", "session"],
                            "default": "project"
                        }
                    },
                    "required": ["key", "value"]
                }),
            },
            Tool {
                name: "memory_recall".to_string(),
                description: "Recall a stored memory".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Memory key to recall"
                        }
                    },
                    "required": ["key"]
                }),
            },
            Tool {
                name: "context_extract".to_string(),
                description: "Extract current project context".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ];

        Ok(McpResponse::success(id, json!({ "tools": tools })))
    }

    async fn handle_tool_call(
        &self,
        id: Option<serde_json::Value>,
        request: &serde_json::Value,
    ) -> anyhow::Result<McpResponse> {
        let params = request
            .get("params")
            .ok_or_else(|| anyhow::anyhow!("Missing params"))?;

        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        info!("Tool call: {} with {:?}", name, arguments);

        let result = match name {
            // DAG tools
            "dag_create_run" => self.dag_create_run(arguments).await,
            "dag_get_status" => self.dag_get_status(arguments).await,
            "dag_control" => self.dag_control(arguments).await,
            "dag_list" => self.dag_list(arguments).await,
            "dag_todo_propose" => self.dag_todo_propose(arguments).await,
            "dag_worker_list" => self.dag_worker_list(arguments).await,
            // Skill tools
            "skill_execute" => self.skill_execute(arguments).await,
            // Memory tools
            "memory_store" => self.memory_store(arguments).await,
            "memory_recall" => self.memory_recall(arguments).await,
            "context_extract" => self.context_extract().await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        };

        match result {
            Ok(output) => {
                let tool_result = ToolCallResult {
                    content: vec![Content::Text { text: output }],
                    is_error: Some(false),
                };
                Ok(McpResponse::success(id, serde_json::to_value(tool_result)?))
            }
            Err(e) => {
                let tool_result = ToolCallResult {
                    content: vec![Content::Text {
                        text: format!("Error: {}", e),
                    }],
                    is_error: Some(true),
                };
                Ok(McpResponse::success(id, serde_json::to_value(tool_result)?))
            }
        }
    }

    async fn handle_resources_list(
        &self,
        id: Option<serde_json::Value>,
    ) -> anyhow::Result<McpResponse> {
        let resources = vec![
            Resource {
                uri: "context://current".to_string(),
                name: "Current Context".to_string(),
                description: Some("Current project context information".to_string()),
                mime_type: "application/json".to_string(),
            },
        ];

        Ok(McpResponse::success(id, json!({ "resources": resources })))
    }

    // DAG Tool implementations

    async fn dag_create_run(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let dag_file = args
            .get("dag_file")
            .and_then(|f| f.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing dag_file"))?;

        let _run_id = args.get("run_id").and_then(|r| r.as_str());
        let scope = args.get("scope").and_then(|s| s.as_str()).unwrap_or("global");
        let _target_node = args.get("target_node").and_then(|t| t.as_str());

        // Create DAG run using cis-core
        let _scheduler = cis_core::scheduler::DagScheduler::with_persistence(
            &dirs::data_dir()
                .unwrap_or_default()
                .join("cis")
                .join("dag_runs.db")
                .to_string_lossy()
        ).map_err(|e| anyhow::anyhow!("Failed to create scheduler: {}", e))?;

        // Load DAG from file
        let dag = load_dag_from_file(dag_file).await?;
        
        // Create run (using a new mutable scheduler)
        let mut scheduler_mut = cis_core::scheduler::DagScheduler::with_persistence(
            &dirs::data_dir()
                .unwrap_or_default()
                .join("cis")
                .join("dag_runs.db")
                .to_string_lossy()
        ).map_err(|e| anyhow::anyhow!("Failed to create scheduler: {}", e))?;
        let run_id = scheduler_mut.create_run(dag);
        
        Ok(format!(
            "DAG run created:\n  Run ID: {}\n  DAG File: {}\n  Scope: {}\n  Status: Created",
            run_id, dag_file, scope
        ))
    }

    async fn dag_get_status(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let run_id = args
            .get("run_id")
            .and_then(|r| r.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing run_id"))?;

        let include_todo = args.get("include_todo").and_then(|t| t.as_bool()).unwrap_or(true);

        // Load run from database
        let scheduler = cis_core::scheduler::DagScheduler::with_persistence(
            &dirs::data_dir()
                .unwrap_or_default()
                .join("cis")
                .join("dag_runs.db")
                .to_string_lossy()
        ).map_err(|e| anyhow::anyhow!("Failed to create scheduler: {}", e))?;

        let run = scheduler.get_run(run_id)
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        let mut result = format!(
            "DAG Run Status:\n  Run ID: {}\n  Status: {:?}\n  Tasks: {}/{}",
            run.run_id,
            run.status,
            run.dag.nodes().values().filter(|n| matches!(n.status, cis_core::scheduler::DagNodeStatus::Completed)).count(),
            run.dag.node_count()
        );

        if include_todo {
            result.push_str("\n\nTODO List:");
            if run.todo_list.items.is_empty() {
                result.push_str("\n  (No TODO items)");
            } else {
                for item in &run.todo_list.items {
                    let status_icon = match item.status {
                        cis_core::scheduler::TodoItemStatus::Completed => "✓",
                        cis_core::scheduler::TodoItemStatus::InProgress => "▸",
                        cis_core::scheduler::TodoItemStatus::Blocked => "⊘",
                        _ => "○",
                    };
                    result.push_str(&format!(
                        "\n  {} [{}] {} (P{})",
                        status_icon, item.id, item.description, item.priority
                    ));
                }
            }
            
            if let Some(checkpoint) = &run.todo_list.last_checkpoint {
                result.push_str(&format!("\n\nLast Checkpoint: {}", checkpoint));
            }
            
            if !run.todo_list.agent_notes.is_empty() {
                result.push_str(&format!("\nAgent Notes: {}", run.todo_list.agent_notes));
            }

            // Show pending proposals
            if !run.todo_list.pending_proposals.is_empty() {
                result.push_str(&format!(
                    "\n\nPending Proposals: {} (awaiting Worker review)",
                    run.todo_list.pending_proposals.len()
                ));
            }
        }

        Ok(result)
    }

    async fn dag_control(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let run_id = args
            .get("run_id")
            .and_then(|r| r.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing run_id"))?;

        let action = args
            .get("action")
            .and_then(|a| a.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing action"))?;

        // Load and modify run
        let mut scheduler = cis_core::scheduler::DagScheduler::with_persistence(
            &dirs::data_dir()
                .unwrap_or_default()
                .join("cis")
                .join("dag_runs.db")
                .to_string_lossy()
        ).map_err(|e| anyhow::anyhow!("Failed to create scheduler: {}", e))?;

        let run = scheduler.get_run_mut(run_id)
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        let old_status = format!("{:?}", run.status);

        match action {
            "pause" => run.status = cis_core::scheduler::DagRunStatus::Paused,
            "resume" => run.status = cis_core::scheduler::DagRunStatus::Running,
            "abort" => run.status = cis_core::scheduler::DagRunStatus::Failed,
            _ => return Err(anyhow::anyhow!("Unknown action: {}", action)),
        }

        Ok(format!(
            "DAG run controlled:\n  Run ID: {}\n  Action: {}\n  Status: {} → {:?}",
            run_id, action, old_status, run.status
        ))
    }

    async fn dag_list(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let status_filter = args.get("status").and_then(|s| s.as_str());
        let _scope_filter = args.get("scope").and_then(|s| s.as_str());
        let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;

        let scheduler = cis_core::scheduler::DagScheduler::with_persistence(
            &dirs::data_dir()
                .unwrap_or_default()
                .join("cis")
                .join("dag_runs.db")
                .to_string_lossy()
        ).map_err(|e| anyhow::anyhow!("Failed to create scheduler: {}", e))?;

        let runs: Vec<_> = scheduler.run_ids()
            .filter(|id| {
                if let Some(run) = scheduler.get_run(id) {
                    if let Some(filter) = status_filter {
                        return format!("{:?}", run.status).to_lowercase() == filter.to_lowercase();
                    }
                    true
                } else {
                    false
                }
            })
            .take(limit)
            .collect();

        let mut result = format!("DAG Runs ({} found):\n", runs.len());
        for run_id in runs {
            if let Some(run) = scheduler.get_run(run_id) {
                result.push_str(&format!(
                    "\n  {} - {:?} ({} tasks)",
                    run_id,
                    run.status,
                    run.dag.node_count()
                ));
            }
        }

        Ok(result)
    }

    async fn dag_todo_propose(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let run_id = args
            .get("run_id")
            .and_then(|r| r.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing run_id"))?;

        let changes = args
            .get("changes")
            .ok_or_else(|| anyhow::anyhow!("Missing changes"))?;

        let reason = args
            .get("reason")
            .and_then(|r| r.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing reason"))?;

        // Create proposal
        let mut scheduler = cis_core::scheduler::DagScheduler::with_persistence(
            &dirs::data_dir()
                .unwrap_or_default()
                .join("cis")
                .join("dag_runs.db")
                .to_string_lossy()
        ).map_err(|e| anyhow::anyhow!("Failed to create scheduler: {}", e))?;

        let run = scheduler.get_run_mut(run_id)
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        // Build diff from changes
        let mut diff = cis_core::scheduler::TodoListDiff::default();

        // Parse additions
        if let Some(add) = changes.get("add").and_then(|a| a.as_array()) {
            for item in add {
                if let (Some(id), Some(desc)) = (
                    item.get("id").and_then(|i| i.as_str()),
                    item.get("description").and_then(|d| d.as_str())
                ) {
                    let priority = item.get("priority").and_then(|p| p.as_i64()).unwrap_or(0) as i32;
                    diff.added.push(cis_core::scheduler::DagTodoItem::new(
                        id.to_string(),
                        desc.to_string()
                    ).with_priority(priority));
                }
            }
        }

        // Parse modifications
        if let Some(modify) = changes.get("modify").and_then(|m| m.as_array()) {
            for item in modify {
                if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                    // Build change record
                    let change = cis_core::scheduler::TodoItemChange {
                        id: id.to_string(),
                        old_status: cis_core::scheduler::TodoItemStatus::Pending,
                        new_status: item.get("status")
                            .and_then(|s| s.as_str())
                            .and_then(parse_status)
                            .unwrap_or(cis_core::scheduler::TodoItemStatus::Pending),
                        old_priority: 0,
                        new_priority: item.get("priority").and_then(|p| p.as_i64()).unwrap_or(0) as i32,
                        old_description: "".to_string(),
                        new_description: item.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string(),
                    };
                    diff.modified.push(change);
                }
            }
        }

        // Parse removals
        if let Some(remove) = changes.get("remove").and_then(|r| r.as_array()) {
            for id in remove.iter().filter_map(|i| i.as_str()) {
                if let Some(item) = run.todo_list.get(id) {
                    diff.removed.push(item.clone());
                }
            }
        }

        // Create proposal
        let proposal = cis_core::scheduler::TodoListProposal::new(
            cis_core::scheduler::ProposalSource::RoomAgent,
            "mcp-agent",
            diff,
            reason
        );

        // Submit to Worker (will be queued for review)
        let proposal_id = run.todo_list.submit_proposal(proposal);

        Ok(format!(
            "TODO Proposal Submitted:\n  Run ID: {}\n  Proposal ID: {}\n  Status: Pending Worker Review\n  Reason: {}\n\nThe Worker Agent will review and decide whether to merge these changes.",
            run_id, proposal_id, reason
        ))
    }

    async fn dag_worker_list(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let _scope_filter = args.get("scope").and_then(|s| s.as_str());

        // Query workers from database
        let result = "Active DAG Workers:\n\n  (Worker listing requires WorkerService - implement as needed)";

        Ok(result.to_string())
    }

    // Skill Tool implementations

    async fn skill_execute(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let command = args
            .get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing command"))?;

        let params = args.get("params").cloned().unwrap_or(json!({}));

        let result = self
            .capability
            .execute(command, params, CallerType::Mcp)
            .await?;

        Ok(format!(
            "Success: {}\nOutput: {}\nDuration: {}ms",
            result.success, result.output, result.duration_ms
        ))
    }

    async fn memory_store(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let key = args
            .get("key")
            .and_then(|k| k.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing key"))?;

        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing value"))?;

        let scope = match args.get("scope").and_then(|s| s.as_str()) {
            Some("global") => cis_capability::MemoryScope::Global,
            Some("session") => cis_capability::MemoryScope::Session,
            _ => cis_capability::MemoryScope::Project,
        };

        let entry = self.capability.remember(key, value, scope).await?;

        Ok(format!("Memory stored: {} = {}", entry.key, entry.value))
    }

    async fn memory_recall(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let key = args
            .get("key")
            .and_then(|k| k.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing key"))?;

        match self.capability.recall(key).await? {
            Some(value) => Ok(format!("Memory found: {} = {}", key, value)),
            None => Ok(format!("No memory found for key: {}", key)),
        }
    }

    async fn context_extract(&self) -> anyhow::Result<String> {
        let context = self.capability.context.read().await;
        let ctx = context.detect_current().await?;

        Ok(format!(
            "Project: {:?}\nType: {:?}\nPackage Manager: {:?}\nBranch: {:?}",
            ctx.project_root, ctx.project_type, ctx.package_manager, ctx.git_branch
        ))
    }
}

/// Load DAG from file
async fn load_dag_from_file(path: &str) -> anyhow::Result<cis_core::scheduler::TaskDag> {
    use std::path::Path;
    
    let path = Path::new(path);
    let content = tokio::fs::read_to_string(path).await
        .map_err(|e| anyhow::anyhow!("Failed to read DAG file: {}", e))?;
    
    // Try to parse as different formats
    if path.extension().map_or(false, |e| e == "toml") {
        // Try to parse skill manifest first
        if let Ok(manifest) = cis_core::skill::manifest::SkillManifest::from_dag_file(path) {
            if let Some(dag_def) = manifest.dag {
                return cis_core::skill::dag::SkillDagConverter::to_task_dag(&dag_def)
                    .map_err(|e| anyhow::anyhow!("Failed to convert DAG: {}", e));
            }
        }
        
        // Try as pure DAG definition
        if let Ok(dag_def) = toml::from_str::<cis_core::skill::manifest::DagDefinition>(&content) {
            return cis_core::skill::dag::SkillDagConverter::to_task_dag(&dag_def)
                .map_err(|e| anyhow::anyhow!("Failed to convert DAG: {}", e));
        }
    }
    
    Err(anyhow::anyhow!("Unsupported DAG file format"))
}

/// Parse status string
fn parse_status(s: &str) -> Option<cis_core::scheduler::TodoItemStatus> {
    match s.to_lowercase().as_str() {
        "pending" => Some(cis_core::scheduler::TodoItemStatus::Pending),
        "in_progress" => Some(cis_core::scheduler::TodoItemStatus::InProgress),
        "completed" => Some(cis_core::scheduler::TodoItemStatus::Completed),
        "blocked" => Some(cis_core::scheduler::TodoItemStatus::Blocked),
        "skipped" => Some(cis_core::scheduler::TodoItemStatus::Skipped),
        _ => None,
    }
}
