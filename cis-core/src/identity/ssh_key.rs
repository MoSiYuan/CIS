//! SSH Key 加密支持
//!
//! 使用 SSH Ed25519 密钥进行数据加密。
//!
//! ## 加密方案
//!
//! 1. 从 OpenSSH 私钥文件加载 Ed25519 密钥
//! 2. 将 Ed25519 密钥转换为 X25519 密钥（用于密钥交换）
//! 3. 使用 X25519 进行 ECDH 密钥交换，派生对称密钥
//! 4. 使用 ChaCha20-Poly1305 加密/解密数据

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Sha256, Digest};
use std::path::PathBuf;

/// 加密数据包
#[derive(Debug, Clone)]
pub struct EncryptedPacket {
    /// X25519 临时公钥
    pub ephemeral_pubkey: [u8; 32],
    /// ChaCha20-Poly1305 nonce
    pub nonce: [u8; 12],
    /// 密文 + 认证标签
    pub ciphertext: Vec<u8>,
}

/// SSH Key 加密器
pub struct SshKeyEncryption;

impl SshKeyEncryption {
    /// 从 OpenSSH 私钥文件加载 Ed25519 签名密钥
    ///
    /// # Arguments
    /// * `key_path` - 私钥文件路径，None 则使用默认路径 ~/.ssh/id_ed25519
    /// * `password` - 私钥密码，None 则无密码
    pub fn load_ed25519_key(
        key_path: Option<&str>,
        password: Option<&str>,
    ) -> crate::error::Result<SigningKey> {
        let path = key_path
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .expect("Cannot find home directory")
                    .join(".ssh/id_ed25519")
            });
        
        // 读取私钥文件
        let pem = std::fs::read_to_string(&path)
            .map_err(|e| crate::error::CisError::configuration(
                format!("Failed to read SSH key file: {}", e)
            ))?;
        
        // 解析 OpenSSH 私钥
        let private_key = ssh_key::PrivateKey::from_openssh(&pem)
            .map_err(|e| crate::error::CisError::configuration(
                format!("Failed to parse SSH key: {}", e)
            ))?;
        
        // 如有密码则解密
        let private_key = if let Some(pass) = password {
            private_key.decrypt(pass)
                .map_err(|e| crate::error::CisError::configuration(
                    format!("Failed to decrypt SSH key: {}", e)
                ))?
        } else {
            private_key
        };
        
        // 确保是 Ed25519 密钥
        use ssh_key::Algorithm;
        match private_key.algorithm() {
            Algorithm::Ed25519 => {
                let keypair = private_key.key_data().ed25519()
                    .ok_or_else(|| crate::error::CisError::configuration(
                        "Invalid Ed25519 key data".to_string()
                    ))?;
                
                // 提取私钥字节
                let secret_bytes: [u8; 32] = (*keypair.private.as_ref())
                    .into();
                
                Ok(SigningKey::from_bytes(&secret_bytes))
            }
            _ => Err(crate::error::CisError::configuration(
                "Only Ed25519 SSH keys are supported".to_string()
            )),
        }
    }

    /// 使用 SSH Ed25519 密钥加密数据
    ///
    /// 使用 X25519 密钥交换 + ChaCha20-Poly1305 加密
    pub fn encrypt(
        recipient_pubkey: &VerifyingKey,
        plaintext: &[u8],
    ) -> crate::error::Result<EncryptedPacket> {
        // 1. 生成临时的 X25519 密钥对
        let ephemeral_secret = x25519_dalek::EphemeralSecret::random_from_rng(&mut rand::thread_rng());
        let ephemeral_pubkey = x25519_dalek::PublicKey::from(&ephemeral_secret);
        
        // 2. 将 Ed25519 公钥转换为 X25519 公钥
        let recipient_x25519_pubkey = Self::ed25519_to_x25519_pubkey(recipient_pubkey)?;
        
        // 3. 执行 X25519 密钥交换
        let shared_secret = ephemeral_secret.diffie_hellman(&recipient_x25519_pubkey);
        
        // 4. 派生加密密钥
        let encryption_key = Self::derive_key(shared_secret.as_bytes(), b"cis-encryption-v1");
        
        // 5. 使用 ChaCha20-Poly1305 加密
        let cipher = ChaCha20Poly1305::new_from_slice(&encryption_key)
            .map_err(|_| crate::error::CisError::configuration(
                "Failed to create cipher".to_string()
            ))?;
        
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|_| crate::error::CisError::configuration(
                "Encryption failed".to_string()
            ))?;
        
        Ok(EncryptedPacket {
            ephemeral_pubkey: ephemeral_pubkey.to_bytes(),
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// 使用 SSH Ed25519 密钥解密数据
    pub fn decrypt(
        signing_key: &SigningKey,
        packet: &EncryptedPacket,
    ) -> crate::error::Result<Vec<u8>> {
        // 1. 将 Ed25519 私钥转换为 X25519 私钥
        let x25519_secret = Self::ed25519_to_x25519_secret(signing_key);
        
        // 2. 重构 X25519 临时公钥
        let ephemeral_pubkey = x25519_dalek::PublicKey::from(packet.ephemeral_pubkey);
        
        // 3. 执行密钥交换
        let shared_secret = x25519_secret.diffie_hellman(&ephemeral_pubkey);
        
        // 4. 派生相同的加密密钥
        let encryption_key = Self::derive_key(shared_secret.as_bytes(), b"cis-encryption-v1");
        
        // 5. 解密
        let cipher = ChaCha20Poly1305::new_from_slice(&encryption_key)
            .map_err(|_| crate::error::CisError::configuration(
                "Failed to create cipher".to_string()
            ))?;
        
        let nonce = Nonce::from_slice(&packet.nonce);
        
        let plaintext = cipher.decrypt(nonce, packet.ciphertext.as_ref())
            .map_err(|_| crate::error::CisError::configuration(
                "Decryption failed (wrong key or corrupted data)".to_string()
            ))?;
        
        Ok(plaintext)
    }

    /// Ed25519 公钥转换为 X25519 公钥
    fn ed25519_to_x25519_pubkey(ed_pubkey: &VerifyingKey) -> crate::error::Result<x25519_dalek::PublicKey> {
        use curve25519_dalek::edwards::CompressedEdwardsY;
        
        let compressed = CompressedEdwardsY::from_slice(ed_pubkey.as_bytes())
            .map_err(|_| crate::error::CisError::configuration(
                "Invalid Ed25519 public key compression".to_string()
            ))?;
        let edwards_point = compressed.decompress()
            .ok_or_else(|| crate::error::CisError::configuration(
                "Invalid Ed25519 public key point".to_string()
            ))?;
        
        let montgomery_point = edwards_point.to_montgomery();
        Ok(x25519_dalek::PublicKey::from(montgomery_point.to_bytes()))
    }

    /// Ed25519 私钥转换为 X25519 私钥
    fn ed25519_to_x25519_secret(signing_key: &SigningKey) -> x25519_dalek::StaticSecret {
        let bytes = signing_key.to_bytes();
        x25519_dalek::StaticSecret::from(bytes)
    }

    /// 使用 HKDF-SHA256 派生密钥
    fn derive_key(shared_secret: &[u8], context: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(shared_secret);
        hasher.update(context);
        hasher.finalize().into()
    }

    /// 派生节点加密密钥
    ///
    /// 使用 SSH 密钥派生固定密钥用于节点私钥加密
    pub fn derive_node_key(signing_key: &SigningKey) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(signing_key.to_bytes());
        hasher.update(b"cis-node-key-v1");
        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    #[test]
    fn test_encrypt_decrypt() {
        // 生成测试密钥对
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        
        // 加密
        let plaintext = b"Hello, SSH Key Encryption!";
        let packet = SshKeyEncryption::encrypt(&verifying_key, plaintext).unwrap();
        
        // 解密
        let decrypted = SshKeyEncryption::decrypt(&signing_key, &packet).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_derive_node_key() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let key1 = SshKeyEncryption::derive_node_key(&signing_key);
        let key2 = SshKeyEncryption::derive_node_key(&signing_key);
        
        // 相同密钥应派生相同结果
        assert_eq!(key1, key2);
        
        // 不同密钥应派生不同结果
        let signing_key2 = SigningKey::generate(&mut OsRng);
        let key3 = SshKeyEncryption::derive_node_key(&signing_key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_wrong_key_decryption_fails() {
        // 生成两个不同的密钥对
        let key1 = SigningKey::generate(&mut OsRng);
        let key2 = SigningKey::generate(&mut OsRng);
        
        // 使用 key1 加密
        let plaintext = b"Secret message";
        let packet = SshKeyEncryption::encrypt(&key1.verifying_key(), plaintext).unwrap();
        
        // 使用 key2 解密应该失败
        let result = SshKeyEncryption::decrypt(&key2, &packet);
        assert!(result.is_err());
    }
}
