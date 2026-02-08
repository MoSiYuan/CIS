# CIS 加密实现安全审查报告

**审查日期**: 2026-02-07  
**审查范围**: CIS Core v1.1.0 加密实现  
**审查人员**: AI Security Auditor  
**风险评级**: 🔴 高危 (发现关键安全问题)

---

## 执行摘要

本次安全审查对 CIS 项目的加密实现进行了全面分析。发现了 **3 个高危问题**、**5 个中危问题**和 **4 个低危问题**。主要问题集中在密钥派生、随机数生成和 TLS 证书验证方面。

### 风险总览

| 风险等级 | 数量 | 状态 |
|---------|------|------|
| 🔴 高危 | 3 | 需立即修复 |
| 🟡 中危 | 5 | 建议尽快修复 |
| 🟢 低危 | 4 | 建议改进 |

---

## 1. ChaCha20-Poly1305 实现审查

### 1.1 文件位置
- `cis-core/src/memory/encryption.rs`

### 1.2 实现分析

```rust
// 当前实现 (第 24-36 行)
pub fn from_node_key(node_key: &[u8]) -> Self {
    // 步骤 1: 使用 SHA256 派生密钥材料
    let mut hasher = Sha256::new();
    hasher.update(node_key);
    hasher.update(b"cis-memory-encryption");
    let key_material = hasher.finalize();

    // 步骤 2: 使用派生出的密钥创建 ChaCha20Poly1305
    let key = chacha20poly1305::Key::from_slice(&key_material);
    let cipher = ChaCha20Poly1305::new(key);

    Self { cipher }
}
```

### 1.3 发现的问题

#### 🔴 高危-1: 弱密钥派生 (SHA256 代替 Argon2id)

**问题描述**:
- 文档声明使用 Argon2id 进行密钥派生
- 实际代码使用简单的 SHA256 哈希
- 没有使用盐值 (salt)
- 没有内存困难性 (memory-hard) 保护

**安全影响**:
- 容易受到暴力破解攻击
- 无法抵抗硬件加速攻击 (ASIC/FPGA)
- 相同密码总是生成相同密钥

**代码位置**:
```rust
// cis-core/src/memory/encryption.rs:24-36
pub fn from_node_key(node_key: &[u8]) -> Self {
    let mut hasher = Sha256::new();
    hasher.update(node_key);
    hasher.update(b"cis-memory-encryption");  // 固定上下文，无 salt
    let key_material = hasher.finalize();
    // ...
}
```

**修复建议**:
```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand::rngs::OsRng;

pub fn from_node_key(node_key: &[u8], salt: &[u8]) -> Self {
    // 使用 Argon2id 进行密钥派生
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(
            65536,  // 64MB memory cost
            3,      // 3 iterations
            4,      // 4 parallelism
            Some(32) // 32-byte output
        ).expect("Valid Argon2 params")
    );
    
    let mut key_material = [0u8; 32];
    argon2.hash_password_into(node_key, salt, &mut key_material)
        .expect("Argon2 hashing failed");
    
    let key = chacha20poly1305::Key::from_slice(&key_material);
    let cipher = ChaCha20Poly1305::new(key);
    
    // 清除敏感数据
    key_material.zeroize();
    
    Self { cipher }
}
```

#### 🟡 中危-1: 缺少盐值存储机制

**问题描述**:
- 加密数据格式: `nonce(12字节) || ciphertext || tag(16字节)`
- 缺少 salt 存储位置

**修复建议**:
- 修改数据格式为: `salt(16字节) || nonce(12字节) || ciphertext || tag(16字节)`
- 或单独存储 salt

---

## 2. 随机数生成审查

### 2.1 安全实现 ✅

以下位置正确使用 `OsRng`:

| 文件 | 用途 | 状态 |
|------|------|------|
| `cis-core/src/identity/did.rs:33` | Ed25519 密钥生成 | ✅ |
| `cis-core/src/memory/encryption.rs:50` | ChaCha20-Poly1305 nonce | ✅ |
| `cis-core/src/matrix/websocket/noise.rs:250` | X25519 密钥生成 | ✅ |

### 2.2 不安全的随机数生成 🔴

