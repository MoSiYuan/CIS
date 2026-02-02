//! DID 身份管理
//! 
//! DID 格式: did:cis:{node_id}:{pub_key_short}

use ed25519_dalek::{SigningKey, Signer, Verifier, Signature, VerifyingKey};
use rand::rngs::OsRng;
use std::path::Path;
use std::fs;
use crate::error::{CisError, Result};
use crate::matrix::error::{MatrixError, MatrixResult};

/// DID 管理器
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
    pub fn from_seed(seed: &[u8], node_id: impl Into<String>) -> Result<Self> {
        // 使用种子生成密钥对
        // 确保种子长度至少为 32 字节
        let seed_bytes: [u8; 32] = if seed.len() >= 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&seed[..32]);
            bytes
        } else {
            // 如果种子太短，使用 SHA256 哈希扩展
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(seed);
            hasher.finalize().into()
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
            fs::write(path.with_extension("key"), hex::encode(key_bytes))
                .map_err(|e| CisError::identity(format!("Failed to write key file: {}", e)))?;
            
            // 设置权限为仅所有者可读写 (0o600)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(path.with_extension("key"))?.permissions();
                perms.set_mode(0o600);
                fs::set_permissions(path.with_extension("key"), perms)?;
            }
            
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
    
    /// 签名十六进制字符串（方便传输）
    pub fn sign_to_hex(&self, data: &[u8]) -> String {
        hex::encode(self.sign(data).to_bytes())
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
        assert!(DIDManager::verify(manager.verifying_key(), data, &signature));
        
        // 验证不同的数据应该失败
        let wrong_data = b"Wrong data";
        assert!(!DIDManager::verify(manager.verifying_key(), wrong_data, &signature));
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
        
        assert!(DIDManager::verify(manager.verifying_key(), data, &signature));
    }
}
