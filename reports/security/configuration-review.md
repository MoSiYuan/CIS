# CIS 项目默认安全配置审查报告

**审查日期**: 2026-02-08  
**审查版本**: CIS Core  
**审查人员**: 安全审查工具  

---

## 执行摘要

本次审查对 CIS 项目的默认安全配置进行了全面分析，涵盖网络模式、ACL 配置、密钥管理、敏感信息处理和超时配置等方面。总体而言，项目在**默认网络模式**和**ACL 配置**方面设计合理，但在**密钥生成**、**传输加密**和**敏感信息存储**方面存在一些需要改进的问题。

### 安全评级

| 类别 | 评级 | 说明 |
|------|------|------|
| 默认网络模式 | ✅ 良好 | 默认 Whitelist 模式，最安全 |
| ACL 配置 | ✅ 良好 | 默认拒绝所有外部连接 |
| 审计日志 | ✅ 良好 | 默认启用，记录完整 |
| 密钥管理 | ⚠️ 中危 | 使用 thread_rng，明文存储 |
| 传输加密 | ❌ 高危 | WebSocket 默认不启用 TLS |
| 超时配置 | ✅ 良好 | 配置合理，可防止 DoS |

---

## 1. 默认网络模式

### 1.1 配置审查

**文件**: `cis-core/src/network/acl.rs`

```rust
#[derive(Default)]
pub enum NetworkMode {
    Open,
    #[default]
    Whitelist,  // ✅ 默认白名单模式
    Solitary,
    Quarantine,
}
```

### 1.2 安全分析

| 检查项 | 状态 | 说明 |
|--------|------|------|
| 默认网络模式 | ✅ 通过 | 默认为 `Whitelist` 模式，最安全 |
| 默认加密传输 | ❌ 失败 | WebSocket 默认使用 `ws://` 而非 `wss://` |

**详细说明**:
- ✅ **Whitelist 模式**: 默认仅允许白名单中的 DID 连接，拒绝所有未授权连接
- ✅ **黑名单优先**: ACL 检查逻辑先检查黑名单，再检查白名单，防止误配置
- ❌ **传输加密**: WebSocket 默认端口 6768 使用明文 `ws://` 协议，无 TLS 加密

### 1.3 发现的问题

**问题 #1: WebSocket 默认不使用 TLS**
- **位置**: `cis-core/src/network/websocket.rs:29`
- **风险等级**: 高
- **描述**: 默认 WebSocket 配置使用 `ws://` 明文传输，敏感数据（包括 DID 认证信息）可能被窃听
- **代码**:
```rust
pub const DEFAULT_WS_PORT: u16 = 6768;  // 未使用 wss://
```

---

## 2. 默认 ACL 配置

### 2.1 配置审查

**文件**: `cis-core/src/network/acl.rs:147-158`

```rust
impl Default for NetworkAcl {
    fn default() -> Self {
        Self {
            local_did: String::new(),
            mode: NetworkMode::Whitelist,  // ✅ 白名单模式
            whitelist: Vec::new(),         // ✅ 空白名单
            blacklist: Vec::new(),
            version: 1,
            updated_at: now(),
            signature: None,
        }
    }
}
```

### 2.2 安全分析

| 检查项 | 状态 | 说明 |
|--------|------|------|
| 默认拒绝外部连接 | ✅ 通过 | 空白名单 + Whitelist 模式 = 拒绝所有 |
| 审计日志启用 | ✅ 通过 | 有完整的审计日志系统 |
| ACL 文件权限 | ✅ 通过 | 保存时设置 0o600 权限 |

**详细说明**:
- ✅ **默认拒绝**: 新节点启动时，白名单为空，所有外部连接默认被拒绝
- ✅ **权限控制**: ACL 文件保存时设置 `0o600` 权限，仅所有者可读写
- ✅ **审计日志**: 所有 ACL 变更（添加/删除白名单、模式切换）都被记录

### 2.3 审计日志配置

