# CIS 安全指南

**版本**: 1.0  
**日期**: 2026-02-08  
**状态**: 已审查

---

## 概述

本指南提供 CIS (Cluster of Independent Systems) 的安全配置建议和最佳实践，帮助用户建立安全的 P2P Agent 网络。

---

## 安全配置建议

### 网络模式选择

CIS 提供四种网络模式，根据安全需求选择：

```
┌─────────────────────────────────────────────────────────────┐
│                    网络模式安全对比                          │
├─────────────┬──────────┬─────────────┬──────────────────────┤
│   模式      │ 安全等级 │  适用场景   │     注意事项         │
├─────────────┼──────────┼─────────────┼──────────────────────┤
│ Solitary    │ ★★★★★   │ 单机密钥保管 │ 完全隔离，仅本地    │
│             │          │ 高敏感环境   │ 需要手动配置连接    │
├─────────────┼──────────┼─────────────┼──────────────────────┤
│ Whitelist   │ ★★★★☆   │ 团队内部    │ 推荐生产环境        │
│             │          │ 企业网络    │ 需维护白名单        │
├─────────────┼──────────┼─────────────┼──────────────────────┤
│ Open        │ ★★☆☆☆   │ 测试环境    │ 不安全！            │
│             │          │ 公开演示    │ 仅验证 DID，不验身份│
├─────────────┼──────────┼─────────────┼──────────────────────┤
│ Quarantine  │ ★★★☆☆   │ 监控模式    │ 审计可疑节点        │
│             │          │ 过渡阶段    │ 不阻断连接          │
└─────────────┴──────────┴─────────────┴──────────────────────┘
```

#### 配置命令

```bash
# 推荐：白名单模式
cis network mode whitelist

# 最高安全：隔离模式
cis network mode solitary

# 查看当前模式
cis network status
```

### ACL 配置

#### 基础白名单配置

```bash
# 1. 添加受信任节点到白名单
cis network allow did:cis:workstation:abc123 \
    --reason "我的工作电脑" \
    --expires 90d

# 2. 添加服务器节点
cis network allow did:cis:server:def456 \
    --reason "家用服务器" \
    --expires never

# 3. 添加黑名单（可选）
cis network deny did:cis:suspicious:xyz789 \
    --reason "连接异常" \
    --expires 30d
```

#### 高级规则配置

```bash
# 创建 IP 限制规则（仅允许内网）
cis network rules add internal-only \
    --name "Internal Network Only" \
    --action allow \
    --ip-cidr "10.0.0.0/8" \
    --ip-cidr "192.168.0.0/16" \
    --ip-cidr "172.16.0.0/12" \
    --priority 100

# 创建时间窗口规则（工作时间）
cis network rules add business-hours \
    --name "Business Hours Access" \
    --action allow \
    --time-window "09:00-18:00" \
    --days "1,2,3,4,5" \
    --priority 50

# 速率限制规则
cis network rules add rate-limit \
    --name "Connection Rate Limit" \
    --action quarantine \
    --rate-limit "100/min" \
    --priority 10
```

#### ACL 同步配置

```toml
# ~/.cis/config.toml
[network.acl]
# 启用 ACL 同步
enable_sync = true

# 可信更新者（可以更新本节点 ACL 的节点）
trusted_updaters = [
    "did:cis:admin-node:admin123"
]

# 自动同步间隔（秒）
sync_interval = 300

# 最大 ACL 版本历史
max_version_history = 100
```

### 加密设置

#### 传输层加密

```toml
# ~/.cis/config.toml
[network.tls]
# 强制 TLS 1.3
min_version = "1.3"

# 证书固定（可选，提高安全性）
pin_certificate = true
cert_fingerprint = "SHA256:abc123..."

# 启用完美前向保密
enable_pfs = true

# 密钥交换算法
key_exchange = "X25519"
```

#### 数据库加密

```toml
# ~/.cis/config.toml
[storage.encryption]
# 数据库加密算法
algorithm = "ChaCha20-Poly1305"

# 密钥派生参数
kdf = "Argon2id"
kdf_iterations = 3          # Argon2id 内存迭代
kdf_memory = 65536          # 64MB 内存
kdf_parallelism = 4         # 4 线程

# 自动重新加密间隔（天）
key_rotation_days = 90
```

#### 私钥保护

```bash
# 检查私钥文件权限
ls -la ~/.cis/node.key
# 期望: -rw------- (0600)

# 如果权限不正确，修复：
chmod 600 ~/.cis/node.key

# 启用私钥密码保护
cis config set security.private_key_password true

# 定期轮换密钥（每 90 天）
cis node rotate-key --confirm
```

