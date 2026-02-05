# CIS / 独联体

**单机 LLM Agent 记忆本地化辅助工具**

[![CI](https://github.com/MoSiYuan/CIS/actions/workflows/ci.yml/badge.svg)](https://github.com/MoSiYuan/CIS/actions/workflows/ci.yml)
[![Release](https://github.com/MoSiYuan/CIS/actions/workflows/release.yml/badge.svg)](https://github.com/MoSiYuan/CIS/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**基于独联体型网络架构（CIS: Cluster of Independent Systems），实现 0 Token 互联的 Agent 阵列**

---

> 🌐 [English Version](#english-version) | 中文版本（默认）

---

## 🎯 核心定位

**解决跨设备幻觉（Cross-Device Hallucination）**：当同一用户在不同设备（工作站/笔记本/服务器）使用独立 Agent 时，由于上下文窗口差异、记忆检索延迟及网络分区，Agent 会生成与事实不符的内容（幻觉）。CIS 通过**硬件绑定的本地记忆 + P2P 联邦同步**，确保每个节点的记忆绝对一致且永不离开本地。

**CIS 是面向高隐私场景的单机 LLM Agent 记忆增强框架**。每个节点都是**硬件绑定的独立 Agent**，通过 **Matrix Federation + P2P 网络** 实现节点间 0 Token 成本的互联互通。

> ⚠️ **注意**：CIS 本身不提供 LLM，需要用户自行配置 AI Provider（Claude / Kimi / OpenAI 等）。如需完全离线使用，请自行部署本地模型（Ollama / Llama.cpp 等）。

---

## ✨ 核心特性

### 1. 单节点绝对隐私（零数据泄露）
- **私域记忆永不上云**：所有对话历史、任务状态、Skill 数据存储于本地 SQLite，ChaCha20-Poly1305 加密，物理层面禁止云端同步
- **硬件绑定防复制**：DID 身份与硬件指纹（CPU/主板/网卡）强绑定，配置复制到异构硬件立即失效，防止数据泄露
- **灵活的 LLM 配置**：支持 Claude / Kimi / OpenAI 等云端 API，也支持 Ollama / Llama.cpp 等本地模型（完全离线需自行部署本地模型）

### 2. DID 网络安全（零信任架构）
- **手动 DID 白名单**：基于 out-of-band 信任的节点准入控制
- **WebSocket 握手挑战**：Ed25519 签名验证，防止中间人攻击
- **四种网络模式**：
  - `Whitelist`（白名单）- 仅允许已知节点
  - `Solitary`（独处）- 拒绝新连接
  - `Open`（开放）- 允许验证通过的节点
  - `Quarantine`（隔离）- 仅审计不拒绝
- **ACL 同步传播**：DNS 风格的权限更新传播，版本控制防回滚

### 3. 远程 Agent 会话（SSH 替代）
- **WebSocket PTY 会话**：通过 Matrix 端口 6767 建立加密终端会话
- **多 Agent 支持**：远程启动 Claude / Kimi / Aider 等 Agent
- **二进制帧传输**：低延迟、高效率的终端 I/O 转发
- **会话管理**：支持多会话、会话恢复、权限控制

### 4. GUI 管理界面
- **节点可视化**：节点状态、信任级别、连接状态一目了然
- **终端集成**：egui + Alacritty 终端，支持本地/远程会话
- **ACL 管理**：可视化白名单/黑名单管理
- **实时日志**：审计日志实时查看

### 5. 解决跨设备幻觉（记忆一致性）
- **本地记忆内联打包**：任务跨节点移交时，相关记忆片段以二进制形式随任务上下文原子性传输，接收节点本地重建完整决策环境
- **零 LLM 状态同步**：设备间仅同步任务状态机变更（Merkle DAG 元数据），不依赖 LLM 对状态进行语义摘要，避免模型随机性引入偏差
- **确定性记忆访问**：单节点记忆访问不依赖云端向量数据库，消除跨设备上下文窗口差异导致的幻觉

### 6. 0 Token 互联（节点间通信零成本）
- **Agent 阵列**：多节点通过 WebSocket + QUIC P2P 直接通信，无需云端中转
- **节点间零 Token**：节点间使用 Protobuf 二进制协议通信，**不消耗 LLM Token**（注意：调用 LLM 生成内容仍需消耗相应 Token）
- **联邦同步**：基于 Matrix 协议的 Room 联邦机制，任务/记忆跨节点安全流转

### 7. 独联体架构（去中心化）
```
┌─────────────────────────────────────────────────────────────┐
│                    CIS Agent 阵列                           │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    P2P/QUIC      ┌──────────────┐       │
│  │  节点 A      │ ◄──────────────► │  节点 B      │       │
│  │  (工作站)    │   0 Token 传输   │  (服务器)    │       │
│  └──────┬───────┘                  └──────┬───────┘       │
│         │                                  │               │
│    ┌────▼────┐                        ┌───▼────┐          │
│    │SQLite   │                        │SQLite  │          │
│    │本地记忆 │                        │本地记忆│          │
│    └─────────┘                        └────────┘          │
└─────────────────────────────────────────────────────────────┘
```

---

## 🚀 适用场景

| 场景 | 痛点 | CIS 解决方案 |
|------|------|-------------|
| **解决跨设备幻觉** | 同一用户多设备 Agent 回答不一致（如笔记本已确认的配置，台式机却给出矛盾建议） | **硬件绑定本地记忆**确保单节点记忆访问确定性，**记忆内联打包**实现任务移交时上下文原子性传输 |
| **集群开发** | 多台服务器 Agent 状态不同步 | 本地记忆 + P2P 同步，代码审查/部署状态实时共享 |
| **具身智能** | 机器人/IoT 设备隐私数据上云风险 | 边缘节点本地推理，关键数据永不离开设备 |
| **高隐私办公** | 企业代码/文档不能上传云端 LLM | 本地 Skill 处理敏感数据，仅脱敏元数据联邦同步，**物理层面禁止云端同步** |
| **离线环境** | 内网/断网环境无法使用 AI 助手 | 配合 Ollama 等本地模型完全离线运行，节点间 mDNS 自动发现组网 |

---

## 📦 快速开始

### 安装

**macOS**:
```bash
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-macos.tar.gz | tar xz
sudo mv cis /usr/local/bin/
cis init
```

**Linux**:
```bash
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-linux.tar.gz | tar xz
sudo mv cis /usr/local/bin/
cis init
```

**从源码构建**:
```bash
git clone https://github.com/MoSiYuan/CIS.git
cd CIS
cargo build --release
```

### 基本使用

```bash
# 1. 初始化节点（生成 DID + 本地数据库）
cis init

# 2. 配置 AI Provider（编辑 ~/.cis/config.toml）
# 云端 API 示例（需要联网）：
[ai]
provider = "kimi"  # 或 "claude", "openai"
api_key = "your-api-key"

# 本地模型示例（完全离线）：
[ai]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"

# 3. 启动节点（自动发现局域网其他 CIS 节点）
cis node start

# 4. 使用自然语言调用 Skill（需配置 LLM）
cis skill do "分析今天的代码提交并生成报告"

# 5. 语义搜索本地记忆（向量检索，sqlite-vec，无需 LLM）
cis memory search "暗黑模式相关的配置"

# 6. 网络 ACL 管理（新增）
cis network allow did:cis:abc123... --reason "信任的工作站"
cis network mode whitelist
cis network list

# 7. 启动 GUI（新增）
cis-gui
```

---

## 🏗️ 架构特点

### 设计目标：消除跨设备幻觉 + 保障单节点隐私

```
传统云原生 Agent                    CIS 独联体架构
─────────────────                  ─────────────────
┌─────────────┐                    ┌─────────────┐
│   云端 LLM   │◄── 上下文摘要 ───►│   云端 LLM   │   ← 幻觉来源：LLM 随机性
└──────┬──────┘                    └──────┬──────┘
       │                                  │
       ▼                                  ▼
┌─────────────┐                    ┌─────────────┐
│  云端数据库  │                    │  节点 A      │   ← 本地 SQLite
│  (共享状态)  │                    │  SQLite     │      物理隔离
└─────────────┘                    └──────┬──────┘
                                          │ P2P
                                    ┌─────▼─────┐
                                    │  节点 B    │   ← 记忆内联打包
                                    │  SQLite   │      原子性传输
                                    └───────────┘      无 LLM 参与
```

### 三零原则

| 原则 | 说明 | 技术实现 |
|------|------|---------|
| **零 Token** | 节点间通信不消耗 LLM Token | Protobuf + WebSocket 二进制协议 |
| **零云端** | 无需 AWS/Azure/云数据库，私域记忆物理隔离 | SQLite + 本地向量存储 + 硬件绑定 |
| **零幻觉** | 跨设备记忆访问确定性，状态同步不经过 LLM | Merkle DAG 元数据同步 + 记忆内联打包 |

### 核心组件

```
CIS Node Architecture
├── Matrix Core        # Matrix 协议内核（Room/联邦/Event）
├── P2P Network        # QUIC + mDNS + DHT 组网
├── Network Security   # DID 验证 + ACL + WebSocket 认证（新增）
├── Agent Session      # 远程 PTY 会话（SSH 替代）（新增）
├── GUI Application    # egui + Alacritty 终端（新增）
├── Vector Memory      # sqlite-vec 语义记忆存储
├── Skill Runtime      # WASM Skill 沙箱执行
├── DID Identity       # Ed25519 硬件绑定身份
└── Federation Manager # 节点间 0 Token 通信
```

### 网络端口

| 端口 | 用途 | 协议 |
|------|------|------|
| 6767 | Matrix Federation + Agent Session | WebSocket |
| 7676 | Matrix Client-Server API | HTTP |
| 7677 | P2P QUIC 传输 | QUIC |

---

## 🔒 安全与隐私：单节点数据绝对保障

### 网络安全机制（新增）

| 层级 | 机制 | 说明 |
|------|------|------|
| **传输层** | WebSocket + TLS | 加密传输通道 |
| **认证层** | DID Challenge/Response | Ed25519 签名验证 |
| **访问控制** | ACL 白名单 | 手动信任管理 |
| **审计层** | 安全事件日志 | 完整操作记录 |

### 跨设备幻觉防护机制

| 风险点 | 传统方案 | CIS 方案 |
|--------|---------|---------|
| **上下文窗口差异** | 不同设备独立维护对话历史 | **私域记忆本地存储**，单节点全量记忆访问 |
| **记忆检索延迟** | 依赖云端向量数据库 RTT | **本地 sqlite-vec**，<10ms 语义检索 |
| **状态同步偏差** | LLM 生成摘要同步 | **Merkle DAG 元数据**同步，零 LLM 参与 |
| **网络分区恢复** | 冲突时依赖 LLM 合并 | **CRDT 自动合并**，确定性冲突解决 |

### 隐私保障措施

- **硬件绑定**：DID = `助记词 + 机器指纹`，复制到另一台机器立即失效，**物理层面防止数据复制**
- **记忆加密**：SQLite 使用 ChaCha20-Poly1305，密钥派生自硬件指纹，**内存中不存储明文密钥**
- **零云端同步**：私域记忆**永不出网**，公域仅同步 Merkle DAG 元数据（无内容载荷）
- **不建议 Docker 虚拟环境部署**：容器化会带来严重的网络穿透问题，且破坏硬件指纹真实性
- **迁移机制**：硬件损毁后通过助记词在新硬件恢复记忆所有权，但生成**新 DID**（新硬件 = 新身份）

---

## 📊 与其他方案对比

| 特性 | CIS | AutoGPT | Dify | 其他 Multi-Agent |
|------|-----|---------|------|-----------------|
| **部署方式** | 单机二进制 | Docker/云 | Docker/云 | 云端服务 |
| **记忆存储** | 本地 SQLite | 云端/Redis | PostgreSQL | 云端数据库 |
| **组网成本** | 0 Token | N/A | API 调用费 | LLM Token 费 |
| **离线运行** | ✅ 完全支持 | ❌ | ❌ | ❌ |
| **隐私保护** | 硬件绑定 | 云端存储 | 自托管可选 | 依赖云端 |
| **具身智能** | ✅ 边缘原生 | ❌ | ❌ | ❌ |
| **DID 安全** | ✅ Ed25519 | ❌ | ❌ | ❌ |
| **远程会话** | ✅ WebSocket PTY | ❌ | ❌ | ❌ |
| **GUI** | ✅ egui | ❌ | ✅ | 部分支持 |

---

## 🛠️ 技术栈

- **语言**: Rust（零成本抽象，静态链接单二进制 ~15MB）
- **存储**: SQLite 3.40+（WAL 模式）+ sqlite-vec（向量检索）
- **网络**: Matrix Federation + QUIC P2P + mDNS 发现 + WebSocket
- **加密**: Ed25519（签名）+ Argon2id（密钥派生）+ ChaCha20-Poly1305（对称加密）
- **GUI**: egui 0.31 + eframe + Alacritty 终端
- **序列化**: Protobuf（节点间）+ JSON（配置）

---

## 📚 文档

- [快速开始指南](docs/USAGE.md)
- [架构设计文档](docs/ARCHITECTURE.md)
- [网络安全设计](plan/NETWORK_ACCESS_DESIGN.md)（新增）
- [GUI+安全设计](plan/GUI_SECURITY_DESIGN.md)（新增）
- [Matrix Federation 实现](docs/MATRIX_FEDERATION_IMPROVEMENT_PLAN.md)
- [生产就绪检查](docs/PRODUCTION_READINESS.md)
- [开发文档](docs/STORAGE_DESIGN.md)

---

## 🤝 参与贡献

我们欢迎 Issue 和 PR！请先阅读 [贡献指南](CONTRIBUTING.md)。

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE)

---

**CIS: 让每一台机器都成为独立的智能体，无需云端，即刻互联。**

---

# English Version

**Local LLM Agent Memory Enhancement Tool**

[![CI](https://github.com/MoSiYuan/CIS/actions/workflows/ci.yml/badge.svg)](https://github.com/MoSiYuan/CIS/actions/workflows/ci.yml)
[![Release](https://github.com/MoSiYuan/CIS/actions/workflows/release.yml/badge.svg)](https://github.com/MoSiYuan/CIS/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**Based on CIS Architecture (Cluster of Independent Systems), enabling 0-Token interconnected Agent clusters**

---

## 🎯 Core Positioning

**Solving Cross-Device Hallucination**: When the same user uses independent Agents on different devices (workstation/laptop/server), context window differences, memory retrieval delays, and network partitions cause Agents to generate factually incorrect content (hallucinations). CIS ensures absolute memory consistency and local-only storage through **hardware-bound local memory + P2P federation sync**.

**CIS is a local LLM Agent memory enhancement framework for high-privacy scenarios**. Each node is a **hardware-bound independent Agent**, interconnected via **Matrix Federation + P2P Network** at 0 Token cost.

> ⚠️ **Note**: CIS itself does not provide an LLM. Users must configure their own AI Provider (Claude / Kimi / OpenAI, etc.). For fully offline operation, please deploy local models (Ollama / Llama.cpp, etc.) yourself.

---

## ✨ Core Features

### 1. Absolute Single-Node Privacy (Zero Data Leakage)
- **Private Memory Never Clouds**: All conversation history, task states, and Skill data stored in local SQLite with ChaCha20-Poly1305 encryption; physical prohibition of cloud sync
- **Hardware Binding Anti-Copy**: DID identity strongly bound to hardware fingerprints (CPU/motherboard/NIC); configuration copied to different hardware immediately fails, preventing data leakage
- **Flexible LLM Configuration**: Supports cloud APIs like Claude / Kimi / OpenAI, as well as local models like Ollama / Llama.cpp (fully offline operation requires deploying local models yourself)

### 2. DID Network Security (Zero-Trust Architecture)
- **Manual DID Whitelist**: Node admission control based on out-of-band trust
- **WebSocket Handshake Challenge**: Ed25519 signature verification, preventing MITM attacks
- **Four Network Modes**:
  - `Whitelist` - Only known nodes allowed
  - `Solitary` - Reject new connections
  - `Open` - Allow verified nodes
  - `Quarantine` - Audit only, no rejection
- **ACL Sync Propagation**: DNS-style permission update propagation with version control to prevent rollback

### 3. Remote Agent Sessions (SSH Alternative)
- **WebSocket PTY Sessions**: Encrypted terminal sessions via Matrix port 6767
- **Multi-Agent Support**: Remotely launch Claude / Kimi / Aider Agents
- **Binary Frame Transport**: Low-latency, high-efficiency terminal I/O forwarding
- **Session Management**: Multi-session support, session recovery, permission control

### 4. GUI Management Interface
- **Node Visualization**: Node status, trust levels, connection states at a glance
- **Terminal Integration**: egui + Alacritty terminal, supporting local/remote sessions
- **ACL Management**: Visual whitelist/blacklist management
- **Real-time Logs**: Real-time audit log viewing

### 5. Solving Cross-Device Hallucination (Memory Consistency)
- **Inline Memory Packing**: When tasks transfer across nodes, relevant memory fragments are transmitted atomically with task context in binary form; receiving nodes locally reconstruct the complete decision environment
- **Zero LLM State Sync**: Devices only sync task state machine changes (Merkle DAG metadata), not relying on LLM semantic summaries, avoiding model randomness bias
- **Deterministic Memory Access**: Single-node memory access doesn't depend on cloud vector databases, eliminating hallucinations from cross-device context window differences

### 6. 0-Token Interconnection (Zero-Cost Inter-Node Communication)
- **Agent Cluster**: Multiple nodes communicate directly via WebSocket + QUIC P2P without cloud relay
- **Zero Token Between Nodes**: Nodes use Protobuf binary protocol for communication, **consuming no LLM Tokens** (Note: Calling LLM to generate content still consumes respective Tokens)
- **Federation Sync**: Matrix protocol-based Room federation mechanism for secure task/memory transfer across nodes

### 7. CIS Architecture (Decentralized)
```
┌─────────────────────────────────────────────────────────────┐
│                    CIS Agent Cluster                        │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    P2P/QUIC      ┌──────────────┐       │
│  │  Node A      │ ◄──────────────► │  Node B      │       │
│  │  (Workstation)│   0-Token Tx    │  (Server)    │       │
│  └──────┬───────┘                  └──────┬───────┘       │
│         │                                  │               │
│    ┌────▼────┐                        ┌───▼────┐          │
│    │SQLite   │                        │SQLite  │          │
│    │Local Mem│                        │LocalMem│          │
│    └─────────┘                        └────────┘          │
└─────────────────────────────────────────────────────────────┘
```

---

## 🚀 Use Cases

| Scenario | Pain Point | CIS Solution |
|----------|------------|--------------|
| **Cross-Device Hallucination** | Same user's Agents on different devices give inconsistent answers | **Hardware-bound local memory** ensures deterministic single-node access; **inline memory packing** enables atomic context transfer |
| **Cluster Development** | Multiple server Agents out of sync | Local memory + P2P sync; code review/deployment states shared in real-time |
| **Embodied Intelligence** | Robot/IoT device privacy data cloud risks | Edge nodes do local inference; critical data never leaves the device |
| **High-Privacy Office** | Enterprise code/docs can't upload to cloud LLM | Local Skill processes sensitive data; only sanitized metadata federated; **physical prohibition of cloud sync** |
| **Offline Environment** | Intranet/disconnected environments can't use AI assistants | Fully offline operation with Ollama or other local models; nodes auto-discover via mDNS |

---

## 📦 Quick Start

### Installation

**macOS**:
```bash
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-macos.tar.gz | tar xz
sudo mv cis /usr/local/bin/
cis init
```

**Linux**:
```bash
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-linux.tar.gz | tar xz
sudo mv cis /usr/local/bin/
cis init
```

**Build from Source**:
```bash
git clone https://github.com/MoSiYuan/CIS.git
cd CIS
cargo build --release
```

### Basic Usage

```bash
# 1. Initialize node (generate DID + local database)
cis init

# 2. Configure AI Provider (edit ~/.cis/config.toml)
# Cloud API example (requires internet):
[ai]
provider = "kimi"  # or "claude", "openai"
api_key = "your-api-key"

# Local model example (fully offline):
[ai]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"

# 3. Start node (auto-discover LAN CIS nodes)
cis node start

# 4. Use natural language to invoke Skill (requires LLM configuration)
cis skill do "Analyze today's commits and generate report"

# 5. Semantic search local memory (vector retrieval, sqlite-vec, no LLM required)
cis memory search "Dark mode related configuration"

# 6. Network ACL management (NEW)
cis network allow did:cis:abc123... --reason "Trusted workstation"
cis network mode whitelist
cis network list

# 7. Launch GUI (NEW)
cis-gui
```

---

## 🏗️ Architecture

### Design Goals: Eliminate Cross-Device Hallucination + Ensure Single-Node Privacy

```
Traditional Cloud-Native Agent         CIS Architecture
───────────────────────────           ──────────────────
┌─────────────┐                       ┌─────────────┐
│   Cloud LLM  │◄── Context Summary ─►│   Cloud LLM  │   ← Hallucination Source: LLM Randomness
└──────┬──────┘                       └──────┬──────┘
       │                                     │
       ▼                                     ▼
┌─────────────┐                       ┌─────────────┐
│  Cloud DB    │                       │  Node A      │   ← Local SQLite
│  (Shared)    │                       │  SQLite     │      Physical Isolation
└─────────────┘                       └──────┬──────┘
                                             │ P2P
                                       ┌─────▼─────┐
                                       │  Node B    │   ← Inline Memory Packing
                                       │  SQLite   │      Atomic Transfer
                                       └───────────┘      No LLM Involved
```

### Three Zero Principles

| Principle | Description | Technical Implementation |
|-----------|-------------|-------------------------|
| **Zero Token** | Node communication consumes no LLM Tokens | Protobuf + WebSocket Binary Protocol |
| **Zero Cloud** | No AWS/Azure/cloud DB needed; private memory physically isolated | SQLite + Local Vector Store + Hardware Binding |
| **Zero Hallucination** | Cross-device memory access is deterministic; state sync bypasses LLM | Merkle DAG Metadata Sync + Inline Memory Packing |

### Core Components

```
CIS Node Architecture
├── Matrix Core        # Matrix protocol kernel (Room/Federation/Event)
├── P2P Network        # QUIC + mDNS + DHT networking
├── Network Security   # DID verification + ACL + WebSocket auth (NEW)
├── Agent Session      # Remote PTY sessions (SSH alternative) (NEW)
├── GUI Application    # egui + Alacritty terminal (NEW)
├── Vector Memory      # sqlite-vec semantic memory storage
├── Skill Runtime      # WASM Skill sandbox execution
├── DID Identity       # Ed25519 hardware-bound identity
└── Federation Manager # 0-Token inter-node communication
```

### Network Ports

| Port | Usage | Protocol |
|------|-------|----------|
| 6767 | Matrix Federation + Agent Session | WebSocket |
| 7676 | Matrix Client-Server API | HTTP |
| 7677 | P2P QUIC Transport | QUIC |

---

## 🔒 Security & Privacy: Absolute Single-Node Data Protection

### Network Security Mechanisms (NEW)

| Layer | Mechanism | Description |
|-------|-----------|-------------|
| **Transport** | WebSocket + TLS | Encrypted transport channel |
| **Authentication** | DID Challenge/Response | Ed25519 signature verification |
| **Access Control** | ACL Whitelist | Manual trust management |
| **Audit** | Security Event Logs | Complete operation records |

### Cross-Device Hallucination Protection

| Risk Point | Traditional | CIS Solution |
|------------|-------------|--------------|
| **Context Window Differences** | Independent history per device | **Private memory local storage**, full memory access per node |
| **Memory Retrieval Latency** | Dependent on cloud vector DB RTT | **Local sqlite-vec**, <10ms semantic retrieval |
| **State Sync Deviation** | LLM-generated summary sync | **Merkle DAG metadata** sync, zero LLM involvement |
| **Network Partition Recovery** | LLM-dependent merge on conflict | **CRDT automatic merge**, deterministic conflict resolution |

### Privacy Protection Measures

- **Hardware Binding**: DID = `mnemonic + hardware fingerprint`; copying to another machine immediately fails, **physically preventing data replication**
- **Memory Encryption**: SQLite uses ChaCha20-Poly1305; keys derived from hardware fingerprints; **plaintext keys never stored in memory**
- **Zero Cloud Sync**: Private memory **never leaves the network**; public domain only syncs Merkle DAG metadata (no content payload)
- **Docker Not Recommended**: Containerization causes serious network穿透 (NAT traversal) issues and destroys hardware fingerprint authenticity
- **Migration Mechanism**: Hardware failure recovery via mnemonic on new hardware restores memory ownership but generates **new DID** (new hardware = new identity)

---

## 📊 Comparison with Other Solutions

| Feature | CIS | AutoGPT | Dify | Other Multi-Agent |
|---------|-----|---------|------|-------------------|
| **Deployment** | Single binary | Docker/Cloud | Docker/Cloud | Cloud service |
| **Memory Storage** | Local SQLite | Cloud/Redis | PostgreSQL | Cloud database |
| **Networking Cost** | 0 Token | N/A | API fees | LLM Token fees |
| **Offline Operation** | ✅ Full support | ❌ | ❌ | ❌ |
| **Privacy Protection** | Hardware binding | Cloud storage | Self-hosted optional | Cloud dependent |
| **Embodied Intelligence** | ✅ Edge native | ❌ | ❌ | ❌ |
| **DID Security** | ✅ Ed25519 | ❌ | ❌ | ❌ |
| **Remote Sessions** | ✅ WebSocket PTY | ❌ | ❌ | ❌ |
| **GUI** | ✅ egui | ❌ | ✅ | Partial |

---

## 🛠️ Tech Stack

- **Language**: Rust (zero-cost abstraction, statically linked single binary ~15MB)
- **Storage**: SQLite 3.40+ (WAL mode) + sqlite-vec (vector retrieval)
- **Network**: Matrix Federation + QUIC P2P + mDNS discovery + WebSocket
- **Crypto**: Ed25519 (signing) + Argon2id (key derivation) + ChaCha20-Poly1305 (symmetric encryption)
- **GUI**: egui 0.31 + eframe + Alacritty terminal
- **Serialization**: Protobuf (inter-node) + JSON (config)

---

## 📚 Documentation

- [Quick Start Guide](docs/USAGE.md)
- [Architecture Design](docs/ARCHITECTURE.md)
- [Network Security Design](plan/NETWORK_ACCESS_DESIGN.md) (NEW)
- [GUI + Security Design](plan/GUI_SECURITY_DESIGN.md) (NEW)
- [Matrix Federation Implementation](docs/MATRIX_FEDERATION_IMPROVEMENT_PLAN.md)
- [Production Readiness](docs/PRODUCTION_READINESS.md)
- [Developer Docs](docs/STORAGE_DESIGN.md)

---

## 🤝 Contributing

We welcome Issues and PRs! Please read [Contributing Guide](CONTRIBUTING.md) first.

## 📄 License

MIT License - See [LICENSE](LICENSE)

---

**CIS: Making every machine an independent intelligent agent, no cloud required, interconnected instantly.**