**文件**: `cis-core/src/network/audit.rs:190-200`

```rust
impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            max_memory_entries: 10000,
            file_enabled: true,  // ✅ 默认启用文件日志
            file_path: crate::storage::paths::Paths::data_dir().join("audit.log"),
            max_file_size_mb: 100,
            max_rotated_files: 10,
        }
    }
}
```

**记录的安全事件**:
- DID 验证成功/失败
- 连接被 ACL 阻止
- 白名单/黑名单变更
- 网络模式变更
- 认证尝试/成功/失败

---

## 3. 密钥管理

### 3.1 密钥生成

**文件**: `cis-core/src/init/wizard.rs:431-438`

```rust
fn generate_node_key(&self) -> Result<String> {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);  // ⚠️ 使用 thread_rng
    Ok(hex::encode(key))
}
```

**文件**: `cis-core/src/identity/did.rs:32-40`

```rust
pub fn generate(node_id: impl Into<String>) -> Result<Self> {
    let mut csprng = OsRng;  // ✅ 使用 OsRng
    let signing_key = SigningKey::generate(&mut csprng);
    // ...
}
```

**文件**: `cis-core/src/network/did_verify.rs:318-324`

```rust
fn generate_nonce() -> [u8; NONCE_LENGTH] {
    use rand::RngCore;
    let mut nonce = [0u8; NONCE_LENGTH];
    rand::thread_rng().fill_bytes(&mut nonce);  // ⚠️ 使用 thread_rng
    nonce
}
```

### 3.2 密钥存储

**文件**: `cis-core/src/init/wizard.rs:483-492`

```rust
std::fs::write(&config_path, config)?;  // ❌ 明文存储密钥

// 设置权限 (仅当前用户可读写)
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(&config_path)?.permissions();
    permissions.set_mode(0o600);  // ✅ 正确设置权限
    std::fs::set_permissions(&config_path, permissions)?;
}
```

**文件**: `cis-core/src/identity/did.rs:130-137`

```rust
// 设置权限为仅所有者可读写 (0o600)
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path.with_extension("key"))?.permissions();
    perms.set_mode(0o600);  // ✅ 正确设置权限
    fs::set_permissions(path.with_extension("key"), perms)?;
}
```

### 3.3 安全分析

| 检查项 | 状态 | 说明 |
|--------|------|------|
| DID 密钥生成 | ✅ 通过 | 使用 `OsRng`，密码学安全 |
| 节点密钥生成 | ⚠️ 警告 | 使用 `thread_rng`，建议使用 `OsRng` |
| Nonce 生成 | ⚠️ 警告 | 使用 `thread_rng`，建议使用 `OsRng` |
| 密钥存储权限 | ✅ 通过 | 设置 0o600 |
| 密钥存储方式 | ❌ 失败 | 明文存储在配置文件 |

### 3.4 发现的问题

**问题 #2: 节点密钥使用非加密安全 RNG**
- **位置**: `cis-core/src/init/wizard.rs:435`
- **风险等级**: 中
- **描述**: `generate_node_key()` 使用 `rand::thread_rng()` 而非密码学安全的 `OsRng`
- **影响**: 在特定环境下，密钥可能可预测

**问题 #3: DID Challenge Nonce 使用非加密安全 RNG**
- **位置**: `cis-core/src/network/did_verify.rs:322`
- **风险等级**: 中
- **描述**: `generate_nonce()` 使用 `rand::thread_rng()`
- **影响**: Challenge nonce 可能在某些情况下可预测

**问题 #4: 节点密钥明文存储在配置文件**
- **位置**: `cis-core/src/init/wizard.rs:357-426`
- **风险等级**: 高
- **描述**: 生成的节点密钥以明文形式存储在 `~/.cis/config.toml`
- **代码**:
```toml
[node]
key = "a1b2c3d4e5f6..."  # ❌ 明文存储
```

---

## 4. 敏感信息处理

