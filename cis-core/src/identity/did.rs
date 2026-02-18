//! DID 身份管理
//! 
//! DID 格式: did:cis:{node_id}:{pub_key_short}

use ed25519_dalek::{SigningKey, Signer, Verifier, Signature, VerifyingKey};
use rand::rngs::OsRng;
use std::path::Path;
use std::fs;
use crate::error::{CisError, Result};
use crate::matrix::error::{MatrixError, MatrixResult};

/// 设置密钥文件权限（Unix + Windows）
///
/// # Security (P0-2)
///
/// - **Unix**: 设置权限为 0o600 (仅所有者可读写)
/// - **Windows**: 使用 icacls 禁用继承并限制访问
/// - **验证**: 权限设置后进行验证，确保生效
fn set_key_permissions(key_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(key_path)
            .map_err(|e| CisError::identity(format!("Failed to read key metadata: {}", e)))?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(key_path, perms)
            .map_err(|e| CisError::identity(format!("Failed to set key permissions: {}", e)))?;

        // 验证权限设置成功
        let verified_perms = fs::metadata(key_path)
            .map_err(|e| CisError::identity(format!("Failed to verify key permissions: {}", e)))?
            .permissions();
        let mode = verified_perms.mode();
        if mode & 0o777 != 0o600 {
            return Err(CisError::identity(
                format!("Key permission verification failed: got {:03o}, expected 0600", mode)
            ));
        }
    }

    #[cfg(windows)]
    {
        use std::process::Command;
        use whoami;

        let key_path_str = key_path.to_str()
            .ok_or_else(|| CisError::identity("Invalid key path".to_string()))?;

        // P0-2: Windows 权限安全修复
        // 1. 设置为完全控制（F）而不是仅读（R）
        // 2. 移除继承权限
        // 3. 验证权限设置成功
        let username = whoami::username();

        // 尝试设置权限
        let set_output = Command::new("icacls")
            .args(&[
                key_path_str,
                "/inheritance:r",  // 移除继承
                "/grant",
                &format!("{}:(F)", username),  // 完全控制权限 (P0-2 修复)
            ])
            .output();

        match &set_output {
            Ok(output) if output.status.success() => {
                tracing::debug!("Windows key permissions set to Full Control");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Failed to set Windows key permissions: {}", stderr);
                return Err(CisError::identity(format!(
                    "Failed to set Windows key permissions: {}", stderr
                )));
            }
            Err(e) => {
                tracing::warn!("Failed to execute icacls: {}", e);
                return Err(CisError::identity(format!(
                    "Failed to execute icacls: {}", e
                )));
            }
        }

        // P0-2: 添加权限验证（Windows）
        let verify_output = Command::new("icacls")
            .args(&[key_path_str])
            .output();

        match verify_output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // 验证当前用户有完全控制权限 (F)
                if !stdout.contains("(F)") && !stdout.contains("(F).") {
                    return Err(CisError::identity(format!(
                        "Windows key permission verification failed: expected Full Control (F), got: {}",
                        stdout.trim()
                    )));
                }
                tracing::debug!("Windows key permissions verified successfully");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("Failed to verify Windows key permissions: {}", stderr);
                // 验证失败时返回错误
                return Err(CisError::identity(format!(
                    "Failed to verify Windows key permissions: {}", stderr
                )));
            }
            Err(e) => {
                tracing::warn!("Failed to verify Windows key permissions: {}", e);
                return Err(CisError::identity(format!(
                    "Failed to verify Windows key permissions: {}", e
                )));
            }
        }
    }

    Ok(())
}

/// DID 管理器
#[derive(Clone)]
pub struct DIDManager {
    signing_key: SigningKey,
    node_id: String,
    did: String,
}

impl std::fmt::Debug for DIDManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DIDManager")
            .field("node_id", &self.node_id)
            .field("did", &self.did)
            .field("public_key", &self.public_key_hex())
            .finish()
    }
}

