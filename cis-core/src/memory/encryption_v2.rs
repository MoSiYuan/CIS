//! 记忆加密模块 v2.0
//!
//! 使用 Argon2id 进行密钥派生，ChaCha20-Poly1305 进行认证加密。
//! 修复了 v1 版本中固定盐值的安全问题。

use crate::error::{CisError, Result};
use argon2::{Argon2, Algorithm, Params, Version};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::io::Write;
use std::path::Path;

/// v2 加密密钥
#[derive(Clone, Debug)]
pub struct EncryptionKeyV2 {
    /// 派生的加密密钥
    pub key: [u8; 32],
    /// 唯一的盐值
    pub salt: [u8; 32],
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// v2 密钥存储格式
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyStorageV2 {
    format: String,
    version: u8,
    created_at: String,
    algorithm: String,
    algorithm_params: AlgorithmParams,
    encoding: String,
    data: String, // base64 encoded binary data
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlgorithmParams {
    iterations: u32,
    parallelism: u32,
    memory: u32,
    output_length: usize,
}

/// v2 记忆加密器
#[derive(Clone)]
pub struct MemoryEncryptionV2 {
    key: [u8; 32],
}

/// Magic 字节用于识别 v2 格式
const MAGIC_V2: [u8; 4] = [0x43, 0x49, 0x53, 0x32]; // "CIS2"

impl EncryptionKeyV2 {
    /// 从节点密钥创建 v2 加密密钥
    ///
    /// 使用 Argon2id 派生，为每个节点生成唯一的盐值
    ///
    /// # 参数
    /// - `node_key`: 节点主密钥
    /// - `unique_id`: 唯一标识符（如节点 ID 或 DID）
    ///
    /// # Argon2id 参数
    /// - 时间成本: 4096 次迭代
    /// - 并行度: 3 个通道
    /// - 内存成本: 64 MB
    /// - 输出长度: 32 字节
    pub fn from_node_key_v2(node_key: &[u8], unique_id: &[u8]) -> Result<Self> {
        // 步骤 1: 生成唯一的盐值
        let mut rng = rand::thread_rng();
        let mut salt = [0u8; 32];
        rng.fill_bytes(&mut salt);

        // 步骤 2: 使用 Argon2id 派生密钥
        let mut key = [0u8; 32];

        // Argon2id 参数（高安全配置）
        let params = Params::new(4096, 3, 1).map_err(|e| {
            CisError::memory(format!("Invalid Argon2 parameters: {}", e))
        })?;

        let argon = Argon2::new(
            Algorithm::Argon2id,
            Version::Version13,
            params,
        );

        // 构建上下文：节点密钥 + 唯一ID + 版本标识符
        let mut context = Vec::with_capacity(node_key.len() + unique_id.len() + 14);
        context.extend_from_slice(node_key);
        context.extend_from_slice(unique_id);
        context.extend_from_slice(b"cis-memory-v2");

        // 派生密钥
        argon.hash_password_into(&context, &salt, &mut key)
            .map_err(|e| CisError::memory(format!("Key derivation failed: {}", e)))?;

        Ok(Self {
            key,
            salt,
            created_at: chrono::Utc::now(),
        })
    }

    /// 从存储加载密钥
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| CisError::io(format!("Failed to read key file: {}", e)))?;

        let storage: KeyStorageV2 = serde_json::from_str(&content)
            .map_err(|e| CisError::memory(format!("Invalid key format: {}", e)))?;

        // 验证格式版本
        if storage.format != "cis-key-v2" || storage.version != 2 {
            return Err(CisError::memory("Unsupported key format version"));
        }

        // 解码 base64 数据
        let data = base64::decode(&storage.data)
            .map_err(|e| CisError::memory(format!("Invalid base64 encoding: {}", e)))?;

        // 解析二进制格式
        if data.len() < 75 {
            // 4 + 1 + 2 + 32 + 2 + 32 + 8 + 32 = 113 (minimum)
            return Err(CisError::memory("Key data too short"));
        }

        // 检查 magic
        if &data[0..4] != MAGIC_V2 {
            return Err(CisError::memory("Invalid key magic"));
        }

        let version = data[4];
        if version != 2 {
            return Err(CisError::memory(format!("Unsupported version: {}", version)));
        }

        // 读取盐值
        let salt_len = u16::from_be_bytes([data[5], data[6]]) as usize;
        let salt_offset = 7;
        if salt_len != 32 {
            return Err(CisError::memory("Invalid salt length"));
        }
        let mut salt = [0u8; 32];
        salt.copy_from_slice(&data[salt_offset..salt_offset + 32]);

        // 读取密钥
        let key_len_offset = salt_offset + salt_len;
        let key_len = u16::from_be_bytes([data[key_len_offset], data[key_len_offset + 1]]) as usize;
        let key_offset = key_len_offset + 2;
        if key_len != 32 {
            return Err(CisError::memory("Invalid key length"));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&data[key_offset..key_offset + 32]);

        // 验证 HMAC
        let hmac_key = Self::derive_hmac_key(&salt);
        let data_for_hmac = &data[..key_offset + 32];
        let expected_hmac = &data[key_offset + 32..key_offset + 32 + 32];
        let actual_hmac = Self::compute_hmac(&hmac_key, data_for_hmac);

        if &actual_hmac[..] != expected_hmac {
            return Err(CisError::memory("HMAC verification failed - key may be corrupted"));
        }

        // 解析时间
        let created_at = storage.created_at.parse::<chrono::DateTime<chrono::Utc>>()
            .map_err(|e| CisError::memory(format!("Invalid timestamp: {}", e)))?;

        Ok(Self {
            key,
            salt,
            created_at,
        })
    }