#### 🔴 高危-2: 使用 `thread_rng()` 生成加密密钥

**问题描述**:
- `thread_rng()` 不是密码学安全随机数生成器 (CSPRNG)
- 使用 ChaCha12 算法，周期较短
- 不适合生成长期密钥或 nonce

**受影响的代码**:

```rust
// cis-core/src/network/did_verify.rs:319-323
fn generate_nonce() -> [u8; NONCE_LENGTH] {
    use rand::RngCore;
    let mut nonce = [0u8; NONCE_LENGTH];
    rand::thread_rng().fill_bytes(&mut nonce);  // ❌ 不安全
    nonce
}
```

```rust
// cis-core/src/init/wizard.rs:435
rand::thread_rng().fill_bytes(&mut key);  // ❌ 不安全
```

```rust
// cis-core/src/matrix/routes/login.rs:168
rand::thread_rng().fill_bytes(&mut bytes);  // ❌ 不安全
```

```rust
// cis-core/src/matrix/routes/register.rs:204
let mut rng = rand::thread_rng();  // ❌ 不安全
```

```rust
// cis-core/src/matrix/store_social.rs:596
let mut rng = rand::thread_rng();  // ❌ 不安全
```

**安全影响**:
- 可预测的 nonce 可能导致密钥恢复攻击
- 可预测的密钥完全破坏加密安全性

**修复建议**:
```rust
use rand::rngs::OsRng;
use rand::RngCore;

fn generate_nonce() -> [u8; NONCE_LENGTH] {
    let mut nonce = [0u8; NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce);  // ✅ 使用 OsRng
    nonce
}
```

---

## 3. DID 签名验证审查

### 3.1 文件位置
- `cis-core/src/identity/did.rs`
- `cis-core/src/network/did_verify.rs`
- `cis-core/src/network/websocket_auth.rs`

### 3.2 实现分析

#### ✅ 正确实现

**Ed25519 签名验证** (`did.rs:176-178`):
```rust
pub fn verify(verifying_key: &VerifyingKey, data: &[u8], signature: &Signature) -> bool {
    verifying_key.verify(data, signature).is_ok()
}
```
- 使用 `ed25519_dalek`  crate，符合 RFC 8032
- 自动处理签名验证

**Challenge-Response 协议** (`did_verify.rs`):
- 使用 32 字节 nonce
- 30 秒超时窗口
- 签名整个 challenge 结构

#### 🟡 中危-2: Challenge-Response 重放攻击防护不足

**问题描述**:
- `PendingChallenges` 仅基于 nonce 存储
- 没有检查 challenge 是否已被使用
- 理论上存在短时间内重放的可能

**代码位置**:
```rust
// cis-core/src/network/did_verify.rs:345-360
pub fn take(&mut self, nonce: &str) -> Option<DidChallenge> {
    self.challenges.remove(nonce)  // 移除后无法检测重放
}
```

**修复建议**:
```rust
pub struct PendingChallenges {
    challenges: HashMap<String, DidChallenge>,
    used_nonces: HashSet<String>, // 添加已使用 nonce 记录
}

pub fn take(&mut self, nonce: &str) -> Option<DidChallenge> {
    // 检查 nonce 是否已被使用
    if self.used_nonces.contains(nonce) {
        return None; // 检测到重放攻击
    }
    
    let challenge = self.challenges.remove(nonce)?;
    self.used_nonces.insert(nonce.to_string());
    Some(challenge)
}
```

#### 🟡 中危-3: DID 公钥解析不完整

**问题描述**:
- DID 格式只存储公钥前 8 字节 (`pub_key_short`)
- 完整公钥需要从外部解析
- 解析逻辑存在潜在的安全问题

**代码位置**:
```rust
// cis-core/src/network/did_verify.rs:303-316
fn resolve_did_to_full_key(did: &str) -> Option<[u8; 32]> {
    let (_, public_key_hex) = DIDManager::parse_did(did)?;
    // 问题: 这里的 public_key_hex 实际上是 short 版本
    let bytes = hex::decode(&public_key_hex).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    bytes.try_into().ok()
}
```

