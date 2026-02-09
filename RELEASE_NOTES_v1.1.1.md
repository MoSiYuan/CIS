# CIS v1.1.1 Release Notes

**Release Date**: 2026-02-09  
**Version**: 1.1.1  
**Codename**: Agent Teams

---

## 🚀 New Features

### Agent Teams Support

CIS v1.1.1 introduces comprehensive support for **Agent Teams** - persistent AI agents that can collaborate on complex tasks.

#### Persistent Agent Runtime

- **Claude Runtime**: PTY-based persistent agent sessions
  - Full attach/detach support for interactive sessions
  - Session multiplexing for concurrent tasks
  - Automatic session recovery

- **OpenCode Runtime**: HTTP-based persistent server
  - Auto port discovery and password generation
  - HTTP Basic Auth support
  - Connect to existing OpenCode servers

#### Multi-Agent DAG Executor

Execute complex workflows with multiple AI agents:

```yaml
# Example DAG with mixed agent types
tasks:
  - id: generate-code
    agent_runtime: claude
    prompt: "Generate a Python function..."
    
  - id: review-code
    agent_runtime: opencode
    reuse_agent: opencode-reviewer
    depends_on: [generate-code]
    prompt: "Review this code..."
```

- Per-task agent runtime selection
- Agent reuse across tasks for efficiency
- Context injection and result aggregation
- Health monitoring and automatic recovery

#### Agent Federation Protocol

Cross-node agent communication via Matrix Federation:

- Agent discovery across nodes
- Task delegation to remote agents
- Heartbeat and status monitoring
- Secure event-based communication

#### Docker Test Environment

Complete 3-node Docker Compose setup:

```bash
cd test_docker/agent_teams
docker-compose up -d
```

- Coordinator node (172.30.0.10)
- Worker node 1 (172.30.0.11)
- Worker node 2 (172.30.0.12)
- Real integration tests with actual AI tools

### sqlite-vec Integration

Local vector engine for semantic search:

- Memory semantic indexing and search
- Task similarity matching
- HNSW index support for fast approximate search
- No external dependencies - 100% local

---

## 📊 Test Results

| Category | Tests | Status |
|----------|-------|--------|
| Unit Tests | 551 | ✅ Pass |
| Vector Tests | 12 | ✅ Pass |
| Integration Tests | 8 | ✅ Pass |
| **Total** | **571** | **100%** |

---

## 🐛 Bug Fixes

- Fixed federation event serialization format
- Fixed sqlite-vec conditional compilation in tests
- All 571 tests passing

---

## 📦 Installation

### Docker (Alpine-based)

```bash
docker pull cisproject/cis:1.1.1
docker run -it cisproject/cis:1.1.1
```

### Binary Release

Download from GitHub Releases:

```bash
# Linux
curl -L https://github.com/cisproject/cis/releases/download/v1.1.1/cis-1.1.1-linux-x64.tar.gz | tar xz

# macOS
curl -L https://github.com/cisproject/cis/releases/download/v1.1.1/cis-1.1.1-macos-x64.tar.gz | tar xz

# Windows
# Download cis-1.1.1-windows-x64.zip from releases page
```

### Build from Source

```bash
git clone https://github.com/cisproject/cis.git
cd cis
cargo build --release --features vector
```

---

## 🔧 Configuration

### Enable Agent Teams

Add to your `~/.cis/config.toml`:

```toml
[agent]
# Default agent runtime
default_runtime = "claude"

# Persistent agent settings
[persistent]
max_agents = 10
idle_timeout = 600

# Claude specific
[claude]
enabled = true

# OpenCode specific
[opencode]
enabled = true
```

---

## 📚 Documentation

- [Agent Teams Guide](docs/agent-teams/README.md)
- [Docker Test Environment](test_docker/agent_teams/README.md)
- [Configuration Reference](docs/configuration.md)

---

## 🔐 Security

- All agent communications use Matrix E2E encryption
- Local-only vector storage (no external API calls)
- Agent credentials isolated per session

---

## 🙏 Credits

This release was made possible by:
- The CIS Core Team
- Claude Code Assistant
- OpenCode Community

---

## 📄 License

MIT License - See [LICENSE](LICENSE) for details.
