# SEC-3 替代方案: 使用 SSH Key 进行密钥加密

> 决策 4 的替代方案探讨
> 日期: 2026-02-10

---

## 原始方案回顾

**SEC-3 原方案**:
- 用户密码 + Argon2id + ChaCha20-Poly1305
- `cis init` 时强制输入密码
- 每次启动需要解锁

**问题**:
- 用户体验差（频繁输入密码）
- 密码遗忘风险
- 跨设备同步困难

---

## SSH Key 替代方案

### 核心思路

使用现有的 SSH 密钥对（或生成新的 CIS 专用密钥）来保护节点私钥。

```
┌─────────────────────────────────────────────────────────────┐
│                  SSH Key 保护方案                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 密钥生成/复用                                            │
│     - 复用现有 SSH key: ~/.ssh/id_ed25519                   │
│     - 或生成 CIS 专用 key: ~/.cis/cis_ed25519               │
│                                                             │
│  2. 私钥加密存储                                               │
│     - 节点私钥（Ed25519）                                    │
│     - 使用 SSH key 派生的对称密钥加密                          │
│     - 存储: ~/.cis/node.key.encrypted                        │
│                                                             │
│  3. 解密流程                                                   │
│     - 启动时读取 SSH key                                     │
│     - 如果 SSH key 有密码，使用 ssh-agent 或输入一次           │
│     - 派生密钥 → 解密节点私钥 → 内存中使用                     │
│                                                             │
│  4. 优势                                                      │
│     - 复用现有 SSH 基础设施                                  │
│     - 支持 ssh-agent（无需重复输入密码）                       │
│     - 支持硬件密钥（YubiKey, etc）                           │
│     - 跨设备同步通过 SSH config                              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 技术实现

### 密钥派生

```rust
// cis-core/src/identity/ssh_key_protection.rs

use ssh_key::{PrivateKey, HashAlg};
use sha2::{Sha256, Digest};
use aes_gcm::{Aes256Gcm, Key, Nonce};

pub struct SshKeyProtection;

impl SshKeyProtection {
    /// 从 SSH 私钥派生加密密钥
    pub fn derive_key(ssh_private_key: &PrivateKey) -> Result<[u8; 32]> {
        // 使用 SSH key 的 secret 部分作为种子
        let seed = ssh_private_key.key_data().as_ed25519()
            .ok_or_else(|| Error::unsupported_key_type())?
            .private
            .as_ref();
        
        // 使用 HKDF 派生 256-bit 密钥
        let mut okm = [0u8; 32];
        Hkdf::<Sha256>::extract(None, seed)
            .expand(b"cis-node-key-v1", &mut okm)
            .map_err(|e| Error::key_derivation_failed(e))?;
        
        Ok(okm)
    }
    
    /// 加密节点私钥
    pub fn encrypt_node_key(
        node_private_key: &[u8; 32],
        ssh_private_key: &PrivateKey,
    ) -> Result<Vec<u8>> {
        let key = Self::derive_key(ssh_private_key)?;
        let cipher = Aes256Gcm::new(Key::from_slice(&key));
        
        // 随机 nonce
        let nonce = Self::generate_nonce();
        
        // 加密
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), node_private_key.as_ref())
            .map_err(|e| Error::encryption_failed(e))?;
        
        // 存储: nonce (12 bytes) + ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
    
    /// 解密节点私钥
    pub fn decrypt_node_key(
        encrypted: &[u8],
        ssh_private_key: &PrivateKey,
    ) -> Result<[u8; 32]> {
        if encrypted.len() < 12 + 32 {
            return Err(Error::invalid_encrypted_data());
        }
        
        let key = Self::derive_key(ssh_private_key)?;
        let cipher = Aes256Gcm::new(Key::from_slice(&key));
        
        let nonce = Nonce::from_slice(&encrypted[..12]);
        let plaintext = cipher
            .decrypt(nonce, &encrypted[12..])
            .map_err(|e| Error::decryption_failed(e))?;
        
        plaintext.try_into()
            .map_err(|_| Error::invalid_key_length())
    }
}
```

### 与 ssh-agent 集成

```rust
/// 使用 ssh-agent 获取 SSH key（无需密码输入）
pub async fn get_key_from_agent() -> Result<PrivateKey> {
    let mut client = ssh_agent_client::Client::connect(
        std::env::var("SSH_AUTH_SOCK")?
    ).await?;
    
    let identities = client.request_identities().await?;
    
    // 查找 CIS 专用的 key comment
    for identity in identities {
        if identity.comment == "cis-node-key" {
            // 使用此 key 进行解密操作
            // 注意：ssh-agent 不直接提供私钥，需要特殊处理
        }
    }
    
    // 备选：直接使用文件
    Self::load_key_from_file()
}

