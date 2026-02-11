//! # Olm 端到端加密实现
//!
//! 基于双棘轮算法 (Double Ratchet Algorithm) 的端到端加密实现。
//! Olm 用于一对一设备间的加密通信。
//!
//! ## 核心概念
//!
//! - **Identity Keys**: 长期的 Ed25519 密钥对，用于设备身份验证
//! - **One-Time Keys**: 临时的 X25519 密钥对，用于建立会话
//! - **Session**: 两个设备之间的加密通道
//!
//! ## 协议流程
//!
//! 1. **会话建立**: Alice 使用 Bob 的 identity key 和 one-time key 创建会话
//! 2. **加密**: 每条消息使用前向安全密钥加密
//! 3. **解密**: 接收方使用相应的密钥解密消息
//! 4. **密钥更新**: 双棘轮算法确保每条消息使用不同的密钥

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use vodozemac::olm::{
    Account, AccountPickle, IdentityKeys as VzIdentityKeys, InboundCreationResult,
    OlmMessage as VzOlmMessage, Session, SessionConfig,
};

/// Olm 错误类型
#[derive(Debug, Error)]
pub enum OlmError {
    #[error("加密失败: {0}")]
    Encryption(String),
    
    #[error("解密失败: {0}")]
    Decryption(String),
    
    #[error("无效的密钥格式: {0}")]
    InvalidKey(String),
    
    #[error("会话未找到: {0}")]
    SessionNotFound(String),
    
    #[error("一次性格钥已用尽")]
    OneTimeKeysExhausted,
    
    #[error("Vodozemac 错误: {0}")]
    Vodozemac(String),
}

/// Olm 结果类型
pub type OlmResult<T> = Result<T, OlmError>;

/// 身份密钥对
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityKeys {
    /// Ed25519 公钥（用于签名）
    pub ed25519: String,
    /// X25519 公钥（用于加密）
    pub curve25519: String,
}

impl From<VzIdentityKeys> for IdentityKeys {
    fn from(keys: VzIdentityKeys) -> Self {
        Self {
            ed25519: keys.ed25519.to_base64(),
            curve25519: keys.curve25519.to_base64(),
        }
    }
}

/// 一次性格钥
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKey {
    /// 密钥 ID（字符串格式）
    pub key_id: String,
    /// 公钥（base64 编码）
    pub key: String,
}

/// Olm 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OlmMessage {
    /// 预密钥消息（用于建立新会话）
    PreKey {
        /// 会话 ID
        session_id: String,
        /// 加密内容（base64 编码）
        ciphertext: String,
    },
    /// 普通消息（已有会话）
    Message {
        /// 会话 ID
        session_id: String,
        /// 加密内容（base64 编码）
        ciphertext: String,
    },
}

/// Olm 账户
/// 
/// 每个设备拥有一个 Olm 账户，用于管理身份密钥和一次性格钥
pub struct OlmAccount {
    /// 底层 vodozemac 账户
    inner: Account,
    /// 身份密钥缓存
    identity_keys: IdentityKeys,
    /// 一次性格钥缓存
    one_time_keys: Vec<OneTimeKey>,
    /// 活跃会话集合（session_id -> Session）
    sessions: HashMap<String, Session>,
    /// 已上传的一次性格钥数量
    uploaded_key_count: usize,
}

impl OlmAccount {
    /// 创建新的 Olm 账户
    ///
    /// # 示例
    /// ```
    /// use cis_core::matrix::e2ee::olm::OlmAccount;
    ///
    /// let account = OlmAccount::new();
    /// ```
    pub fn new() -> Self {
        let account = Account::new();
        let identity_keys = IdentityKeys::from(account.identity_keys());
        
        Self {
            inner: account,
            identity_keys,
            one_time_keys: Vec::new(),
            sessions: HashMap::new(),
            uploaded_key_count: 0,
        }
    }

    /// 从 pickle 恢复账户
    ///
    /// # 参数
    /// * `pickle` - 序列化的账户数据（JSON 格式）
    /// * `pickle_key` - 加密密钥（可选，当前未使用）
    pub fn from_pickle(pickle: &str, _pickle_key: Option<&[u8]>) -> OlmResult<Self> {
        let pickle: AccountPickle = serde_json::from_str(pickle)
            .map_err(|e| OlmError::Vodozemac(format!("Failed to parse pickle: {}", e)))?;

        let account = Account::from_pickle(pickle);
        let identity_keys = IdentityKeys::from(account.identity_keys());
        
        Ok(Self {
            inner: account,
            identity_keys,
            one_time_keys: Vec::new(),
            sessions: HashMap::new(),
            uploaded_key_count: 0,
        })
    }