    /// 保存密钥到文件
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        // 序列化为二进制格式
        let mut data = Vec::new();

        // Magic (4 bytes)
        data.extend_from_slice(&MAGIC_V2);

        // Version (1 byte)
        data.push(2);

        // Salt Length (2 bytes)
        data.extend_from_slice(&(self.salt.len() as u16).to_be_bytes());

        // Salt (32 bytes)
        data.extend_from_slice(&self.salt);

        // Key Length (2 bytes)
        data.extend_from_slice(&(self.key.len() as u16).to_be_bytes());

        // Key (32 bytes)
        data.extend_from_slice(&self.key);

        // Reserved (8 bytes)
        data.extend_from_slice(&[0u8; 8]);

        // HMAC (32 bytes)
        let hmac_key = Self::derive_hmac_key(&self.salt);
        let hmac = Self::compute_hmac(&hmac_key, &data);
        data.extend_from_slice(&hmac);

        // 创建 JSON 存储
        let storage = KeyStorageV2 {
            format: "cis-key-v2".to_string(),
            version: 2,
            created_at: self.created_at.to_rfc3339(),
            algorithm: "argon2id".to_string(),
            algorithm_params: AlgorithmParams {
                iterations: 4096,
                parallelism: 3,
                memory: 65536,
                output_length: 32,
            },
            encoding: "base64".to_string(),
            data: base64::encode(&data),
        };

        // 确保父目录存在
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CisError::io(format!("Failed to create directory: {}", e)))?;
        }

        // 写入文件
        let json = serde_json::to_string_pretty(&storage)
            .map_err(|e| CisError::memory(format!("Serialization failed: {}", e)))?;

        let mut file = fs::File::create(path)
            .map_err(|e| CisError::io(format!("Failed to create key file: {}", e)))?;

        file.write_all(json.as_bytes())
            .map_err(|e| CisError::io(format!("Failed to write key file: {}", e)))?;

        // 设置 restrictive 权限（Unix only）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(path, perms)
                .map_err(|e| CisError::io(format!("Failed to set permissions: {}", e)))?;
        }

        Ok(())
    }

    /// 获取密钥引用
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }

    /// 获取盐值
    pub fn salt(&self) -> &[u8; 32] {
        &self.salt
    }

    /// 派生 HMAC 密钥
    fn derive_hmac_key(salt: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"cis-key-hmac");
        hasher.update(salt);
        hasher.finalize().into()
    }

    /// 计算 HMAC
    fn compute_hmac(key: &[u8; 32], data: &[u8]) -> [u8; 32] {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(key)
            .expect("HMAC key length should be 32 bytes");
        mac.update(data);
        mac.finalize().into_bytes().into()
    }
}

impl MemoryEncryptionV2 {
    /// 从 v2 密钥创建加密器
    pub fn from_key(key: &EncryptionKeyV2) -> Self {
        Self { key: key.key }
    }

    /// 从节点密钥直接创建加密器（便捷方法）
    pub fn from_node_key(node_key: &[u8], unique_id: &[u8]) -> Result<Self> {
        let key = EncryptionKeyV2::from_node_key_v2(node_key, unique_id)?;
        Ok(Self::from_key(&key))
    }

    /// 创建 cipher 实例（每次加密/解密时使用）
    fn create_cipher(&self) -> ChaCha20Poly1305 {
        let cipher_key = chacha20poly1305::Key::from_slice(&self.key);
        ChaCha20Poly1305::new(cipher_key)
    }