---

## 最佳实践

### 密钥管理

#### 1. 助记词安全

```
┌─────────────────────────────────────────────────────────────┐
│                    助记词安全存储指南                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ✅ 推荐做法:                                                 │
│  • 手写在纸上，存放在安全位置（保险箱）                       │
│  • 使用金属助记词板防火防水                                  │
│  • 分片存储（Shamir Secret Sharing）                         │
│  • 多地点备份（家 + 银行保险柜）                              │
│                                                              │
│  ❌ 禁止做法:                                                 │
│  • 拍照保存在手机或云端                                      │
│  • 通过邮件、聊天软件传输                                     │
│  • 保存在未加密的文本文件                                    │
│  • 截屏保存                                                  │
│                                                              │
│  ⚠️ 注意:                                                     │
│  • 助记词 = 完全控制权                                       │
│  • 硬件损毁后可通过助记词恢复                                 │
│  • 但会生成新的 DID（新硬件 = 新身份）                        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

#### 2. API 密钥管理

```bash
# 配置 LLM API 密钥
# ~/.cis/config.toml
[ai]
provider = "kimi"
# 使用环境变量，不要硬编码
api_key = "${KIMI_API_KEY}"

# 在 shell 中设置
export KIMI_API_KEY="sk-..."

# 或者使用系统密钥链
cis config set ai.api_key --keychain
```

#### 3. 密钥轮换计划

| 密钥类型 | 轮换频率 | 命令/方法 |
|----------|----------|-----------|
| 节点私钥 | 90 天 | `cis node rotate-key` |
| Bearer Token | 30 天 | 自动轮换 |
| API Key | 根据需要 | 手动更新配置 |
| 数据库密钥 | 90 天 | 重新初始化数据库 |

### 节点验证

#### 1. 首次连接验证

```bash
# 步骤 1: 获取对方 DID（通过安全渠道）
# 示例: 面对面交换、加密邮件、已验证的聊天

# 步骤 2: 验证 DID 格式
cis did verify did:cis:partner:node1

# 步骤 3: 添加到白名单
cis network allow did:cis:partner:node1 \
    --reason "Verified in person on 2026-02-08" \
    --expires never

# 步骤 4: 测试连接
cis p2p ping did:cis:partner:node1
```

#### 2. 持续验证

```bash
# 定期检查节点状态
cis network status

# 查看连接节点的 DID 验证状态
cis p2p peers --verified

# 检查异常连接
cis network list quarantine
```

#### 3. 指纹验证

```bash
# 显示本节点 DID 和指纹
cis node info
# DID: did:cis:my-node:abc123
# Fingerprint: SHA256:def456...

# 验证远程节点指纹
cis p2p verify-fingerprint did:cis:remote:xyz789 \
    --expected "SHA256:expected..."
```

### 审计日志

#### 1. 启用审计日志

```toml
# ~/.cis/config.toml
[logging.audit]
# 启用审计日志
enabled = true

# 审计日志级别: debug, info, warning, error
critical_only = false

# 审计事件类型
audit_events = [
    "network.connect",      # 网络连接
    "network.acl_change",   # ACL 变更
    "memory.access",        # 记忆访问
    "skill.execution",      # Skill 执行
    "config.change",        # 配置变更
    "key.rotation"          # 密钥轮换
]

# 日志保留天数
retention_days = 365

# 日志完整性检查
integrity_check = true
```

#### 2. 查看审计日志

```bash
# 查看最新审计日志
cis log audit --limit 50

# 查看特定类型事件
cis log audit --event network.acl_change

# 查看特定时间范围
cis log audit --since "2026-02-01" --until "2026-02-08"

# 导出审计日志
cis log export --format json --output audit_2026.json
```

#### 3. 审计日志示例

```json
{
  "timestamp": "2026-02-08T14:30:00Z",
  "level": "info",
  "event": "network.acl_change",
  "node_did": "did:cis:my-node:abc123",
  "action": "add_to_whitelist",
  "target_did": "did:cis:partner:node1",
  "performed_by": "user",
  "acl_version": 42,
  "signature": "ed25519:sig...",
  "result": "success"
}
```

---

## 常见攻击防护

### DDoS 防护

#### 1. 连接限制

```toml
# ~/.cis/config.toml
[network.protection]
# 最大并发连接数
max_connections = 100

# 每 IP 最大连接数
max_connections_per_ip = 10

# 连接速率限制（每分钟）
connection_rate_limit = 30

