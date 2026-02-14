# MCP Adapter Integration Guide

> **CIS Version**: v1.1.6
> **MCP Protocol**: 2024-11-05
> **Last Updated**: 2026-02-12

---

## Overview

CIS MCP Adapter provides a Model Context Protocol (MCP) server that exposes CIS capabilities (DAG, Skills, Memory, Context) to AI agents through a standardized protocol.

**What is MCP?**
- Standard protocol for AI model context management
- Enables AI agents to access tools, resources, and prompts
- JSON-RPC 2.0 based communication
- Supports multiple transport layers (stdio, SSE, WebSocket)

---

## Quick Start

### 1. Start MCP Server

```bash
# Start in stdio mode (default)
cis-mcp

# Start with verbose logging
cis-mcp --verbose
```

### 2. Configure Claude Desktop

Create/Edit `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "cis": {
      "command": "/path/to/cis-mcp",
      "args": ["--verbose"]
    }
  }
}
```

### 3. Use MCP Tools in Claude

In Claude Desktop or MCP-enabled clients:

```
User: "Create a DAG run from my workflow"

Claude: [calls tools/list] → [calls dag_create_run]
"I've created a DAG run from your workflow definition..."
```

---

## MCP Capabilities

### 1. Tools (Executable Actions)

CIS exposes the following tools via MCP:

#### DAG Tools

| Tool | Description | Parameters |
|-------|-------------|-------------|
| `dag_create_run` | Create new DAG run | `dag_file`, `run_id`, `scope` |
| `dag_get_status` | Get DAG run status | `run_id`, `include_todo` |
| `dag_control` | Control DAG (pause/resume/abort) | `run_id`, `action` |
| `dag_list` | List DAG runs | `status`, `scope`, `limit` |
| `dag_todo_propose` | Propose TODO changes | `run_id`, `changes`, `reason` |
| `dag_worker_list` | List active workers | `scope` |

#### Skill Tools

| Tool | Description | Parameters |
|-------|-------------|-------------|
| `skill_execute` | Execute CIS skill | `command`, `params` |

#### Memory Tools

| Tool | Description | Parameters |
|-------|-------------|-------------|
| `memory_store` | Store memory | `key`, `value`, `scope` |
| `memory_recall` | Recall memory | `key` |
| `context_extract` | Extract current context | (none) |

### 2. Resources (Data Access)

#### Built-in Resources

| URI | Description | Type |
|-----|-------------|------|
| `context://current` | Current project context | Dynamic JSON |
| `cis://config` | CIS configuration | Static JSON |

#### Resource Operations

```json
// List resources
{
  "method": "resources/list",
  "params": {}
}

// Read resource
{
  "method": "resources/read",
  "params": {
    "uri": "context://current"
  }
}

// Subscribe to updates
{
  "method": "resources/subscribe",
  "params": {
    "uri": "context://current",
    "subscriberId": "client-1"
  }
}
```

### 3. Prompts (Template Messages)

#### Built-in Prompts

| Name | Description | Arguments |
|------|-------------|------------|
| `summarize_code` | Summarize code snippet | `code`, `language` |
| `review_code` | Review code for issues | `code`, `focus_areas` |
| `dag_execution_plan` | Create DAG execution plan | `dag_definition`, `context` |

#### Prompt Operations

```json
// List prompts
{
  "method": "prompts/list",
  "params": {}
}

// Get prompt details
{
  "method": "prompts/get",
  "params": {
    "name": "summarize_code"
  }
}

// Render prompt with arguments
{
  "method": "prompts/render",
  "params": {
    "name": "summarize_code",
    "arguments": {
      "code": "fn main() {}",
      "language": "rust"
    }
  }
}
```

---

## Protocol Details

### JSON-RPC 2.0 Format

All MCP messages follow JSON-RPC 2.0:

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {}
}

// Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [...]
  }
}

// Error
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32601,
    "message": "Method not found"
  }
}
```

### Error Codes

| Code | Name | Description |
|------|------|-------------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Invalid JSON-RPC |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Invalid parameters |
| -32603 | Internal error | Server error |
| -32001 | Resource not found | Resource URI not found |
| -32002 | Prompt not found | Prompt name not found |
| -32003 | Tool execution error | Tool failed |
| -32004 | Subscription failed | Cannot subscribe |

---

## Advanced Usage

### Custom Resources

Register custom resources in your CIS instance:

```rust
use cis_mcp_adapter::resources::{Resource, ResourceManager};

let resource = Resource {
    uri: "myapp://config".to_string(),
    name: "App Config".to_string(),
    description: Some("Application configuration".to_string()),
    mime_type: "application/json".to_string(),
    metadata: None,
    annotations: None,
};

manager.create_resource(resource).await?;
```

### Custom Prompts

Add custom prompt templates:

```rust
use cis_mcp_adapter::prompts::{Prompt, PromptStore, PromptArgument};

let prompt = Prompt {
    name: "generate_api_docs".to_string(),
    description: "Generate API documentation".to_string(),
    arguments: vec![
        PromptArgument {
            name: "endpoint".to_string(),
            description: Some("API endpoint path".to_string()),
            required: true,
            default_value: None,
        },
        PromptArgument {
            name: "method".to_string(),
            description: Some("HTTP method".to_string()),
            required: false,
            default_value: Some(serde_json::json!("GET")),
        },
    ],
    metadata: None,
};