**注意**: DID 格式 `did:cis:{node_id}:{pub_key_short}` 中的 `pub_key_short` 只有 16 字符 (8 字节)，无法恢复完整 32 字节公钥。

---

## 4. TLS/Noise 协议审查

### 4.1 Noise Protocol 实现

**文件**: `cis-core/src/matrix/websocket/noise.rs`

#### ✅ 正确实现

- 使用 `Noise_XX_25519_ChaChaPoly_BLAKE2s` 模式
- 使用 `OsRng` 生成 X25519 密钥
- 正确的 XX 握手流程实现

### 4.2 TLS 配置问题

#### 🔴 高危-3: QUIC 传输层使用自签名证书

**问题描述**:
- 使用 `rcgen` 动态生成自签名证书
- 没有证书验证机制
- 容易受到中间人攻击

**代码位置**:
```rust
// cis-core/src/p2p/transport.rs:96-114
fn configure_server() -> Result<ServerConfig> {
    let cert = rcgen::generate_simple_self_signed(vec!["cis".into()])
        .map_err(|e| CisError::p2p(format!("Failed to generate certificate: {}", e)))?;
    // ...
}
```

**安全影响**:
- 攻击者可以轻易伪造证书
- 无法验证对端身份
- 完全破坏 TLS 的安全保证

**修复建议**:
1. 使用固定证书或证书 pinning
2. 实现证书链验证
3. 支持 mTLS 双向认证

```rust
fn configure_server() -> Result<ServerConfig> {
    // 从安全存储加载证书，不要动态生成
    let cert_path = std::env::var("CIS_TLS_CERT")
        .map_err(|_| CisError::p2p("TLS certificate path not set".into()))?;
    let key_path = std::env::var("CIS_TLS_KEY")
        .map_err(|_| CisError::p2p("TLS key path not set".into()))?;
    
    let cert = std::fs::read(&cert_path)?;
    let key = std::fs::read(&key_path)?;
    
    let cert_chain = vec![rustls::Certificate(cert)];
    let key = rustls::PrivateKey(key);
    
    ServerConfig::with_single_cert(cert_chain, key)
        .map_err(|e| CisError::p2p(format!("Failed to create server config: {}", e)))
}
```

#### 🟡 中危-4: WebSocket 默认使用未加密连接

**问题描述**:
- WebSocket 服务器默认监听 `ws://` (未加密)
- 生产环境应强制使用 `wss://`

**代码位置**:
```rust
// cis-core/src/matrix/websocket/server.rs:145-148
info!(
    "WebSocket federation server starting on ws://{}{}",
    addr, WS_PATH  // ❌ 默认 ws://
);
```

**修复建议**:
- 默认启用 TLS
- 添加配置选项强制加密
- 生产环境拒绝未加密连接

#### 🟡 中危-5: rustls 版本过旧

**问题描述**:
- 使用 rustls 0.21 (发布于 2023)
- 当前最新版本为 0.23
- 可能存在已知安全漏洞

**代码位置**:
```toml
# cis-core/Cargo.toml:58
rustls = "0.21"
```

**修复建议**:
```toml
rustls = "0.23"
```

---

## 5. 密钥管理审查

### 5.1 文件权限

#### ✅ 正确实现

**代码位置**: `cis-core/src/identity/did.rs:131-137`

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path.with_extension("key"))?.permissions();
    perms.set_mode(0o600);  // ✅ 仅所有者可读写
    fs::set_permissions(path.with_extension("key"), perms)?;
}
```

### 5.2 🟢 低危-1: 缺少密钥零化

**问题描述**:
- 密钥材料在使用后未清零
- 可能在内存中残留

**受影响的代码**:
- `cis-core/src/memory/encryption.rs` - 派生密钥未清零
- `cis-core/src/identity/did.rs` - 种子材料未清零

**修复建议**:
```rust
use zeroize::Zeroize;

