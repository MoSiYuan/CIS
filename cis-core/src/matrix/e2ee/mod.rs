//! # Matrix 端到端加密 (E2EE) 模块
//!
//! 提供基于 Olm 和 Megolm 协议的端到端加密支持。
//!
//! ## 模块概述
//!
//! - **olm**: 双棘轮算法实现，用于一对一设备加密
//! - **megolm**: 对称密钥加密，用于群组消息加密
//!
//! ## 使用示例
//!
//! ### Olm 加密（一对一）
//!
//! ```rust
//! use cis_core::matrix::e2ee::{OlmAccount, OlmMessage};
//!
//! // 创建账户
//! let mut alice = OlmAccount::new();
//! let mut bob = OlmAccount::new();
//!
//! // Bob 生成一次性格钥
//! bob.generate_one_time_keys(1);
//! let bob_otk = &bob.one_time_keys()[0];
//!
//! // Alice 创建会话并加密
//! let session_id = alice.create_outbound_session(
//!     &bob.identity_keys().curve25519,
//!     &bob_otk.key,
//! ).unwrap();
//!
//! let message = alice.encrypt(&session_id, "Hello, Bob!").unwrap();
//! ```
//!
//! ### Megolm 加密（群组）
//!
//! ```rust
//! use cis_core::matrix::e2ee::{MegolmSession, GroupSessionManager};
//!
//! // 创建群组会话管理器
//! let mut manager = GroupSessionManager::new();
//!
//! // 创建出站会话
//! let session_id = manager.create_outbound_session(
//!     "!room:example.com",
//!     "my_device",
//!     "my_sender_key",
//! );
//!
//! // 加密消息
//! let encrypted = manager.encrypt_room_message(
//!     "!room:example.com",
//!     "Hello, group!",
//! ).unwrap();
//! ```

pub mod megolm;
pub mod olm;

// 重新导出主要类型
pub use megolm::{
    DeviceKey, EncryptedEvent, GroupSessionManager, MegolmError, MegolmResult, MegolmSession,
    SessionExport, SessionInfo, SharedSessionKey,
};

pub use olm::{
    DeviceInfo, DeviceManager, IdentityKeys, OlmAccount, OlmError, OlmMessage, OlmResult,
    OneTimeKey, VerificationStatus,
};

/// 密钥上传 API 路径
pub const KEY_UPLOAD_PATH: &str = "/_matrix/client/v3/keys/upload";

/// 密钥查询 API 路径
pub const KEY_QUERY_PATH: &str = "/_matrix/client/v3/keys/query";

/// 声明密钥 API 路径
pub const KEY_CLAIM_PATH: &str = "/_matrix/client/v3/keys/claim";

/// 设备密钥算法标识
pub const ED25519_ALGORITHM: &str = "ed25519";
pub const CURVE25519_ALGORITHM: &str = "curve25519";
pub const MEGOLM_ALGORITHM: &str = "m.megolm.v1.aes-sha2";
pub const OLM_ALGORITHM: &str = "m.olm.v1.curve25519-aes-sha2";

/// 加密管理器
///
/// 统一管理 Olm 和 Megolm 加密
pub struct EncryptionManager {
    /// Olm 账户
    olm_account: OlmAccount,
    /// 群组会话管理器
    group_manager: GroupSessionManager,
    /// 设备管理器
    device_manager: DeviceManager,
}

impl EncryptionManager {
    /// 创建新的加密管理器
    pub fn new() -> Self {
        Self {
            olm_account: OlmAccount::new(),
            group_manager: GroupSessionManager::new(),
            device_manager: DeviceManager::new(),
        }
    }

    /// 从持久化数据恢复
    pub fn from_pickle(
        olm_pickle: &str,
        pickle_key: Option<&[u8]>,
    ) -> crate::error::Result<Self> {
        Ok(Self {
            olm_account: OlmAccount::from_pickle(olm_pickle, pickle_key)
                .map_err(|e| crate::error::CisError::encryption(e.to_string()))?,
            group_manager: GroupSessionManager::new(),
            device_manager: DeviceManager::new(),
        })
    }

    /// 获取 Olm 账户
    pub fn olm_account(&self) -> &OlmAccount {
        &self.olm_account
    }

    /// 获取可变的 Olm 账户
    pub fn olm_account_mut(&mut self) -> &mut OlmAccount {
        &mut self.olm_account
    }

    /// 获取群组管理器
    pub fn group_manager(&self) -> &GroupSessionManager {
        &self.group_manager
    }

    /// 获取可变的群组管理器
    pub fn group_manager_mut(&mut self) -> &mut GroupSessionManager {
        &mut self.group_manager
    }

    /// 获取设备管理器
    pub fn device_manager(&self) -> &DeviceManager {
        &self.device_manager
    }

    /// 获取可变的设备管理器
    pub fn device_manager_mut(&mut self) -> &mut DeviceManager {
        &mut self.device_manager
    }

    /// 获取本机设备密钥
    pub fn device_key(&self) -> &str {
        self.olm_account.device_key()
    }

    /// 序列化保存
    pub fn to_pickle(&self, pickle_key: Option<&[u8]>) -> String {
        self.olm_account.to_pickle(pickle_key)
    }
}

impl Default for EncryptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_manager_creation() {
        let manager = EncryptionManager::new();
        assert!(!manager.device_key().is_empty());
    }

    #[test]
    fn test_full_e2ee_flow() {
        // Alice 和 Bob 各自创建加密管理器
        let mut alice = EncryptionManager::new();
        let mut bob = EncryptionManager::new();

        // 1. 设备发现阶段 - 交换身份密钥
        let bob_identity_key = bob.olm_account().identity_keys().curve25519.clone();
        
        // 2. Bob 生成一次性格钥供 Alice 使用
        bob.olm_account_mut().generate_one_time_keys(1);
        let bob_otk = bob.olm_account().one_time_keys()[0].key.clone();

        // 3. Alice 创建到 Bob 的 Olm 会话
        let session_id = alice
            .olm_account_mut()
            .create_outbound_session(&bob_identity_key, &bob_otk)
            .unwrap();

        // 4. Alice 通过 Olm 发送 Megolm 会话密钥给 Bob
        let mut alice_room_session = MegolmSession::new(
            "alice_device",
            &alice.olm_account().identity_keys().ed25519,
        );
        let session_key = alice_room_session.session_key_base64()
            .expect("Session key should be available");

        // 5. Alice 加密消息
        let plaintext = "Hello, encrypted world!";
        let encrypted = alice_room_session.encrypt(plaintext).unwrap();

        // 6. Bob 接收到会话密钥后，创建入站 Megolm 会话
        let mut bob_room_session = MegolmSession::from_key(
            &session_key,
            "alice_device",
            &alice.olm_account().identity_keys().ed25519,
        )
        .unwrap();

        // 7. Bob 解密消息
        let decrypted = bob_room_session.decrypt(&encrypted.ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
