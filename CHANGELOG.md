# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.0] - 2026-02-07

### Security
- **Security Audit**: Comprehensive security review of network, storage, and WASM modules
- **Memory Encryption**: Verified ChaCha20-Poly1305 implementation for private memory
- **DID Authentication**: Validated Ed25519 signature verification and challenge-response protocol
- **ACL System**: Reviewed whitelist/blacklist access control mechanisms
- **File Permissions**: Verified 0o600 permissions for sensitive files

### Fixed
- Fixed type inference issues in cis-skill-sdk
- Fixed serde_json dependency configuration for WASM compatibility
- Fixed compiler warnings in AI and types modules
- Fixed borrow checker issue in WebSocket connection handler

### Changed
- **Version Bump**: All crates updated to v1.1.0
  - cis-core: 0.1.0 → 1.1.0
  - cis-node: 0.1.0 → 1.1.0
  - cis-skill-sdk: 0.1.0 → 1.1.0
  - cis-skill-sdk-derive: 0.1.0 → 1.1.0

### Code Quality
- Cleaned up unused variables and imports
- Updated documentation

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