    /// 序列化账户
    ///
    /// # 参数
    /// * `pickle_key` - 加密密钥（当前未使用，保留供将来使用）
    pub fn to_pickle(&self, _pickle_key: Option<&[u8]>) -> String {
        let pickle = self.inner.pickle();
        serde_json::to_string(&pickle)
            .unwrap_or_default()
    }

    /// 获取身份密钥
    pub fn identity_keys(&self) -> &IdentityKeys {
        &self.identity_keys
    }

    /// 获取 Ed25519 公钥（设备标识）
    pub fn device_key(&self) -> &str {
        &self.identity_keys.ed25519
    }

    /// 生成一次性格钥
    ///
    /// # 参数
    /// * `count` - 要生成的密钥数量
    ///
    /// # 说明
    /// 一次性格钥用于建立新的加密会话，每个密钥只能使用一次。
    /// 当密钥用尽时，需要重新生成并上传。
    pub fn generate_one_time_keys(&mut self, count: usize) {
        self.inner.generate_one_time_keys(count);
        self.update_one_time_keys_cache();
    }

    /// 更新一次性格钥缓存
    fn update_one_time_keys_cache(&mut self) {
        self.one_time_keys = self
            .inner
            .one_time_keys()
            .iter()
            .map(|(key_id, key)| OneTimeKey {
                key_id: String::from(*key_id),  // KeyId 实现了 Into<String>
                key: key.to_base64(),
            })
            .collect();
    }

    /// 获取所有一次性格钥
    pub fn one_time_keys(&self) -> &[OneTimeKey] {
        &self.one_time_keys
    }

    /// 获取未上传的一次性格钥数量
    pub fn unpublished_key_count(&self) -> usize {
        self.one_time_keys.len().saturating_sub(self.uploaded_key_count)
    }

    /// 标记密钥为已上传
    pub fn mark_keys_as_uploaded(&mut self) {
        self.inner.mark_keys_as_published();
        self.uploaded_key_count = self.one_time_keys.len();
    }

    /// 获取最大一次性格钥数量
    pub fn max_one_time_keys(&self) -> usize {
        self.inner.max_number_of_one_time_keys()
    }

    /// 发布一次性格钥（用于上传服务器）
    ///
    /// # 返回
    /// 返回未上传的一次性格钥，格式为 (key_id, base64_key) 的列表
    pub fn unpublished_keys(&self) -> Vec<(String, String)> {
        self.one_time_keys
            .iter()
            .skip(self.uploaded_key_count)
            .map(|k| (k.key_id.clone(), k.key.clone()))
            .collect()
    }

    /// 获取 Curve25519 身份密钥
    pub fn curve25519_key(&self) -> String {
        self.inner.curve25519_key().to_base64()
    }

    /// 创建新的出站加密会话
    ///
    /// # 参数
    /// * `identity_key` - 对方设备的 identity key (Curve25519)
    /// * `one_time_key` - 对方设备的一次性格钥
    ///
    /// # 返回
    /// * `Ok(String)` - 会话 ID
    pub fn create_outbound_session(
        &mut self,
        identity_key: &str,
        one_time_key: &str,
    ) -> OlmResult<String> {
        let identity_key = vodozemac::Curve25519PublicKey::from_base64(identity_key)
            .map_err(|e| OlmError::InvalidKey(e.to_string()))?;
        let one_time_key = vodozemac::Curve25519PublicKey::from_base64(one_time_key)
            .map_err(|e| OlmError::InvalidKey(e.to_string()))?;

        let session = self.inner.create_outbound_session(
            SessionConfig::version_2(),
            identity_key,
            one_time_key,
        );

        let session_id = session.session_id().to_string();
        self.sessions.insert(session_id.clone(), session);

        Ok(session_id)
    }