### 4.1 配置文件中的敏感信息

**文件**: `cis-core/src/init/wizard.rs:357-426`

生成的配置包含：
```toml
[node]
id = "uuid-here"
name = "username"
key = "hex-encoded-key"  # ⚠️ 敏感信息
```

### 4.2 错误消息

**文件**: `cis-core/src/error.rs`

错误消息设计较为通用，未暴露内部路径或敏感信息：
```rust
#[error("IO error: {0}")]
Io(#[from] std::io::Error),
#[error("Identity error: {0}")]
Identity(String),
```

### 4.3 日志处理

**审查结果**:
- ✅ 错误消息未包含内部文件路径
- ✅ DID 验证失败时只记录 DID，不记录密钥
- ⚠️ 审计日志可能包含敏感操作记录，但未加密存储

### 4.4 发现的问题

**问题 #5: 审计日志未加密**
- **位置**: `cis-core/src/network/audit.rs`
- **风险等级**: 中
- **描述**: 安全审计日志以明文 JSON 格式存储，包含 DID 验证记录、ACL 变更等敏感信息

---

## 5. 超时配置

### 5.1 网络超时

| 超时类型 | 默认值 | 位置 | 评价 |
|----------|--------|------|------|
| WebSocket 连接超时 | 30 秒 | `websocket.rs:32` | ✅ 合理 |
| 心跳间隔 | 5 秒 | `websocket.rs:35` | ✅ 合理 |
| 空闲超时 | 60 秒 | `websocket.rs:525` | ✅ 合理 |
| 最大重连尝试 | 10 次 | `websocket.rs:38` | ✅ 合理 |

### 5.2 认证超时

| 超时类型 | 默认值 | 位置 | 评价 |
|----------|--------|------|------|
| DID Challenge 超时 | 30 秒 | `did_verify.rs:42` | ✅ 合理 |

### 5.3 会话超时

| 超时类型 | 默认值 | 位置 | 评价 |
|----------|--------|------|------|
| Session 超时 | 3600 秒 (1小时) | `session_manager.rs:22` | ✅ 合理 |
| Checkpoint 间隔 | 300 秒 (5分钟) | `session_manager.rs:25` | ✅ 合理 |

### 5.4 安全分析

所有超时配置均设计合理，能够有效防止：
- ✅ DoS 攻击（连接超时限制）
- ✅ 资源耗尽（会话超时限制）
- ✅ 重放攻击（Challenge 超时限制）

---

## 6. 加密实现

### 6.1 记忆加密

**文件**: `cis-core/src/memory/encryption.rs`

```rust
pub struct MemoryEncryption {
    cipher: ChaCha20Poly1305,  // ✅ 使用现代 AEAD 算法
}

impl MemoryEncryption {
    pub fn from_node_key(node_key: &[u8]) -> Self {
        // 使用 HKDF 风格的两步派生
        let mut hasher = Sha256::new();
        hasher.update(node_key);
        hasher.update(b"cis-memory-encryption");
        let key_material = hasher.finalize();
        // ...
    }
}
```

### 6.2 安全分析

| 检查项 | 状态 | 说明 |
|--------|------|------|
| 算法选择 | ✅ 通过 | ChaCha20-Poly1305，现代 AEAD 算法 |
| 密钥派生 | ⚠️ 警告 | 使用简单 SHA256，建议使用 HKDF |
| Nonce 生成 | ✅ 通过 | 使用 `OsRng` 生成随机 nonce |

### 6.3 发现的问题

**问题 #6: 密钥派生使用简单哈希**
- **位置**: `cis-core/src/memory/encryption.rs:24-36`
- **风险等级**: 低
- **描述**: 使用 `SHA256` 进行密钥派生，而非标准的 HKDF
- **建议**: 使用 `hkdf::Hkdf` 进行标准密钥派生

---

## 7. 改进建议列表

### 高优先级