pub fn from_node_key(node_key: &[u8]) -> Self {
    let mut key_material = derive_key(node_key);
    let key = chacha20poly1305::Key::from_slice(&key_material);
    let cipher = ChaCha20Poly1305::new(key);
    key_material.zeroize();  // ✅ 清除敏感数据
    Self { cipher }
}
```

---

## 6. 其他安全问题

### 6.1 🟢 低危-2: 时间侧信道攻击

**问题描述**:
- 解密失败返回通用错误消息
- 但时序可能泄露信息

**代码位置**:
```rust
// cis-core/src/memory/encryption.rs:79-81
.map_err(|_| CisError::memory("decryption failed (invalid key or corrupted data)"))
```

**修复建议**:
- 添加随机延迟使时序恒定
- 或使用 constant-time 比较

### 6.2 🟢 低危-3: 文档与实际实现不符

**问题描述**:
- README 声明使用 Argon2id
- 实际使用 SHA256
- 可能导致用户误判安全级别

### 6.3 🟢 低危-4: 缺少加密功能测试

**问题描述**:
- 未发现专门的加密测试套件
- 缺少 fuzz 测试
- 缺少侧信道测试

---

## 7. 合规性检查

### 7.1 加密算法合规性

| 算法 | 用途 | 合规性 | 备注 |
|------|------|--------|------|
| Ed25519 | 签名 | ✅ 合规 | RFC 8032 |
| ChaCha20-Poly1305 | AEAD 加密 | ✅ 合规 | RFC 8439 |
| X25519 | 密钥交换 | ✅ 合规 | RFC 7748 |
| SHA256 | 密钥派生 | ⚠️ 不推荐 | 应使用 Argon2id |
| Argon2id | 密钥派生 | ❌ 未实现 | 文档声明但未实现 |

### 7.2 最佳实践合规性

| 实践 | 状态 | 备注 |
|------|------|------|
| 使用 OsRng | ⚠️ 部分 | 部分代码使用 thread_rng |
| 密钥零化 | ❌ 否 | 未实现 |
| 恒定时间操作 | ❌ 否 | 未验证 |
| 证书验证 | ❌ 否 | QUIC 使用自签名证书 |
| mTLS 支持 | ⚠️ 部分 | 有代码但未强制启用 |

---

## 8. 修复优先级建议

### 8.1 立即修复 (🔴 高危)

1. **将密钥派生从 SHA256 改为 Argon2id**
   - 影响: 所有加密数据
   - 工作量: 中等
   - 向后兼容: 需要迁移策略

2. **将所有 `thread_rng()` 替换为 `OsRng`**
   - 影响: 所有随机数生成
   - 工作量: 小
   - 向后兼容: 无影响

3. **为 QUIC 实现正确的证书管理**
   - 影响: P2P 传输安全
   - 工作量: 中等
   - 向后兼容: 需要配置更新

### 8.2 尽快修复 (🟡 中危)

4. **增强 Challenge-Response 重放保护**
5. **修复 DID 公钥解析逻辑**
6. **默认启用 WebSocket TLS**
7. **升级 rustls 到 0.23**

### 8.3 建议改进 (🟢 低危)

8. 实现密钥零化
9. 添加时序攻击防护
10. 更新文档
11. 添加加密测试套件

---

## 9. 附录

### 9.1 参考标准

- [RFC 8032] Edwards-Curve Digital Signature Algorithm (EdDSA)
- [RFC 8439] ChaCha20 and Poly1305 for IETF Protocols
- [RFC 7748] Elliptic Curves for Security
- [OWASP Cryptographic Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html)
- [Argon2 RFC 9106](https://datatracker.ietf.org/doc/html/rfc9106)

### 9.2 工具建议

- **静态分析**: `cargo audit`, `cargo geiger`
- **Fuzz 测试**: `cargo fuzz`
- **侧信道测试**: `dudect`, `microwalk`

### 9.3 修订历史

| 版本 | 日期 | 说明 |
|------|------|------|
| 1.0 | 2026-02-07 | 初始版本 |

---

## 10. 总结

CIS 项目的加密实现存在若干关键安全问题，主要集中在:

1. **密钥派生不符合文档声明** - 实际使用 SHA256 而非 Argon2id
2. **不安全的随机数生成** - 多处使用 `thread_rng()`
3. **TLS 证书验证缺失** - QUIC 使用动态自签名证书

建议立即采取行动修复这些问题，特别是在生产环境部署之前。

---

*报告结束*