    /// 从预密钥消息创建入站会话
    ///
    /// # 参数
    /// * `identity_key` - 发送方设备的 identity key (Curve25519)
    /// * `pre_key_message` - 预密钥消息（使用 to_parts 编码）
    ///
    /// # 返回
    /// * `Ok((String, String))` - (会话 ID, 解密后的明文)
    pub fn create_inbound_session(
        &mut self,
        identity_key: &str,
        pre_key_message: &str,
    ) -> OlmResult<(String, String)> {
        let identity_key = vodozemac::Curve25519PublicKey::from_base64(identity_key)
            .map_err(|e| OlmError::InvalidKey(e.to_string()))?;
        
        // 使用 from_parts 解码消息 (type, base64_ciphertext)
        let pre_key_msg = VzOlmMessage::from_parts(0, pre_key_message)
            .map_err(|e| OlmError::InvalidKey(e.to_string()))?;
        
        // 提取 PreKeyMessage
        if let VzOlmMessage::PreKey(ref m) = pre_key_msg {
            let InboundCreationResult { session, plaintext } = self
                .inner
                .create_inbound_session(identity_key, m)
                .map_err(|e| OlmError::Vodozemac(e.to_string()))?;

            let session_id = session.session_id().to_string();
            let plaintext = String::from_utf8(plaintext)
                .map_err(|e| OlmError::Decryption(e.to_string()))?;

            self.sessions.insert(session_id.clone(), session);

            Ok((session_id, plaintext))
        } else {
            Err(OlmError::InvalidKey("Expected PreKey message".to_string()))
        }
    }

    /// 加密消息
    ///
    /// # 参数
    /// * `session_id` - 会话 ID
    /// * `plaintext` - 要加密的明文
    ///
    /// # 返回
    /// * `Ok(OlmMessage)` - 加密后的消息
    pub fn encrypt(&mut self, session_id: &str, plaintext: &str) -> OlmResult<OlmMessage> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| OlmError::SessionNotFound(session_id.to_string()))?;

        let message = session.encrypt(plaintext);

        // 使用 to_parts 获取 (type, base64_ciphertext)
        let (message_type, ciphertext) = message.to_parts();

        // 根据消息类型返回
        match message_type {
            0 => Ok(OlmMessage::PreKey {
                session_id: session_id.to_string(),
                ciphertext,
            }),
            1 => Ok(OlmMessage::Message {
                session_id: session_id.to_string(),
                ciphertext,
            }),
            _ => Err(OlmError::Encryption("Unknown message type".to_string())),
        }
    }

    /// 解密消息
    ///
    /// # 参数
    /// * `session_id` - 会话 ID
    /// * `ciphertext` - base64 编码的密文
    /// * `message_type` - 消息类型（0 = PreKey, 1 = Normal）
    ///
    /// # 返回
    /// * `Ok(String)` - 解密后的明文
    pub fn decrypt(
        &mut self,
        session_id: &str,
        ciphertext: &str,
        message_type: usize,
    ) -> OlmResult<String> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| OlmError::SessionNotFound(session_id.to_string()))?;

        // 使用 from_parts 解析消息
        let message = VzOlmMessage::from_parts(message_type, ciphertext)
            .map_err(|e| OlmError::InvalidKey(e.to_string()))?;

        let plaintext = session
            .decrypt(&message)
            .map_err(|e| OlmError::Decryption(e.to_string()))?;

        String::from_utf8(plaintext).map_err(|e| OlmError::Decryption(e.to_string()))
    }

    /// 获取会话 ID 列表
    pub fn session_ids(&self) -> Vec<&String> {
        self.sessions.keys().collect()
    }

    /// 检查是否有活跃的会话
    pub fn has_session(&self, session_id: &str) -> bool {
        self.sessions.contains_key(session_id)
    }
}

impl Default for OlmAccount {
    fn default() -> Self {
        Self::new()
    }
}

/// 设备验证状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// 未验证
    Unverified,
    /// 验证中
    Pending,
    /// 已验证（受信任）
    Verified,
    /// 已阻止（不受信任）
    Blocked,
}

/// 设备信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// 设备 ID
    pub device_id: String,
    /// 用户 ID
    pub user_id: String,
    /// Ed25519 公钥
    pub ed25519_key: String,
    /// Curve25519 公钥
    pub curve25519_key: String,
    /// 验证状态
    pub verification_status: VerificationStatus,
    /// 显示名称（可选）
    pub display_name: Option<String>,
}

/// 设备密钥管理器
pub struct DeviceManager {
    /// 已知设备集合
    devices: HashMap<String, DeviceInfo>,
    /// 会话到设备的映射
    session_devices: HashMap<String, String>,
}

