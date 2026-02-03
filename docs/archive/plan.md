**AgentFlow 架构设计文档 v1.0-FINAL**  
*Polity-Based Federated Agent Architecture*

---

## 1. 核心哲学

### 1.1 主权原则（Sovereignty）
- **私域记忆绝对本地**：SQLite 加密存储，永不上传内容至云端或区块链
- **硬件绑定身份**：DID 与机器指纹（CPU/主板/网卡）绑定，复制配置即失效  
  *转世机制*：助记词可恢复**记忆访问权**，但生成**新 DID**（新硬件 = 新身份，需重新加入联邦）
- **节点即主权实体**：进程生命周期与物理硬件同生共死，无容器化抽象

### 1.2 三重解耦（Triple Decoupling）
```
IM平台 ↔ Bot ↔ Bot
 │         │      │
 │         │      └─ P2P接口（gRPC/Protobuf，零Token）
 │         └─ 私域记忆（SQLite，绝对主权）
 └─ 统一事件标准（跨平台兼容）
```

### 1.3 三零原则
- **零Token（机器间）**：Protobuf二进制通信，无LLM参与
- **零轮询（表格）**：Webhook被动触发，Bot不主动查询表格状态
- **零保姆（硬件）**：电源管理、自动扩缩容由用户自行开发Skill，核心不集成

---

## 2. 分层架构

### 2.1 表现层（Presentation）— 可替换皮肤
**功能**：人机界面，不承载控制逻辑，单向数据流（展示）
- **IM Adapter**：飞书/钉钉/CLI（Moltbot模式，个人Bot轻量接入）
- **在线表格**：飞书多维表格/本地Web，**仅Bot写入、人眼读取**，不触发控制逻辑
- **静默协议**：Bot不主动汇报进度，仅任务完成/卡点/离线告警/被@时响应

### 2.2 控制层（Control）— 联邦核心
**功能**：任务状态机、P2P联邦、身份管理
- **Task State Machine（CRDT）**：本地维护任务状态，支持离线修改与冲突解决
- **P2P Interface（gRPC over QUIC）**：
  - `TaskService.Transfer()`：跨平台任务移交（Mac↔PC↔Linux）
  - `HealthService.Check()`：节点存活探测（二进制心跳）
- **Memory Vault**：本地SQLite加密，向量索引本地计算（零API Token）
- **DAG同步**：轻量级Merkle DAG，仅同步任务元数据（ID/Status/Hash），不共享内容

### 2.3 物理层（Physical）— 极简假设
- **存储**：本地文件系统（`~/.agentflow/`）
- **网络**：Libp2p（QUIC传输，NAT穿透）
- **硬件**：仅假设节点**在线或离线**，不管理电源/休眠/温度（用户自行Skill开发）

---

## 3. 关键机制（已修复坑点）

### 3.1 身份与恢复（硬绑定+转世）
```yaml
did:
  derivation: "mnemonic_seed + machine_fingerprint"  
  # 助记词恢复私钥，但machine_fingerprint不匹配时生成新DID
  
reincarnation: 
  death: "硬件损毁且无备份 = 数字主权终结"
  rebirth: "新硬件 + 助记词 → 新DID + 旧记忆导入（手动迁移私域记忆包）"
  federation: "需重新拉群认证，广播旧节点死亡"
  
binding_verification:
  im_binding: "DID注册时绑定飞书UserID，消息双重验证（签名+IM来源）"
  anti_spoof: "DID与IM身份不匹配则标记为FORGED"
```

### 3.2 任务移交（Exactly-Once语义）
**前提**：目标节点P2P可达（在线），离线任务阻塞等待（不自动唤醒）

```protobuf
message TransferRequest {
  string task_id = 1;
  string transfer_nonce = 2;       # UUID，全局唯一，防重放
  bytes task_context = 3;          # 含内联私域记忆（见3.3）
  uint64 timestamp = 4;
  bytes authorization = 5;         # 用户签名
  string from_node = 6;
  string to_node = 7;
}

message TransferResponse {
  enum Status {
    ACCEPTED = 0;        # 首次接收
    ALREADY_EXISTS = 1;  # 幂等：此nonce已处理
    REJECTED = 2;
  }
}
```

**SQLite幂等锁**：
```sql
CREATE TABLE task_transfers (
    nonce TEXT PRIMARY KEY,  -- 重复插入即失败，实现天然幂等
    task_id TEXT NOT NULL,
    received_at INTEGER
);
```

### 3.3 私域记忆与路径（内联+相对路径）
**内联打包（Inlining）**：移交时递归打包所有引用记忆到`task_context`，目标节点解压至临时私域执行，执行完毕后按策略清理或保留。

**路径处理**：
- **禁止绝对路径**：私域记忆仅存储相对路径（`./project/data.txt`）
- **公域路径映射**：文件树结构存于DAG（含Hash），移交时根据目标节点`workspace_root`重新锚定
- **跨平台**：自动处理Windows/mac/Linux路径分隔符

### 3.4 表格交互（异步确认）
**列权限**（约定非强制，状态机以本地为准）：

| 列            | 用户   | Bot    | 说明                                |
| ------------- | ------ | ------ | ----------------------------------- |
| **UserNotes** | **写** | 读     | 指令缓冲区，Webhook触发             |
| **BotReply**  | 读     | **写** | 确认回执（🟡收到/✅完成/❌失败）       |
| **Status**    | 读     | **写** | 展示缓存，Bot**永不读取此列做决策** |

**Webhook确认机制**：
1. 用户编辑UserNotes → Webhook推送
2. Bot立即写BotReply: "🟡 收到"（确认送达）
3. Bot执行（异步）
4. Bot更新BotReply: "✅ 完成"（执行结果）