impl DIDManager {
    /// 生成新 DID
    pub fn generate(node_id: impl Into<String>) -> Result<Self> {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let node_id = node_id.into();
        let pub_key_short = hex::encode(&signing_key.verifying_key().to_bytes()[..8]);
        let did = format!("did:cis:{}:{}", node_id, pub_key_short);
        
        Ok(Self { signing_key, node_id, did })
    }
    
    /// 从种子恢复（确定性密钥）
    ///
    /// # Security Warning (P0-3)
    ///
    /// **当前实现的限制**:
    /// - 当种子长度不足时，仅使用单次 SHA256 扩展
    /// - 缺少盐值 (salt) 和密钥派生函数 (KDF)
    ///
    /// **推荐做法**:
    /// - 使用 Argon2id 或 PBKDF2 进行密钥派生
    /// - 为每个派生操作生成随机盐值
    /// - 设置足够的迭代次数（推荐 > 100,000）
    ///
    /// **升级路径**:
    /// ```toml
    /// # Cargo.toml
    /// [dependencies]
    /// argon2 = "0.5"
    /// ```
    ///
    /// ```rust
    /// use argon2::{Argon2, PasswordHasher, password_hash::{SaltString, rand_core::OsRng}};
    ///
    /// let salt = SaltString::generate(&mut OsRng);
    /// let argon2 = Argon2::default();
    /// let hash = argon2.hash_password(
    ///     seed,
    ///     &salt
    /// ).unwrap();
    /// ```
    ///
    /// # 向后兼容性
    ///
    /// 修改此方法将破坏现有用户的密钥！建议：
    /// 1. 添加新方法 `from_seed_kdf()` 使用 Argon2
    /// 2. 保留 `from_seed()` 用于向后兼容
    /// 3. 在文档中标记 `from_seed()` 为 deprecated
    pub fn from_seed(seed: &[u8], node_id: impl Into<String>) -> Result<Self> {
        // 使用种子生成密钥对
        // 确保种子长度至少为 32 字节
        let seed_bytes: [u8; 32] = if seed.len() >= 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&seed[..32]);
            bytes
        } else {
            // P0-3 安全修复：使用 Argon2id 进行密钥派生
            // - Argon2id 是密码学安全的 KDF，抵抗 GPU/ASIC 攻击
            // - 使用固定的盐值（用于向后兼容）+ 种子作为密码
            // - 使用推荐的默认参数（m=19456, t=2, p=1）
            tracing::info!(
                "Seed length < 32 bytes, using Argon2id KDF for secure key derivation"
            );

            use argon2::{Argon2, PasswordHasher};
            use argon2::password_hash::Salt;

            // 使用固定的盐值以保持向后兼容（确定性行为）
            // 注意：为了更好的安全性，应该使用随机盐值并存储它
            let salt_bytes = [0u8; 16];  // 固定盐值（向后兼容）
            let salt = Salt::try_from(&salt_bytes[..])
                .map_err(|e| CisError::identity(format!("Invalid salt: {}", e)))?;

            // 使用 Argon2id (Algorithm::Argon2id)
            let argon2 = Argon2::default();
            let mut output = [0u8; 32];

            argon2.hash_password_into(
                seed,  // 种子作为密码
                &salt_bytes,  // 盐值
                &mut output
            ).map_err(|e| CisError::identity(format!("Argon2id KDF failed: {}", e)))?;

            output
        };

        let signing_key = SigningKey::from_bytes(&seed_bytes);
        
        let node_id = node_id.into();
        let pub_key_short = hex::encode(&signing_key.verifying_key().to_bytes()[..8]);
        let did = format!("did:cis:{}:{}", node_id, pub_key_short);
        
        Ok(Self { signing_key, node_id, did })
    }
    
    /// 从文件加载/保存
    pub fn load_or_generate(path: &Path, node_id: impl Into<String>) -> Result<Self> {
        let node_id = node_id.into();
        
        if path.exists() {
            // 从文件加载
            let data = fs::read_to_string(path)
                .map_err(|e| CisError::identity(format!("Failed to read DID file: {}", e)))?;
            
            let parts: Vec<&str> = data.trim().split(':').collect();
            if parts.len() != 4 || parts[0] != "did" || parts[1] != "cis" {
                return Err(CisError::identity("Invalid DID format in file"));
            }
            
            // 从文件的十六进制编码加载密钥
            let key_hex = fs::read_to_string(path.with_extension("key"))
                .map_err(|e| CisError::identity(format!("Failed to read key file: {}", e)))?;
            
            let key_bytes = hex::decode(key_hex.trim())
                .map_err(|e| CisError::identity(format!("Invalid key hex: {}", e)))?;
            
            if key_bytes.len() != 64 {
                return Err(CisError::identity("Invalid key length"));
            }
            
            let secret_bytes: [u8; 32] = key_bytes[..32].try_into()
                .map_err(|_| CisError::identity("Invalid secret key length"))?;
            let public_bytes: [u8; 32] = key_bytes[32..].try_into()
                .map_err(|_| CisError::identity("Invalid public key length"))?;
            
            let signing_key = SigningKey::from_bytes(&secret_bytes);
            let verifying_key = VerifyingKey::from_bytes(&public_bytes)
                .map_err(|e| CisError::identity(format!("Invalid public key: {:?}", e)))?;
            
            // 验证密钥对是否匹配
            if signing_key.verifying_key() != verifying_key {
                return Err(CisError::identity("Key pair mismatch"));
            }
            
            let did = format!("did:cis:{}:{}", parts[2], parts[3]);
            
            Ok(Self { signing_key, node_id: parts[2].to_string(), did })
        } else {
            // 生成新的 DID
            let manager = Self::generate(node_id)?;
            
            // 保存到文件
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| CisError::identity(format!("Failed to create directory: {}", e)))?;
            }
            
            // 保存 DID
            fs::write(path, &manager.did)
                .map_err(|e| CisError::identity(format!("Failed to write DID file: {}", e)))?;
            
            // 保存密钥（私钥 + 公钥）
            let mut key_bytes = Vec::with_capacity(64);
            key_bytes.extend_from_slice(&manager.signing_key.to_bytes());
            key_bytes.extend_from_slice(&manager.signing_key.verifying_key().to_bytes());
            let key_path = path.with_extension("key");
            fs::write(&key_path, hex::encode(&key_bytes))
                .map_err(|e| CisError::identity(format!("Failed to write key file: {}", e)))?;

            // 设置密钥文件权限
            set_key_permissions(&key_path)?;

            Ok(manager)
        }
    }
    
    /// 从现有的签名密钥创建
    pub fn from_signing_key(signing_key: SigningKey, node_id: impl Into<String>) -> Self {
        let node_id = node_id.into();
        let pub_key_short = hex::encode(&signing_key.verifying_key().to_bytes()[..8]);
        let did = format!("did:cis:{}:{}", node_id, pub_key_short);
        
        Self { signing_key, node_id, did }
    }
    
    /// 获取 DID
    pub fn did(&self) -> &str { &self.did }
    
    /// 获取 node_id
    pub fn node_id(&self) -> &str { &self.node_id }
    
    /// 获取验证公钥（返回拷贝，因为 VerifyingKey 实现了 Copy）
    pub fn verifying_key(&self) -> VerifyingKey { self.signing_key.verifying_key() }
    
    /// 获取签名密钥（谨慎使用）
    pub fn signing_key(&self) -> &SigningKey { &self.signing_key }
    
    /// 签名数据
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }
    
    /// 签名数据并返回十六进制字符串
    pub fn sign_to_hex(&self, data: &[u8]) -> String {
        let signature = self.sign(data);
        hex::encode(signature.to_bytes())
    }
    
    /// 验证签名（静态方法）
    pub fn verify(verifying_key: &VerifyingKey, data: &[u8], signature: &Signature) -> bool {
        verifying_key.verify(data, signature).is_ok()
    }
    
    /// 解析 DID
    /// did:cis:node:abc123 -> Some(("node", "abc123"))
    pub fn parse_did(did: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = did.split(':').collect();
        if parts.len() != 4 || parts[0] != "did" || parts[1] != "cis" {
            return None;
        }
        Some((parts[2].to_string(), parts[3].to_string()))
    }
    
    /// 验证 DID 格式是否有效
    pub fn is_valid_did(did: &str) -> bool {
        Self::parse_did(did).is_some()
    }
    
    /// 导出公钥为十六进制字符串
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }
    
    /// 从十六进制字符串导入公钥
    pub fn verifying_key_from_hex(hex_str: &str) -> MatrixResult<VerifyingKey> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| MatrixError::InvalidParameter(format!("Invalid hex: {}", e)))?;
        
        if bytes.len() != 32 {
            return Err(MatrixError::InvalidParameter("Invalid public key length".to_string()));
        }
        
        let key_bytes: [u8; 32] = bytes.try_into()
            .map_err(|_| MatrixError::InvalidParameter("Invalid public key length".to_string()))?;
        
        VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| MatrixError::InvalidParameter(format!("Invalid public key: {:?}", e)))
    }
    
    /// 从十六进制解析签名
    pub fn signature_from_hex(hex_str: &str) -> MatrixResult<Signature> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| MatrixError::InvalidParameter(format!("Invalid hex: {}", e)))?;
        
        if bytes.len() != 64 {
            return Err(MatrixError::InvalidParameter("Invalid signature length".to_string()));
        }
        
        let sig_bytes: [u8; 64] = bytes.try_into()
            .map_err(|_| MatrixError::InvalidParameter("Invalid signature length".to_string()))?;
        
        Ok(Signature::from_bytes(&sig_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_generate_did() {
        let manager = DIDManager::generate("test-node").unwrap();
        assert!(manager.did().starts_with("did:cis:test-node:"));
        assert_eq!(manager.node_id(), "test-node");
        assert!(!manager.public_key_hex().is_empty());
    }

    #[test]
    fn test_parse_did() {
        let did = "did:cis:my-node:abc12345";
        let parsed = DIDManager::parse_did(did);
        assert!(parsed.is_some());
        let (node, key_short) = parsed.unwrap();
        assert_eq!(node, "my-node");
        assert_eq!(key_short, "abc12345");
        
        // 无效的 DID
        assert!(DIDManager::parse_did("invalid").is_none());
        assert!(DIDManager::parse_did("did:other:node:key").is_none());
    }

    #[test]
    fn test_sign_and_verify() {
        let manager = DIDManager::generate("test-node").unwrap();
        let data = b"Hello, World!";
        
        let signature = manager.sign(data);
        assert!(DIDManager::verify(&manager.verifying_key(), data, &signature));
        
        // 验证不同的数据应该失败
        let wrong_data = b"Wrong data";
        assert!(!DIDManager::verify(&manager.verifying_key(), wrong_data, &signature));
    }

    #[test]
    fn test_from_seed() {
        let seed = b"my-test-seed-for-deterministic-keys";
        let manager1 = DIDManager::from_seed(seed, "node1").unwrap();
        let manager2 = DIDManager::from_seed(seed, "node1").unwrap();
        
        // 相同的种子应该生成相同的密钥对
        assert_eq!(manager1.did(), manager2.did());
        assert_eq!(manager1.public_key_hex(), manager2.public_key_hex());
    }

    #[test]
    fn test_load_or_generate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.did");
        
        // 第一次调用应该生成
        let manager1 = DIDManager::load_or_generate(&path, "test-node").unwrap();
        assert!(path.exists());
        
        // 第二次调用应该加载相同的 DID
        let manager2 = DIDManager::load_or_generate(&path, "test-node").unwrap();
        assert_eq!(manager1.did(), manager2.did());
    }

    #[test]
    fn test_hex_signature() {
        let manager = DIDManager::generate("test-node").unwrap();
        let data = b"Test data";
        
        let sig_hex = manager.sign_to_hex(data);
        let signature = DIDManager::signature_from_hex(&sig_hex).unwrap();
        
        assert!(DIDManager::verify(&manager.verifying_key(), data, &signature));
    }
}