impl DeviceManager {
    /// 创建新的设备管理器
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            session_devices: HashMap::new(),
        }
    }

    /// 添加或更新设备
    pub fn add_device(&mut self, device: DeviceInfo) {
        let key = format!("{}:{}", device.user_id, device.device_id);
        self.devices.insert(key, device);
    }

    /// 获取设备信息
    pub fn get_device(&self, user_id: &str, device_id: &str) -> Option<&DeviceInfo> {
        let key = format!("{}:{}", user_id, device_id);
        self.devices.get(&key)
    }

    /// 标记设备为已验证
    pub fn verify_device(&mut self, user_id: &str, device_id: &str) -> OlmResult<()> {
        let key = format!("{}:{}", user_id, device_id);
        let device = self
            .devices
            .get_mut(&key)
            .ok_or_else(|| OlmError::InvalidKey(format!("Device {} not found", key)))?;
        
        device.verification_status = VerificationStatus::Verified;
        Ok(())
    }

    /// 关联会话与设备
    pub fn associate_session(&mut self, session_id: &str, device_key: &str) {
        self.session_devices.insert(session_id.to_string(), device_key.to_string());
    }

    /// 获取会话关联的设备
    pub fn get_session_device(&self, session_id: &str) -> Option<&str> {
        self.session_devices.get(session_id).map(|s| s.as_str())
    }

    /// 获取所有已验证设备
    pub fn verified_devices(&self) -> Vec<&DeviceInfo> {
        self.devices
            .values()
            .filter(|d| d.verification_status == VerificationStatus::Verified)
            .collect()
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_olm_account_creation() {
        let account = OlmAccount::new();
        assert!(!account.identity_keys().ed25519.is_empty());
        assert!(!account.identity_keys().curve25519.is_empty());
    }

    #[test]
    fn test_one_time_keys_generation() {
        let mut account = OlmAccount::new();
        account.generate_one_time_keys(10);
        assert_eq!(account.one_time_keys().len(), 10);
    }

    #[test]
    fn test_olm_encrypt_decrypt() {
        let alice = Account::new();
        let mut bob = Account::new();

        // Bob 生成一次性格钥
        bob.generate_one_time_keys(1);
        let bob_otk = bob.one_time_keys().values().next().copied().unwrap();
        bob.mark_keys_as_published();

        // Alice 创建出站会话
        let mut alice_session = alice.create_outbound_session(
            SessionConfig::version_2(),
            bob.curve25519_key(),
            bob_otk,
        );

        // Alice 加密消息
        let plaintext = "Hello, Bob!";
        let alice_msg = alice_session.encrypt(plaintext);

        // Bob 创建入站会话并解密
        if let VzOlmMessage::PreKey(ref m) = alice_msg {
            let InboundCreationResult { 
                session: mut bob_session, 
                plaintext: decrypted 
            } = bob.create_inbound_session(alice.curve25519_key(), m).unwrap();

            // 第一跳消息包含明文
            assert_eq!(decrypted, plaintext.as_bytes());

            // Bob 回复
            let reply = "Hi, Alice!";
            let bob_reply = bob_session.encrypt(reply);

            // Alice 解密回复
            let alice_decrypted = alice_session.decrypt(&bob_reply).unwrap();
            assert_eq!(alice_decrypted, reply.as_bytes());
        } else {
            panic!("Expected PreKey message");
        }
    }

    #[test]
    fn test_pickle_roundtrip() {
        let mut account = OlmAccount::new();
        account.generate_one_time_keys(5);
        
        let pickle = account.to_pickle(None);
        let restored = OlmAccount::from_pickle(&pickle, None).unwrap();
        
        assert_eq!(
            account.identity_keys().ed25519,
            restored.identity_keys().ed25519
        );
    }

    #[test]
    fn test_device_manager() {
        let mut manager = DeviceManager::new();
        
        let device = DeviceInfo {
            device_id: "device1".to_string(),
            user_id: "@alice:example.com".to_string(),
            ed25519_key: "key1".to_string(),
            curve25519_key: "key2".to_string(),
            verification_status: VerificationStatus::Unverified,
            display_name: Some("Alice's Phone".to_string()),
        };
        
        manager.add_device(device);
        assert!(manager.get_device("@alice:example.com", "device1").is_some());
        
        manager.verify_device("@alice:example.com", "device1").unwrap();
        let verified = manager.verified_devices();
        assert_eq!(verified.len(), 1);
    }
}