/// 加载 SSH key 从文件
pub fn load_key_from_file() -> Result<PrivateKey> {
    let paths = [
        "~/.cis/cis_ed25519",
        "~/.ssh/id_ed25519",
    ];
    
    for path in &paths {
        let expanded = shellexpand::tilde(path);
        if std::path::Path::new(expanded.as_ref()).exists() {
            let pem = std::fs::read_to_string(expanded.as_ref())?;
            return PrivateKey::from_openssh(&pem)
                .map_err(|e| Error::invalid_ssh_key(e));
        }
    }
    
    Err(Error::ssh_key_not_found())
}
```

### 初始化流程

```bash
# 方案 A: 复用现有 SSH key
$ cis init
✓ 发现 SSH key: ~/.ssh/id_ed25519
? 是否使用此 key 保护 CIS 节点私钥? (Y/n): y
✓ 节点私钥已加密存储

# 方案 B: 生成 CIS 专用 key
$ cis init --generate-ssh-key
✓ 生成新的 SSH key: ~/.cis/cis_ed25519
? 是否设置密码? (y/N): n  # 可以是空密码，依赖文件权限
✓ 节点私钥已加密存储

# 启动（无密码输入，如果 SSH key 无密码）
$ cis node start
✓ 自动解锁节点私钥

# 或使用 ssh-agent（SSH key 有密码时）
$ eval $(ssh-agent)
$ ssh-add ~/.cis/cis_ed25519
Enter passphrase: ****
$ cis node start
✓ 通过 ssh-agent 解锁
```

---

## 方案对比

| 维度 | 原方案 (密码) | SSH Key 方案 |
|------|--------------|-------------|
| **用户体验** | 频繁输入密码 | 一次设置，自动解锁 |
| **安全性** | 依赖密码强度 | 复用 SSH 安全基础设施 |
| **硬件支持** | 不支持 | 支持 YubiKey/HSM |
| **跨设备** | 困难 | 通过 SSH config 同步 |
| **备份** | 助记词 | SSH key + 助记词 |
| **复杂度** | 简单 | 中等（需要 SSH 知识）|
| **依赖** | 无 | 需要 SSH key |

---

## 推荐决策

### 采用 SSH Key 方案（修改 SEC-3）

**实施计划**:

**Week 1**: 基础实现
- 实现 `SshKeyProtection` 模块
- 实现密钥派生和加解密
- 创建 `cis init --ssh-key` 流程

**Week 2**: ssh-agent 集成
- 实现 agent 通信
- 自动解锁逻辑
- 错误处理和回退

**Week 3**: 测试和文档
- 单元测试
- 集成测试
- 用户文档

### 与助记词的关系

```
用户
├── SSH Key (用于保护节点私钥)
│   └── 存储在 ~/.cis/cis_ed25519
│   └── 可以设置密码或空密码
│   └── 可以通过 ssh-agent 管理
│
└── 助记词 (用于恢复所有权)
    └── 24 个单词
    └── 只在初始化时显示
    └── 硬件损坏时用于恢复
```

**关键区别**:
- **SSH Key**: 日常操作的"钥匙"，可以更换
- **助记词**: 恢复所有权的"根凭证"，永不更换

---

## 迁移路径

**现有用户**:
```bash
# 升级时自动迁移
$ cis upgrade
✓ 发现未加密的节点私钥
? 是否使用 SSH key 保护? (Y/n): y
✓ 选择 SSH key:
  1. ~/.ssh/id_ed25519 (已存在)
  2. 生成新的 CIS 专用 key
? 选择: 1
✓ 节点私钥已加密
✓ 原始私钥已安全擦除
```

---

## 验收标准

- [ ] 支持复用现有 SSH key
- [ ] 支持生成 CIS 专用 SSH key
- [ ] 支持 ssh-agent 自动解锁
- [ ] 支持 YubiKey 等硬件密钥
- [ ] 加密后的私钥文件权限 0600
- [ ] 清晰的错误提示（SSH key 不存在、密码错误等）
- [ ] 完整的文档和示例

---

*方案创建时间: 2026-02-10*  
*建议采用: 是（优于原密码方案）*
