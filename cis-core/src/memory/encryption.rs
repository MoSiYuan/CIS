//! 记忆加密模块
//!
//! 私域记忆使用 ChaCha20-Poly1305 进行认证加密，
//! 使用 HKDF 风格的两步派生进行密钥派生。

use crate::error::{CisError, Result};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use sha2::{Digest, Sha256};

/// 记忆加密器
pub struct MemoryEncryption {
    cipher: ChaCha20Poly1305,
}

impl MemoryEncryption {
    /// 从节点密钥创建加密器
    ///
    /// 使用 HKDF 风格的两步派生：
    /// 1. 先对密钥材料做 SHA256
    /// 2. 使用派生出的密钥创建 ChaCha20Poly1305
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

    /// 从原始密钥创建（用于测试）
    pub fn from_key(key: &[u8; 32]) -> Self {
        let key = chacha20poly1305::Key::from_slice(key);
        let cipher = ChaCha20Poly1305::new(key);
        Self { cipher }
    }

    /// 加密数据
    ///
    /// 返回格式: nonce(12字节) || ciphertext || tag(16字节)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // 生成随机 nonce
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

        // 加密（自动附加认证标签）
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| CisError::memory(format!("encryption failed: {}", e)))?;

        // 组合: nonce || ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// 解密数据
    ///
    /// 输入格式: nonce(12字节) || ciphertext || tag(16字节)
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 + 16 {
            return Err(CisError::memory("ciphertext too short"));
        }

        // 提取 nonce
        let nonce = Nonce::from_slice(&ciphertext[0..12]);
        let encrypted = &ciphertext[12..];

        // 解密（自动验证标签）
        self.cipher
            .decrypt(nonce, encrypted)
            .map_err(|_| CisError::memory("decryption failed (invalid key or corrupted data)"))
    }

    /// 重新加密数据（用于密钥轮换）
    pub fn re_encrypt(&self, old_ciphertext: &[u8], new_cipher: &Self) -> Result<Vec<u8>> {
        let plaintext = self.decrypt(old_ciphertext)?;
        new_cipher.encrypt(&plaintext)
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

    #[test]
    fn test_encrypt_decrypt_with_password() {
        let node_key = b"test_password_123";

        let encryption = MemoryEncryption::from_node_key(node_key);
        let plaintext = b"Hello, CIS Memory!";

        let ciphertext = encryption.encrypt(plaintext).unwrap();
        assert_ne!(ciphertext, plaintext.to_vec());
        assert!(ciphertext.len() >= plaintext.len() + 12 + 16); // nonce + tag

        let decrypted = encryption.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key() {
        let encryption1 = MemoryEncryption::from_node_key(b"correct_password");
        let encryption2 = MemoryEncryption::from_node_key(b"wrong_password");

        let plaintext = b"secret data";
        let ciphertext = encryption1.encrypt(plaintext).unwrap();

        // 用错误的密钥解密应该失败
        let result = encryption2.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_ciphertext_different_each_time() {
        // 相同的明文应该产生不同的密文（因为随机 nonce）
        let encryption = MemoryEncryption::from_node_key(b"test-key");
        let plaintext = b"same text";

        let ciphertext1 = encryption.encrypt(plaintext).unwrap();
        let ciphertext2 = encryption.encrypt(plaintext).unwrap();

        assert_ne!(ciphertext1, ciphertext2);

        // 但都能正确解密
        assert_eq!(encryption.decrypt(&ciphertext1).unwrap(), plaintext);
        assert_eq!(encryption.decrypt(&ciphertext2).unwrap(), plaintext);
    }

    #[test]
    fn test_authentication_tag() {
        let encryption = MemoryEncryption::from_node_key(b"test-key");
        let plaintext = b"authenticated data";

        let mut ciphertext = encryption.encrypt(plaintext).unwrap();

        // 篡改密文（修改最后一个字节）
        let last_idx = ciphertext.len() - 1;
        ciphertext[last_idx] ^= 0xFF;

        // 解密应该失败（认证标签验证失败）
        let result = encryption.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_re_encrypt() {
        let old_encryption = MemoryEncryption::from_node_key(b"old-key");
        let new_encryption = MemoryEncryption::from_node_key(b"new-key");

        let plaintext = b"data to be re-encrypted";
        let old_ciphertext = old_encryption.encrypt(plaintext).unwrap();

        // 重新加密
        let new_ciphertext = old_encryption
            .re_encrypt(&old_ciphertext, &new_encryption)
            .unwrap();

        // 新密文应该能用新密钥解密
        let decrypted = new_encryption.decrypt(&new_ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);

        // 旧密钥不应该能解密新密文
        assert!(old_encryption.decrypt(&new_ciphertext).is_err());
    }

    #[test]
    fn test_from_key() {
        let key = [0u8; 32];
        let encryption = MemoryEncryption::from_key(&key);

        let plaintext = b"test with raw key";
        let ciphertext = encryption.encrypt(plaintext).unwrap();
        let decrypted = encryption.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_ciphertext_too_short() {
        let encryption = MemoryEncryption::from_node_key(b"test-key");

        // 密文太短应该返回错误
        let result = encryption.decrypt(b"short");
        assert!(result.is_err());

        // 刚好 12 字节（只有 nonce）也应该失败
        let result = encryption.decrypt(&[0u8; 12]);
        assert!(result.is_err());

        // 27 字节（nonce + 15 字节）也应该失败（需要至少 28 字节）
        let result = encryption.decrypt(&[0u8; 27]);
        assert!(result.is_err());
    }
}
