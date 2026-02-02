//! 记忆加密模块
//!
//! 私域记忆使用 ChaCha20-Poly1305 加密

use crate::error::{CisError, Result};

/// 记忆加密器
pub struct MemoryEncryption {
    key: Vec<u8>,
}

impl MemoryEncryption {
    /// 从节点密钥创建加密器
    pub fn from_node_key(node_key: &[u8]) -> Self {
        // 使用 HKDF 派生加密密钥
        // 简化：直接使用前 32 字节作为密钥
        // TODO: 使用 HKDF 派生
        let mut key = vec![0u8; 32];
        for (i, byte) in node_key.iter().enumerate() {
            key[i % 32] ^= *byte;
        }
        key.extend_from_slice(b"cis-memory-encryption");
        let key = &key[..32];

        Self { key: key.to_vec() }
    }

    /// 加密数据
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // 简化实现：使用 XOR（实际应使用 ChaCha20-Poly1305）
        // TODO: 集成 chacha20poly1305 crate
        let ciphertext: Vec<u8> = plaintext
            .iter()
            .zip(self.key.iter().cycle())
            .map(|(p, k)| p ^ k)
            .collect();

        // 格式: [nonce (12 bytes)] [ciphertext]
        let mut result = vec![0u8; 12]; // nonce placeholder
        result.extend(ciphertext);

        Ok(result)
    }

    /// 解密数据
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(CisError::storage("Invalid ciphertext"));
        }

        // 简化实现
        let encrypted = &ciphertext[12..];
        let plaintext: Vec<u8> = encrypted
            .iter()
            .zip(self.key.iter().cycle())
            .map(|(c, k)| c ^ k)
            .collect();

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let enc = MemoryEncryption::from_node_key(b"test-key");
        let plaintext = b"hello, world!";

        let ciphertext = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }
}
