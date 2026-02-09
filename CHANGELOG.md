# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.2] - 2026-02-09

### Fixed
- **CLI Provider Invocation** - Fixed CLI argument format for all AI providers
  - Removed `--` separator from Claude, Kimi, and OpenCode CLI calls
  - Prompt now passed directly as positional argument instead of after `--`
  - Fixes "no prompt provided" errors when using AI agent providers

### Changed
- **Version**: 1.1.1 → 1.1.2 for all crates

## [1.1.1] - 2026-02-09

### Added

#### Agent Teams Support
- **Persistent Agent Runtime** - Support for Claude and OpenCode persistent agents
  - Claude Runtime: PTY-based persistent sessions with attach/detach
  - OpenCode Runtime: HTTP-based persistent server with auto-port discovery
  - Agent Pool management with reuse and lifecycle control
- **Multi-Agent DAG Executor** - Execute DAG tasks with mixed agent types
  - Per-task agent runtime selection (Claude/OpenCode)
  - Agent reuse across tasks for efficiency
  - Context injection and result aggregation
- **Agent Federation Protocol** - Cross-node agent communication
  - Matrix-based federation events (heartbeat, task request/response)
  - Agent discovery and routing across nodes
- **Docker Test Environment** - 3-node Docker Compose setup for testing
  - Coordinator + 2 Workers configuration
  - Real integration tests with actual AI tools
- **sqlite-vec Integration** - Local vector engine for semantic search
  - Memory semantic indexing and search
  - Task similarity matching
  - HNSW index support

### Changed
- **Version**: 1.1.0 → 1.1.1 for all crates

## [1.1.0] - 2026-02-08

### Added

#### Core Features
- **Four-Tier Decision System** - Complete implementation of Mechanical/Recommended/Confirmed/Arbitrated decision levels
- **GUI Data Connection** - Real-time connection between GUI and Service layer (Node/DAG/Terminal)
- **P2P Networking** - Full DHT/Kademlia implementation with NAT traversal (STUN/TURN/UPnP)
- **ACL Rule Engine** - Complex rule conditions (IP/Time/Rate limits) with four network modes
- **Remote Session Management** - Multi-session support with persistence and Agent multiplexing
- **E2E Test Suite** - Comprehensive end-to-end tests using assert_cmd

#### Security
- **Security Audit Phase 5** - Complete security review
  - Dependency vulnerability scan (0 critical issues)
  - Unsafe code review (6 blocks, all necessary)
  - Input validation audit
  - Fuzzing infrastructure (3 targets)
  - Configuration security review
- **Memory Safety Fixes** - Unicode truncation, lifetime issues, boundary checks
- **WASM Sandbox** - Memory limits (512MB), execution timeout (30s), step limits

#### Ecosystem
- **Homebrew Formula** - `brew install cis` support
- **VS Code Extension** - Sidebar panel, CodeLens, command integration
- **Shell Integration** - Bash/Zsh/Fish completions, aliases, chpwd hooks
- **Docker Support** - Dockerfile, docker-compose, Dev Container
- **Documentation** - User guide, developer docs, deployment guide

### Fixed
- **600+ Test Fixes** - All core tests passing
- Unicode character boundary handling in text truncation
- Async lock handling across await points
- SQLite vector extension registration
- Type inference in cis-skill-sdk
- WebSocket connection handler borrow issues
- DAG execution async compatibility

### Changed
- **Version**: 0.1.0 → 1.1.0 for all crates
- **Performance**: Memory optimization, async improvements, storage indexing
- **Dependencies**: Removed vulnerable rsa crate (RUSTSEC-2023-0071)

### Code Quality
- **Clippy**: 100+ warnings fixed
- **Documentation**: 50+ doc tests fixed
- **Unsafe Code**: Added SAFETY comments for all 6 blocks

## [1.0.0] - 2026-02-10

### Added

#### Core Features
- **Hardware-bound distributed system** - DID identity anchored to hardware fingerprint
- **Private/Public memory system** - Encrypted private memory (chacha20poly1305) + shareable public memory
- **Skill system** - WASM-based extensible skills with manifest standard
- **WASM Runtime** - Wasmer integration with complete Host API (memory_get/set, ai_chat, log, http_post)
- **CLI tool** - Full-featured command-line interface with natural language commands

#### Vector Intelligence (CVI)
- **sqlite-vec integration** - Vector storage with HNSW indexing
- **Embedding Service** - Local (MiniLM-L6-v2) and remote (OpenAI) embeddings with fallback
- **Memory vector index** - Semantic search for memories with cross-project recovery
- **Task vector index** - Semantic indexing for tasks
- **Skill Vector Router** - Intent-based skill routing with confidence scoring
- **Skill Chain Orchestrator** - Automatic skill chain discovery and execution
- **Conversation Context** - Persistent conversation history with RAG integration

#### MATRIX Federation
- **MatrixNucleus** - Unified core for room/skill management
- **DID identity system** - Ed25519 keys with did:cis: method
- **Noise Protocol XX** - Handshake for secure connections
- **Event federation** - Broadcast with delivery guarantees
- **Sync queue** - Offline message queuing with priority

#### Storage
- **Multi-database architecture** - Core, memory, federation, skills, telemetry databases
- **WAL mode** - Write-ahead logging for crash safety
- **Hot-pluggable skills** - ATTACH/DETACH skill databases at runtime
- **Cross-database JOIN** - Query across multiple databases

#### P2P Network
- **mDNS discovery** - Local network node discovery
- **QUIC transport** - Fast, encrypted peer connections
- **Gossip protocol** - Public memory synchronization
- **CRDT** - Conflict-free data types for distributed state

#### Developer Experience
- **Init wizard** - Interactive setup with environment checks
- **Project mode** - Per-project CIS configuration in Git repositories
- **Portable mode** - Self-contained executable with local data
- **Shell aliases** - Convenient shortcuts for common commands

### Security
- Private memory encrypted with chacha20poly1305
- Node keys with 600 permissions (owner-only access)
- DID-based identity verification
- Noise protocol for encrypted connections

### Documentation
- Complete API documentation (580+ pages)
- User guides (USAGE, DEPLOYMENT, NETWORKING)
- Skill development documentation
- Architecture documentation

### Testing
- 85 unit tests (~85% coverage)
- 23 integration tests
- Cross-platform test suite

## [0.9.0] - 2026-01-20

### Added
- Initial project structure
- Basic CLI framework
- Core database schema
- Agent provider abstraction

### Changed
- Refactored from AgentFlow project
- Adopted MATRIX federation protocol

## [0.1.0] - 2026-01-01

### Added
- Initial commit
- Project skeleton
- Basic README

---

## Release Notes Format

Each release should include:

### Added
- New features

### Changed
- Changes to existing functionality

### Deprecated
- Soon-to-be removed features

### Removed
- Now removed features

### Fixed
- Bug fixes

### Security
- Security improvements

---

## Versioning Guide

- **MAJOR** version (X.0.0) - Incompatible API changes
- **MINOR** version (0.X.0) - Added functionality (backwards compatible)
- **PATCH** version (0.0.X) - Bug fixes (backwards compatible)

## Planned Releases

### v1.1.0 (2026-Q2)
- Shell completion scripts (Bash/Zsh/Fish)
- Auto-update mechanism
- Performance optimizations
- Additional Host API functions

### v1.2.0 (2026-Q3)
- P2P network full implementation
- WAN discovery (DHT)
- NAT traversal (STUN/TURN)
- Public memory synchronization

### v2.0.0 (2026-Q4)
- IM Skill complete implementation
- Cloud sync service (optional)
- Plugin marketplace
- Advanced analytics