1. **启用 WebSocket TLS 加密**
   - 将默认 WebSocket 协议从 `ws://` 改为 `wss://`
   - 提供 TLS 证书配置选项
   - 添加 `use_tls` 配置项，默认 `true`

2. **加密存储节点密钥**
   - 使用系统密钥链（如 macOS Keychain、Linux Secret Service）存储密钥
   - 或提供主密码加密配置文件的选项
   - 避免明文存储在 `config.toml`

### 中优先级

3. **使用密码学安全 RNG**
   - 将 `wizard.rs` 中的 `thread_rng()` 替换为 `OsRng`
   - 将 `did_verify.rs` 中的 `thread_rng()` 替换为 `OsRng`
   - **代码修改示例**:
   ```rust
   // 当前
   rand::thread_rng().fill_bytes(&mut key);
   
   // 建议
   use rand::rngs::OsRng;
   use rand::RngCore;
   OsRng.fill_bytes(&mut key);
   ```

4. **加密审计日志**
   - 提供审计日志加密选项
   - 或支持将审计日志发送到安全的远程服务器

5. **使用标准 HKDF 密钥派生**
   - 替换 `memory/encryption.rs` 中的简单 SHA256 派生
   - **代码修改示例**:
   ```rust
   use hkdf::Hkdf;
   use sha2::Sha256;
   
   let hkdf = Hkdf::<Sha256>::new(Some(b"cis-salt"), node_key);
   let mut key_material = [0u8; 32];
   hkdf.expand(b"cis-memory-encryption", &mut key_material)
       .expect("HKDF expansion failed");
   ```

### 低优先级

6. **添加配置验证**
   - 启动时检查安全配置
   - 如果检测到不安全的配置（如明文传输），发出警告

7. **安全头部**
   - WebSocket 握手时添加安全相关的 HTTP 头部
   - 如 `X-Content-Type-Options`, `X-Frame-Options` 等

---

## 8. 结论

CIS 项目在网络安全架构方面具有较好的基础设计，特别是在**默认网络模式**和**ACL 配置**方面采用了安全优先的设计原则。但在**密钥管理**和**传输加密**方面存在一些需要改进的问题。

### 总体安全评分: 7/10

| 方面 | 评分 | 说明 |
|------|------|------|
| 架构设计 | 8/10 | 默认安全模式，良好的 ACL 设计 |
| 实现细节 | 6/10 | 部分使用非加密安全 RNG |
| 传输安全 | 5/10 | 默认不使用 TLS |
| 密钥管理 | 6/10 | 明文存储密钥 |
| 审计日志 | 9/10 | 完整的审计记录 |

### 关键行动项

1. **立即处理**: 为 WebSocket 添加 TLS 支持
2. **短期内处理**: 修复 RNG 使用问题，加密存储密钥
3. **长期改进**: 完善密钥派生机制，加密审计日志

---

## 附录: 代码引用

### 安全相关代码位置

| 功能 | 文件 | 行号 |
|------|------|------|
| 默认网络模式 | `cis-core/src/network/acl.rs` | 14-29 |
| ACL 默认配置 | `cis-core/src/network/acl.rs` | 147-158 |
| WebSocket 配置 | `cis-core/src/network/websocket.rs` | 28-38 |
| 审计日志配置 | `cis-core/src/network/audit.rs` | 190-200 |
| 节点密钥生成 | `cis-core/src/init/wizard.rs` | 431-438 |
| DID 密钥生成 | `cis-core/src/identity/did.rs` | 32-40 |
| Nonce 生成 | `cis-core/src/network/did_verify.rs` | 318-324 |
| 密钥文件权限 | `cis-core/src/identity/did.rs` | 130-137 |
| 记忆加密 | `cis-core/src/memory/encryption.rs` | 1-89 |
| DID Challenge 超时 | `cis-core/src/network/did_verify.rs` | 42 |
| Session 超时 | `cis-core/src/network/session_manager.rs` | 22 |

---

*报告生成时间: 2026-02-08 18:45:00+08:00*