store.register_prompt(prompt)?;
store.register_template(template)?;
```

### Resource Subscriptions

Subscribe to resource changes (useful for dynamic resources):

```rust
// Subscribe
let sub_id = manager
    .subscribe("context://current", "my-client")
    .await?;

// List subscriptions
let subs = manager.list_subscriptions("context://current").await;

// Unsubscribe
manager.unsubscribe(&sub_id).await?;
```

---

## Transport Modes

### STDIO (Default)

Best for: Local CLI integration

```bash
cis-mcp
```

- Simple stdin/stdout communication
- One client at a time
- Low overhead

### SSE (Server-Sent Events)

Best for: Web applications

```bash
cis-mcp --transport sse --port 7676
```

- Multiple concurrent connections
- One-way server → client streaming
- Event-based updates

### WebSocket

Best for: Real-time interactive apps

```bash
cis-mcp --transport ws --port 7676
```

- Bidirectional communication
- Low latency
- Full-duplex

---

## Configuration

### Server Configuration

`~/.cis/config.toml`:

```toml
[mcp]
# Transport mode: stdio, sse, ws
transport = "stdio"

# Server settings
host = "127.0.0.1"
port = 7676

# Logging
log_level = "info"

# Limits
max_connections = 100
request_timeout_secs = 300
```

### Client Configuration

MCP client connection settings:

```json
{
  "timeout": 30000,
  "retryAttempts": 3,
  "retryDelay": 1000
}
```

---

## Testing

### Run Protocol Tests

```bash
cd crates/cis-mcp-adapter
cargo test --test mcp_protocol_tests
```

### Manual Testing

```bash
# Start server
cargo run --bin cis-mcp

# In another terminal, send requests
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' | \
  cargo run --bin cis-mcp
```

---

## Troubleshooting

### Common Issues

#### 1. "Method not found"

**Cause**: Typo in method name or not implemented

**Fix**: Check method name spelling, verify implementation exists

#### 2. "Resource not found"

**Cause**: Invalid URI or resource not registered

**Fix**: Use `resources/list` to see available URIs

#### 3. "Prompt not found"

**Cause**: Prompt name doesn't exist

**Fix**: Use `prompts/list` to see available prompts

#### 4. Subscription fails

**Cause**: Resource doesn't support subscriptions

**Fix**: Check resource metadata for subscription capability

### Debug Logging

```bash
cis-mcp --verbose 2>&1 | tee mcp-debug.log
```

---

## Performance Tips

1. **Resource Caching**: Enable caching for frequently accessed resources
2. **Batch Operations**: Group multiple tool calls in one request
3. **Connection Pooling**: Reuse connections for multiple requests
4. **Subscription Management**: Unsubscribe when done

---

## Security Considerations

### Resource Access Control

```rust
// Restrict resource access
resource.annotations = Some(ResourceAnnotations {
    role: Some("admin".to_string()),
    priority: Some(1),
    tags: None,
    custom: None,
});
```

### Tool Permissions

CIS capability layer enforces permissions:

```rust
// Caller identification
use cis_capability::CallerType;

match caller_type {
    CallerType::Mcp => /* MCP client */,
    CallerType::Cli => /* CLI */,
    CallerType::Agent => /* Agent */,
}
```

---

## Migration Guide

### From v1.1.5 to v1.1.6

**New Features**:
- ✅ Prompts module (`prompts/list`, `prompts/get`, `prompts/render`)
- ✅ Resource subscriptions (`resources/subscribe`, `resources/unsubscribe`)
- ✅ Resource metadata and annotations
- ✅ Enhanced error codes

**Breaking Changes**:
- Resource response format now includes optional `blob` field for binary data
- Prompt rendering now returns `messages` array instead of single `text`

**Migration Steps**:

1. Update client to handle new resource fields
2. Update prompt rendering to use `messages` array
3. Add error handling for new error codes (-32001 to -32004)

---

## Reference Implementation

### MCP Client (Python)

```python
import json
import subprocess

class CISMCPClient:
    def __init__(self, command=["cis-mcp"]):
        self.proc = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True
        )
        self.request_id = 0

    def call(self, method, params=None):
        self.request_id += 1
        request = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params or {}
        }

        self.proc.stdin.write(json.dumps(request) + "\n")
        self.proc.stdin.flush()

        response_line = self.proc.stdout.readline()
        return json.loads(response_line)

# Usage
client = CISMCPClient()

# List tools
result = client.call("tools/list")
print([t["name"] for t in result["result"]["tools"]])

# Create DAG run
result = client.call("tools/call", {
    "name": "dag_create_run",
    "arguments": {
        "dag_file": "./workflow.toml"
    }
})
print(result["result"]["content"][0]["text"])
```

---

## Appendix

### A. Full Tool List

```bash
# Query all tools
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | cis-mcp
```

### B. Resource URI Schemes

| Scheme | Description | Example |
|---------|-------------|----------|
| `file://` | Local filesystem | `file:///path/to/file.txt` |
| `context://` | Dynamic context | `context://current` |
| `cis://` | CIS internals | `cis://config` |
| `memory://` | Stored memories | `memory://key` |

### C. Prompt Template Syntax

```
Simple variable: {{variable_name}}

Conditional block:
{{#if condition}}
  Content when true
{{/if}}
```

---

**Related Documentation**:
- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [CIS Architecture](../architecture/overview.md)
- [DAG Guide](../user/dag-guide.md)

---

**Need Help?**
- GitHub Issues: https://github.com/your-org/cis/issues
- Documentation: https://docs.cis.dev
