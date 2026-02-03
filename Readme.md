# CIS / 独联体

**单机 LLM Agent 记忆本地化辅助工具**

[![CI](https://github.com/MoSiYuan/CIS/actions/workflows/ci.yml/badge.svg)](https://github.com/MoSiYuan/CIS/actions/workflows/ci.yml)
[![Release](https://github.com/MoSiYuan/CIS/actions/workflows/release.yml/badge.svg)](https://github.com/MoSiYuan/CIS/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**基于独联体型网络架构（CIS: Cluster of Independent Systems），实现 0 Token 互联的 Agent 阵列**

---

## 🎯 核心定位

**解决跨设备幻觉（Cross-Device Hallucination）**：当同一用户在不同设备（工作站/笔记本/服务器）使用独立 Agent 时，由于上下文窗口差异、记忆检索延迟及网络分区，Agent 会生成与事实不符的内容（幻觉）。CIS 通过**硬件绑定的本地记忆 + P2P 联邦同步**，确保每个节点的记忆绝对一致且永不离开本地。

**CIS 是面向高隐私场景的单机 LLM Agent 记忆增强框架**。每个节点都是**硬件绑定的独立 Agent**，通过 **Matrix Federation + P2P 网络** 实现节点间 0 Token 成本的互联互通，在完全离线环境下构建具身智能网络。

---

## ✨ 核心特性

### 1. 单节点绝对隐私（零数据泄露）
- **私域记忆永不上云**：所有对话历史、任务状态、Skill 数据存储于本地 SQLite，ChaCha20-Poly1305 加密，物理层面禁止云端同步
- **硬件绑定防复制**：DID 身份与硬件指纹（CPU/主板/网卡）强绑定，配置复制到异构硬件立即失效，防止数据泄露
- **零云端依赖**：无需 OpenAI/Claude API 即可运行，支持本地 Ollama/Llama.cpp，断网环境完全可用

### 2. 解决跨设备幻觉（记忆一致性）
- **本地记忆内联打包**：任务跨节点移交时，相关记忆片段以二进制形式随任务上下文原子性传输，接收节点本地重建完整决策环境
- **零 LLM 状态同步**：设备间仅同步任务状态机变更（Merkle DAG 元数据），不依赖 LLM 对状态进行语义摘要，避免模型随机性引入偏差
- **确定性记忆访问**：单节点记忆访问不依赖云端向量数据库，消除跨设备上下文窗口差异导致的幻觉

### 3. 0 Token 互联（零成本组网）
- **Agent 阵列**：多节点通过 WebSocket + QUIC P2P 直接通信，无需云端中转
- **零 LLM 参与**：节点间使用 Protobuf 二进制协议，不消耗任何 LLM Token
- **联邦同步**：基于 Matrix 协议的 Room 联邦机制，任务/记忆跨节点安全流转

### 4. 独联体架构（去中心化）
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
| **离线环境** | 内网/断网环境无法使用 AI 助手 | 完全离线运行，节点间 mDNS 自动发现组网 |

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

# 2. 启动节点（自动发现局域网其他 CIS 节点）
cis node start

# 3. 使用自然语言调用 Skill（本地处理，无需联网）
cis skill do "分析今天的代码提交并生成报告"

# 4. 语义搜索本地记忆（向量检索，sqlite-vec）
cis memory search "暗黑模式相关的配置"

# 5. 查看联邦节点状态
cis node status

# 6. 向其他节点广播消息（0 Token 成本）
cis node broadcast "部署完成通知"
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
├── Vector Memory      # sqlite-vec 语义记忆存储
├── Skill Runtime      # WASM Skill 沙箱执行
├── DID Identity       # Ed25519 硬件绑定身份
└── Federation Manager # 节点间 0 Token 通信
```

---

## 🔒 安全与隐私：单节点数据绝对保障

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
- **Docker 禁用**：容器化会破坏硬件指纹真实性，系统**明确禁止虚拟化部署**
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

---

## 🛠️ 技术栈

- **语言**: Rust（零成本抽象，静态链接单二进制 ~15MB）
- **存储**: SQLite 3.40+（WAL 模式）+ sqlite-vec（向量检索）
- **网络**: Matrix Federation + QUIC P2P + mDNS 发现
- **加密**: Ed25519（签名）+ Argon2id（密钥派生）+ ChaCha20-Poly1305（对称加密）
- **序列化**: Protobuf（节点间）+ JSON（配置）

---

## 📚 文档

- [快速开始指南](docs/USAGE.md)
- [架构设计文档](docs/ARCHITECTURE.md)
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