# 连接超时（秒）
connection_timeout = 30

# 空闲连接超时（秒）
idle_timeout = 300
```

#### 2. 速率限制

```bash
# 配置全局速率限制
cis config set network.rate_limit.requests_per_minute 120
cis config set network.rate_limit.burst_size 20

# 配置 P2P 消息速率限制
cis config set network.rate_limit.p2p_messages_per_sec 50
```

#### 3. 资源监控

```bash
# 实时监控连接数
watch -n 1 'cis p2p status | grep connections'

# 设置告警阈值
cis alert set --metric connection_count --threshold 80

# 自动进入隔离模式（紧急情况）
cis network mode solitary --reason "DDoS detected"
```

### 中间人攻击防护

#### 1. 证书固定

```bash
# 获取远程节点证书指纹
cis p2p get-fingerprint did:cis:remote:node1
# 输出: SHA256:abc123...

# 在配置中固定证书
# ~/.cis/config.toml
[network.peer.did:cis:remote:node1]
pinned_fingerprint = "SHA256:abc123..."
verify_fingerprint = true
```

#### 2. 强制加密

```toml
# ~/.cis/config.toml
[network.security]
# 拒绝未加密连接
require_encryption = true

# 禁用明文回退
allow_plaintext = false

# 强制证书验证
verify_certificates = true

# 禁用不安全的加密算法
disabled_ciphers = ["RSA", "DES", "3DES", "RC4"]
```

#### 3. 连接验证

```bash
# 每次连接时验证证书
cis p2p connect did:cis:remote:node1 --verify-cert

# 检查连接安全状态
cis p2p status --security
```

### 重放攻击防护

#### 1. 时间戳验证

```toml
# ~/.cis/config.toml
[network.replay_protection]
# 启用时间戳验证
enable_timestamp_check = true

# 最大时间偏差（毫秒）
max_timestamp_skew = 30000

# 时间同步源
ntp_servers = ["pool.ntp.org", "time.google.com"]
```

#### 2. 序列号/Nonce

```rust
// 内部实现，无需配置
// 每条消息包含:
// - 单调递增序列号
// - 128-bit 随机 nonce
// - 发送时间戳
// - 过期时间
```

#### 3. 防重放缓存

```toml
# ~/.cis/config.toml
[network.replay_protection]
# 防重放缓存大小
nonce_cache_size = 10000

# Nonce 过期时间（秒）
nonce_ttl = 300

# 清理间隔（秒）
cleanup_interval = 60
```

---

## 安全配置模板

### 高安全环境（企业/政府）

```toml
# ~/.cis/config.toml - 高安全配置
[network]
mode = "solitary"

[network.acl]
enable_sync = false  # 禁用自动同步，手动控制
trusted_updaters = []  # 仅本地管理员

[network.tls]
min_version = "1.3"
pin_certificate = true
enable_pfs = true

[network.protection]
max_connections = 10
max_connections_per_ip = 2
connection_rate_limit = 5

[storage.encryption]
algorithm = "ChaCha20-Poly1305"
kdf = "Argon2id"
kdf_iterations = 4
kdf_memory = 131072  # 128MB

[logging.audit]
enabled = true
critical_only = false
retention_days = 2555  # 7年
integrity_check = true
```

### 标准安全环境（团队）

```toml
# ~/.cis/config.toml - 标准配置
[network]
mode = "whitelist"

[network.acl]
enable_sync = true
trusted_updaters = ["did:cis:admin:node1"]

[network.tls]
min_version = "1.3"
enable_pfs = true

[network.protection]
max_connections = 50
max_connections_per_ip = 5
connection_rate_limit = 20

[storage.encryption]
algorithm = "ChaCha20-Poly1305"
kdf = "Argon2id"

[logging.audit]
enabled = true
retention_days = 365
```

### 开发/测试环境

```toml
# ~/.cis/config.toml - 开发配置
[network]
mode = "whitelist"  # 即使开发也不用 open

[network.protection]
max_connections = 100
connection_rate_limit = 1000  # 较宽松

[logging]
level = "debug"

[logging.audit]
enabled = true  # 开发时也启用，便于调试
retention_days = 30
```

---

## 参考

- [CIS 威胁模型](./THREAT_MODEL.md)
- [CIS 应急响应手册](./INCIDENT_RESPONSE.md)
- [NETWORK_ACCESS_DESIGN.md](../../plan/NETWORK_ACCESS_DESIGN.md)

---

**文档维护**: CIS 安全团队  
**最后更新**: 2026-02-08