若5分钟BotReply仍为🟡或为空，用户可重试（幂等nonce防重放）。

### 3.5 联邦守望者（静默例外）
**离线告警**：若节点心跳丢失>5分钟且最后状态为Running，**其他在线节点**在IM群@用户：
> "⚠️ 节点Hugin离线（最后状态: Running, 任务T-001），任务可能中断"

**限制**：
- 仅多节点联邦时有效（单机版无此功能）
- 不自动恢复任务（符合不保姆原则）

### 3.6 资源策略（可配置）
**个人版默认**（无限制，信任用户）：
```yaml
resource_policy:
  personal:
    max_concurrent_tasks: unlimited
    max_disk_usage: unlimited
    oom_behavior: system_kill  # 系统OOM时杀进程，不干预
    
  soft_warnings:  # 可选项，仅日志提示
    disk_threshold_90: true   # 磁盘>90%时日志警告
    memory_threshold_95: true # 内存>95%时日志警告
    task_queue_depth_100: true # 积压>100时警告
```

**企业版差异**（未来闭源）：硬性配额限制（多租户必需）。

---

## 4. 部署与运维

### 4.1 裸机部署（唯一官方支持）
- **macOS**：LaunchAgent（`~/Library/LaunchAgents/`），用户级权限
- **Linux**：systemd user service（`~/.config/systemd/user/`），非root
- **Windows**：Service/NSSM（`%LOCALAPPDATA%/AgentFlow/`）

**数据主权**：
- 配置：`~/.agentflow/config.yaml`
- 身份：`~/.agentflow/identity.sec`（硬件绑定，ACL 600）
- 记忆：`~/.agentflow/memory.db`（加密SQLite）
- 日志：`~/.agentflow/logs/`

### 4.2 Docker政策（负面支持）
**官方禁止Docker**。原因：
1. **身份挥发**：容器销毁=DID丢失（硬件指纹失效）
2. **网络幻觉**：NAT下P2P不可达，联邦分裂
3. **记忆泄露**：镜像层可能残留私域数据

**用户自承风险**：执意Docker化需自行处理身份持久化（Bind Mount）、网络模式（Host/macvlan）、硬件指纹失效（DID冲突）问题，官方Issue直接关闭（标记`wontfix/docker-unsupported`）。

---

## 5. 扩展边界（Skill机制）

**核心不集成，Skill自行开发**：
- 电源管理（WOL/休眠/ACPI）
- 硬件监控（温度/GPU显存）
- 自动调度（负载均衡/预测）
- 企业认证（LDAP/SSO）

**Skill接口**：
```rust
#[skill]
fn on_task_stalled(task: &Task, ctx: &Context) {
    // 用户自行实现：发送邮件/短信/唤醒其他节点
    // 核心仅提供事件钩子，不执行动作
}
```

---

## 6. 里程碑（精简）

### Phase 1: 单机主权（Single Sovereignty）
- 本地SQLite记忆与向量检索
- 任务状态机（CRDT准备）
- 工作区沙箱（安全限制）
- IM接入（Moltbot模式，飞书/钉钉个人Bot）

### Phase 2: 联邦互联（Federation）
- **P2P gRPC（跨平台Mac↔PC↔Linux）**
- **DAG状态同步（二进制Gossip）**
- **任务移交（Exactly-Once，含内联记忆与相对路径）**
- 双向身份绑定（防DID伪造）
- 表格展示（Webhook确认机制）
- 联邦守望者（离线告警）

**无Phase 3**。硬件电源管理、自动休眠由用户通过Skill实现。

---

## 7. 技术栈（Rust）

```toml
[dependencies]
# 异步
tokio = { version = "1", features = ["rt-multi-thread"] }

# P2P（无区块链）
libp2p = { version = "0.52", features = ["tcp-tokio", "noise", "yamux", "mdns", "quic"] }

# 存储
rusqlite = { version = "0.30", features = ["bundled"] }

# 序列化（零Token通信）
prost = "0.12"        # Protobuf
tonic = "0.10"        # gRPC

# 加密（纯Rust）
ed25519-dalek = "2"   # DID签名
chacha20poly1305 = "0.10"  # 本地加密
argon2 = "0.5"        # 密钥派生

# 配置
config = "0.14"       # 跨平台配置路径
directories = "5"     # XDG/BaseDirs
```

**构建**：静态链接（MUSL/CRT），单文件15-25MB，GitHub Actions自动构建（macOS Universal/Linux ARM64/Windows）。

---

## 8. 风险与缓解（坑点汇总）

| 坑点             | 缓解方案                                    | 状态   |
| ---------------- | ------------------------------------------- | ------ |
| **身份恢复悖论** | 助记词恢复记忆，新硬件生成新DID（转世机制） | 已修复 |
| **任务重复执行** | Transfer_nonce + SQLite幂等锁               | 已修复 |
| **记忆幽灵依赖** | 内联打包（Inlining）所有引用                | 已修复 |
| **Webhook丢失**  | BotReply确认回执（🟡/✅）                     | 已修复 |
| **跨平台路径**   | 强制相对路径，公域DAG存储路径映射           | 已修复 |
| **静默崩溃**     | 联邦守望者（离线告警例外）                  | 已修复 |
| **DID污染**      | IM身份与DID双向绑定验证                     | 已修复 |
| **表格权限虚设** | 状态机仅信任P2P/DAG，表格为纯展示           | 已修复 |
| **版本分裂**     | Schema版本号，不兼容时标记为UpgradeRequired | 已修复 |
| **资源枯竭**     | 个人版无限制（信任用户），可配软警告        | 可配置 |

---

**文档结束**  
*架构即边界，Skill即自由。AgentFlow只保证联邦通信与主权隔离，不介入硬件生命周期与资源管理。*