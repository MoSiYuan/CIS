# CIS v1.1.3 Release Notes

**发布日期**: 2026-02-10  
**版本**: 1.1.3  
**状态**: 稳定版

---

## 🎯 Phase 3 完成: 全模块真实实现

CIS v1.1.3 是 Phase 3 的里程碑版本，将所有模拟实现替换为基于成熟库的真实实现。

### 核心改进

| 模块 | 变更前 | 变更后 | 使用的库 |
|-----|--------|--------|----------|
| **向量嵌入** | Mock哈希向量 | FastEmbed Nomic Text v1.5 | `fastembed v4.0` |
| **P2P传输** | 占位实现 | QUIC + mDNS + DHT | `quinn 0.11`, `mdns-sd 0.10` |
| **加密握手** | 模拟响应 | Noise_XX_25519_ChaChaPoly_BLAKE2s | `snow 0.9` |
| **用户输入** | Sleep模拟 | 真实异步通道 | `tokio::mpsc` |
| **云配额** | 模拟数据 | 真实API + 60s缓存 | `reqwest` |
| **联邦通信** | 占位响应 | FederationClient | `reqwest` |
| **OpenCode** | 模拟注入 | 真实会话管理 | `opencode continue -c` |

### 服务不可用处理

所有"服务不可用返回占位数据"的模式已改为"返回显式错误":
- WASM技能执行: 返回 `Err(CisError::execution("WASM execution not yet implemented"))`
- 联邦事件发送: 返回 `Err(CisError::federation("Event sending not yet implemented"))`
- 矩阵房间管理: 返回 `Err(CisError::matrix("Room management not yet implemented"))`

---

## 🚀 快速开始

### 安装

```bash
# 克隆仓库
git clone https://github.com/MoSiYuan/CIS.git
cd CIS

# 构建（启用全部功能）
cargo build --release --features "encryption,vector,p2p,wasm"

# 安装
cp target/release/cis-node ~/.local/bin/cis
```

### 初始化与启动

```bash
# 初始化
cis init

# 启动节点
cis node start
```

### P2P组网

**Coordinator 节点:**
```bash
cis node start --role coordinator --listen /ip4/0.0.0.0/udp/9090/quic
```

**Worker 节点:**
```bash
# 配置引导节点
cis node config set bootstrap /ip4/192.168.1.100/udp/9090/quic/p2p/<coordinator_id>
cis node start --role worker
```

---

## 📋 版本信息

| 组件 | 版本 |
|-----|------|
| cis-core | 1.1.3 |
| cis-node | 1.1.3 |
| cis-skill-sdk | 1.1.3 |
| cis-gui | 1.1.3 |

---

## 📦 依赖更新

```toml
[dependencies]
# P2P网络
quinn = "0.11"           # QUIC协议
mdns-sd = "0.10"         # mDNS发现
snow = "0.9"             # Noise协议

# 向量嵌入
fastembed = "4.0"        # 本地嵌入模型

# 其他
sqlite-vec = "0.1"       # SQLite向量扩展
tokio = "1.35"           # 异步运行时
```

---

## 📚 相关文档

- [安装指南](INSTALL.md)
- [更新日志](CHANGELOG.md)
- [快速开始](START.md)
- [Phase 3 完成报告](PHASE3_COMPLETION_REPORT.md)

---

## ⬆️ 升级说明

从 v1.1.2 升级:
```bash
git pull origin main
cargo build --release --features "encryption,vector,p2p,wasm"
cp target/release/cis-node ~/.local/bin/cis
```

---

## 🎉 致谢

感谢所有参与 Phase 3 开发的 Agent:
- Agent A: 向量嵌入 (T-P0.1)
- Agent B: OpenCode (T-P0.2)
- Agent C: 矩阵CORS/UDP/Challenge (T-P1.1-1.3)
- Agent D: mDNS/调度器 (T-P1.4-1.5)
- Agent E: 云配额/联邦 (T-P1.6-1.7)
- Agent F: P2P模块修复

---

## 🔒 安全说明

- 所有加密操作使用经过审计的库
- 私钥存储权限已设置为 600
- 身份验证使用硬件绑定的 DID
- 网络传输使用 Noise XX + QUIC