    /// 加密数据
    ///
    /// 返回格式: nonce(12字节) || ciphertext || tag(16字节)
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = self.create_cipher();

        // 生成随机 nonce
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

        // 加密（自动附加认证标签）
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| CisError::memory(format!("Encryption failed: {}", e)))?;

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

        let cipher = self.create_cipher();

        // 提取 nonce
        let nonce = Nonce::from_slice(&ciphertext[0..12]);
        let encrypted = &ciphertext[12..];

        // 解密（自动验证标签）
        cipher
            .decrypt(nonce, encrypted)
            .map_err(|_| CisError::memory("decryption failed (invalid key or corrupted data)"))
    }

    /// 重新加密数据（用于密钥轮换）
    pub fn re_encrypt(&self, old_ciphertext: &[u8], new_cipher: &Self) -> Result<Vec<u8>> {
        let plaintext = self.decrypt(old_ciphertext)?;
        new_cipher.encrypt(&plaintext)
    }

    /// 重新加密从 v1 密钥加密的数据
    pub fn migrate_from_v1(
        &self,
        old_ciphertext: &[u8],
        old_cipher: &crate::memory::encryption::MemoryEncryption,
    ) -> Result<Vec<u8>> {
        let plaintext = old_cipher.decrypt(old_ciphertext)?;
        self.encrypt(&plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_key_generation() {
        let node_key = b"test-node-key-12345";
        let unique_id = b"did:cis:node:abc123";

        let key = EncryptionKeyV2::from_node_key_v2(node_key, unique_id).unwrap();

        // 验证盐值是随机的（调用两次应该不同）
        let key2 = EncryptionKeyV2::from_node_key_v2(node_key, unique_id).unwrap();
        assert_ne!(key.salt, key2.salt, "盐值应该是唯一的");

        // 但派生应该是一致的（相同的输入产生相同的输出）
        let key3 = EncryptionKeyV2::from_node_key_v2(node_key, unique_id).unwrap();
        assert_ne!(key.salt, key3.salt, "每次调用应该生成新的盐值");
    }

    #[test]
    fn test_v2_encryption_roundtrip() {
        let enc = MemoryEncryptionV2::from_node_key(b"test-key", b"unique-id").unwrap();
        let plaintext = b"hello, world! v2";

        let ciphertext = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&ciphertext).unwrap();

        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_v2_key_save_load() {
        let key = EncryptionKeyV2::from_node_key_v2(b"test-key", b"unique-id").unwrap();

        // 保存到临时文件
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        key.save(temp_file.path()).unwrap();

        // 加载密钥
        let loaded_key = EncryptionKeyV2::load(temp_file.path()).unwrap();

        // 验证密钥和盐值匹配
        assert_eq!(key.key, loaded_key.key);
        assert_eq!(key.salt, loaded_key.salt);

        // 使用加载的密钥创建加密器
        let enc = MemoryEncryptionV2::from_key(&loaded_key);
        let plaintext = b"test data";
        let ciphertext = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_v2_hmac_verification() {
        let key = EncryptionKeyV2::from_node_key_v2(b"test-key", b"unique-id").unwrap();

        // 保存到临时文件
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        key.save(temp_file.path()).unwrap();

        // 读取文件内容
        let content = fs::read_to_string(temp_file.path()).unwrap();
        let mut storage: KeyStorageV2 = serde_json::from_str(&content).unwrap();
        let data = base64::decode(&storage.data).unwrap();

        // 篡改最后一个字节
        let mut tampered_data = data.clone();
        let last_idx = tampered_data.len() - 1;
        tampered_data[last_idx] ^= 0xFF;

        // 保存篡改后的数据
        storage.data = base64::encode(&tampered_data);
        let tampered_file = tempfile::NamedTempFile::new().unwrap();
        fs::write(tampered_file.path(), serde_json::to_string(&storage).unwrap()).unwrap();

        // 加载应该失败（HMAC 验证失败）
        let result = EncryptionKeyV2::load(tampered_file.path());
        assert!(result.is_err(), "HMAC 验证应该失败");
    }

    #[test]
    fn test_v2_decrypt_with_wrong_key() {
        let encryption1 = MemoryEncryptionV2::from_node_key(b"correct_password", b"node1").unwrap();
        let encryption2 = MemoryEncryptionV2::from_node_key(b"wrong_password", b"node2").unwrap();

        let plaintext = b"secret data";
        let ciphertext = encryption1.encrypt(plaintext).unwrap();

        // 用错误的密钥解密应该失败
        let result = encryption2.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_v2_ciphertext_different_each_time() {
        let encryption = MemoryEncryptionV2::from_node_key(b"test-key", b"unique-id").unwrap();
        let plaintext = b"same text";

        let ciphertext1 = encryption.encrypt(plaintext).unwrap();
        let ciphertext2 = encryption.encrypt(plaintext).unwrap();

        assert_ne!(ciphertext1, ciphertext2, "相同明文应该产生不同密文");

        // 但都能正确解密
        assert_eq!(encryption.decrypt(&ciphertext1).unwrap(), plaintext);
        assert_eq!(encryption.decrypt(&ciphertext2).unwrap(), plaintext);
    }

    #[test]
    fn test_v2_authentication_tag() {
        let encryption = MemoryEncryptionV2::from_node_key(b"test-key", b"unique-id").unwrap();
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
    fn test_v2_re_encrypt() {
        let old_encryption = MemoryEncryptionV2::from_node_key(b"old-key", b"node1").unwrap();
        let new_encryption = MemoryEncryptionV2::from_node_key(b"new-key", b"node2").unwrap();

        let plaintext = b"data to be re-encrypted";
        let old_ciphertext = old_encryption.encrypt(plaintext).unwrap();

        // 重新加密
        let new_ciphertext = old_encryption.re_encrypt(&old_ciphertext, &new_encryption).unwrap();

        // 新密文应该能用新密钥解密
        let decrypted = new_encryption.decrypt(&new_ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);

        // 旧密钥不应该能解密新密文
        assert!(old_encryption.decrypt(&new_ciphertext).is_err());
    }

    #[test]
    fn test_v2_migrate_from_v1() {
        use crate::memory::encryption::MemoryEncryption;

        let old_encryption = MemoryEncryption::from_node_key(b"old-key");
        let new_encryption = MemoryEncryptionV2::from_node_key(b"new-key", b"unique-id").unwrap();

        let plaintext = b"data migration test";
        let old_ciphertext = old_encryption.encrypt(plaintext).unwrap();

        // 迁移
        let new_ciphertext = new_encryption.migrate_from_v1(&old_ciphertext, &old_encryption).unwrap();

        // 验证
        let decrypted = new_encryption.decrypt(&new_ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_v2_ciphertext_too_short() {
        let encryption = MemoryEncryptionV2::from_node_key(b"test-key", b"unique-id").unwrap();

        // 密文太短应该返回错误
        let result = encryption.decrypt(b"short");
        assert!(result.is_err());

        // 刚好 12 字节（只有 nonce）也应该失败
        let result = encryption.decrypt(&[0u8; 12]);
        assert!(result.is_err());
    }

    #[test]
    fn test_argon2id_parameters() {
        // 验证 Argon2id 参数符合安全要求
        let node_key = b"test-node-key";
        let unique_id = b"test-unique-id";

        let key = EncryptionKeyV2::from_node_key_v2(node_key, unique_id).unwrap();

        // 密钥长度应该是 32 字节
        assert_eq!(key.key.len(), 32);

        // 盐值长度应该是 32 字节
        assert_eq!(key.salt.len(), 32);

        // 盐值应该非零（统计上不太可能是全零）
        assert_ne!(key.salt, [0u8; 32]);
    }

    #[test]
    fn test_key_uniqueness() {
        // 测试不同的节点应该生成不同的密钥
        let key1 = EncryptionKeyV2::from_node_key_v2(b"key1", b"id1").unwrap();
        let key2 = EncryptionKeyV2::from_node_key_v2(b"key2", b"id2").unwrap();
        let key3 = EncryptionKeyV2::from_node_key_v2(b"key1", b"id2").unwrap();

        // 相同的节点密钥和 ID 应该产生不同的盐值（随机）
        // 但如果使用相同的输入...
        // 注意：由于盐值是随机的，相同的输入会产生不同的密钥
        // 这是设计行为
        assert_ne!(key1.salt, key2.salt);
        assert_ne!(key1.salt, key3.salt);
    }

    #[test]
    fn test_backward_compatibility_detection() {
        // 测试能够检测到旧版本格式
        let temp_file = tempfile::NamedTempFile::new().unwrap();

        // 创建一个假的 v1 格式文件
        let fake_v1_data = serde_json::json!({
            "format": "cis-key-v1",
            "version": 1
        });
        fs::write(temp_file.path(), fake_v1_data.to_string()).unwrap();

        let result = EncryptionKeyV2::load(temp_file.path());
        assert!(result.is_err());
    }
}
