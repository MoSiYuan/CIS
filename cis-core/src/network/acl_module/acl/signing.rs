//! ACL 签名模块
//!
//! 处理 ACL 条目的签名和验证，包含时间戳。

use crate::error::{CisError, Result};
use crate::network::acl::{AclEntry, NetworkAcl, NetworkMode};
use ed25519_dalek::{Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// ACL 签名器
///
/// 用于创建和验证 ACL 条目的签名。
pub struct AclSigner {
    /// 签名密钥
    signing_key: ed25519_dalek::SigningKey,
}

impl AclSigner {
    /// 从字节创建签名器
    pub fn from_bytes(key_bytes: &[u8; 32]) -> Self {
        let signing_key = ed25519_dalek::SigningKey::from_bytes(key_bytes);
        Self { signing_key }
    }

    /// 从 hex 字符串创建签名器
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let key_bytes = hex::decode(hex_str)
            .map_err(|e| CisError::crypto(format!("Invalid hex: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(CisError::crypto("Signing key must be 32 bytes"));
        }

        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&key_bytes);

        Ok(Self::from_bytes(&key_array))
    }

    /// 签名 ACL 条目（包含时间戳）
    ///
    /// # 参数
    /// - `entry`: 要签名的 ACL 条目
    ///
    /// # 返回
    /// - `Result<String>`: Base64 编码的签名
    pub fn sign_entry(&self, entry: &AclEntry) -> Result<String> {
        // 创建待签名数据（包含时间戳）
        let data = self.prepare_signing_data(entry);

        // 签名
        let signature = self
            .signing_key
            .sign(&data)
            .to_bytes();

        // Base64 编码
        Ok(base64::encode(&signature))
    }

    /// 签名整个 ACL 配置
    ///
    /// # 参数
    /// - `acl`: 要签名的 ACL 配置
    ///
    /// # 返回
    /// - `Result<String>`: Base64 编码的签名
    pub fn sign_acl(&self, acl: &NetworkAcl) -> Result<String> {
        // 创建待签名数据（不包含签名字段）
        let payload = AclPayload::from_acl(acl);

        // 序列化
        let data =
            serde_json::to_vec(&payload)
                .map_err(|e| CisError::serialization(format!("Failed to serialize ACL: {}", e)))?;

        // 签名
        let signature = self
            .signing_key
            .sign(&data)
            .to_bytes();

        // Base64 编码
        Ok(base64::encode(&signature))
    }

    /// 准备签名数据（包含时间戳）
    fn prepare_signing_data(&self, entry: &AclEntry) -> Vec<u8> {
        // 创建包含时间戳的数据结构
        let sign_data = AclEntrySignData {
            did: &entry.did,
            added_at: entry.added_at,
            added_by: &entry.added_by,
            reason: entry.reason.as_deref(),
            expires_at: entry.expires_at,
        };

        // 序列化为 JSON（规范格式）
        serde_json::to_vec(&sign_data).unwrap_or_default()
    }

    /// 获取公钥（Base64 编码）
    pub fn public_key_base64(&self) -> String {
        let public_key = self.signing_key.verifying_key();
        base64::encode(&public_key.to_bytes())
    }

    /// 获取公钥字节
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

/// ACL 验证器
///
/// 用于验证 ACL 条目的签名。
pub struct AclVerifier {
    /// 验证密钥
    verifying_key: ed25519_dalek::VerifyingKey,
}

impl AclVerifier {
    /// 从字节创建验证器
    pub fn from_bytes(key_bytes: &[u8; 32]) -> Result<Self> {
        let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(key_bytes)
            .map_err(|e| CisError::crypto(format!("Invalid verifying key: {}", e)))?;

        Ok(Self { verifying_key })
    }

    /// 从 hex 字符串创建验证器
    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let key_bytes = hex::decode(hex_str)
            .map_err(|e| CisError::crypto(format!("Invalid hex: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(CisError::crypto("Verifying key must be 32 bytes"));
        }

        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&key_bytes);

        Self::from_bytes(&key_array)
    }

    /// 从签名器提取验证器
    pub fn from_signer(signer: &AclSigner) -> Self {
        Self {
            verifying_key: signer.signing_key.verifying_key(),
        }
    }

    /// 验证 ACL 条目签名
    ///
    /// # 参数
    /// - `entry`: 要验证的 ACL 条目
    /// - `signature`: Base64 编码的签名
    ///
    /// # 返回
    /// - `Result<()>`: 签名有效返回 Ok，否则返回错误
    pub fn verify_entry(&self, entry: &AclEntry, signature: &str) -> Result<()> {
        // 解码签名
        let signature_bytes = base64::decode(signature)
            .map_err(|e| CisError::crypto(format!("Invalid base64: {}", e)))?;

        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|e| CisError::crypto(format!("Invalid signature: {}", e)))?;

        // 准备签名数据
        let data = self.prepare_signing_data(entry);

        // 验证签名
        self.verifying_key
            .verify(&data, &signature)
            .map_err(|e| CisError::crypto(format!("Signature verification failed: {}", e)))
    }

    /// 验证 ACL 配置签名
    ///
    /// # 参数
    /// - `acl`: 要验证的 ACL 配置
    /// - `signature`: Base64 编码的签名
    ///
    /// # 返回
    /// - `Result<()>`: 签名有效返回 Ok，否则返回错误
    pub fn verify_acl(&self, acl: &NetworkAcl, signature: &str) -> Result<()> {
        // 解码签名
        let signature_bytes = base64::decode(signature)
            .map_err(|e| CisError::crypto(format!("Invalid base64: {}", e)))?;

        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|e| CisError::crypto(format!("Invalid signature: {}", e)))?;

        // 创建待验证数据
        let payload = AclPayload::from_acl(acl);

        // 序列化
        let data =
            serde_json::to_vec(&payload)
                .map_err(|e| CisError::serialization(format!("Failed to serialize ACL: {}", e)))?;

        // 验证签名
        self.verifying_key
            .verify(&data, &signature)
            .map_err(|e| CisError::crypto(format!("ACL signature verification failed: {}", e)))
    }

    /// 准备签名数据（包含时间戳）
    fn prepare_signing_data(&self, entry: &AclEntry) -> Vec<u8> {
        // 创建包含时间戳的数据结构
        let sign_data = AclEntrySignData {
            did: &entry.did,
            added_at: entry.added_at,
            added_by: &entry.added_by,
            reason: entry.reason.as_deref(),
            expires_at: entry.expires_at,
        };

        // 序列化为 JSON（规范格式）
        serde_json::to_vec(&sign_data).unwrap_or_default()
    }
}

/// ACL 条目签名数据（包含时间戳）
#[derive(Serialize)]
struct AclEntrySignData<'a> {
    did: &'a str,
    added_at: i64,
    added_by: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
}

/// ACL 负载（用于签名整个 ACL）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AclPayload {
    local_did: String,
    mode: NetworkMode,
    whitelist: Vec<AclEntry>,
    blacklist: Vec<AclEntry>,
    version: u64,
    updated_at: i64,
}

impl AclPayload {
    /// 从 ACL 创建负载（排除签名字段）
    fn from_acl(acl: &NetworkAcl) -> Self {
        Self {
            local_did: acl.local_did.clone(),
            mode: acl.mode,
            whitelist: acl.whitelist.clone(),
            blacklist: acl.blacklist.clone(),
            version: acl.version,
            updated_at: acl.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_sign_and_verify_entry() {
        // 创建签名器
        let signing_key = [1u8; 32];
        let signer = AclSigner::from_bytes(&signing_key);

        // 创建 ACL 条目
        let entry = AclEntry {
            did: "did:cis:test".to_string(),
            added_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            added_by: "did:cis:admin".to_string(),
            reason: Some("Test reason".to_string()),
            expires_at: Some(12345),
        };

        // 签名
        let signature = signer.sign_entry(&entry).unwrap();

        // 验证
        let verifier = AclVerifier::from_signer(&signer);
        verifier.verify_entry(&entry, &signature).unwrap();
    }

    #[test]
    fn test_verify_invalid_signature() {
        let signing_key = [1u8; 32];
        let signer = AclSigner::from_bytes(&signing_key);

        let entry = AclEntry {
            did: "did:cis:test".to_string(),
            added_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            added_by: "did:cis:admin".to_string(),
            reason: None,
            expires_at: None,
        };

        // 使用错误的签名
        let invalid_signature = "invalid_signature_base64";
        let verifier = AclVerifier::from_signer(&signer);

        let result = verifier.verify_entry(&entry, invalid_signature);
        assert!(result.is_err());
    }

    #[test]
    fn test_sign_and_verify_acl() {
        let signing_key = [2u8; 32];
        let signer = AclSigner::from_bytes(&signing_key);

        let mut acl = NetworkAcl::new("did:cis:local");
        acl.allow("did:cis:friend", "did:cis:local");

        // 签名 ACL
        let signature = signer.sign_acl(&acl).unwrap();

        // 验证
        let verifier = AclVerifier::from_signer(&signer);
        verifier.verify_acl(&acl, &signature).unwrap();
    }

    #[test]
    fn test_tampered_acl() {
        let signing_key = [3u8; 32];
        let signer = AclSigner::from_bytes(&signing_key);

        let mut acl = NetworkAcl::new("did:cis:local");
        acl.allow("did:cis:friend", "did:cis:local");

        let signature = signer.sign_acl(&acl).unwrap();

        // 篡改 ACL
        acl.mode = NetworkMode::Open;

        // 验证应该失败
        let verifier = AclVerifier::from_signer(&signer);
        let result = verifier.verify_acl(&acl, &signature);
        assert!(result.is_err());
    }

    #[test]
    fn test_public_key_extraction() {
        let signing_key_bytes = [4u8; 32];
        let signer = AclSigner::from_bytes(&signing_key_bytes);

        let pub_key_b64 = signer.public_key_base64();
        let pub_key_bytes = signer.public_key_bytes();

        // 验证可以从公钥创建验证器
        let verifier = AclVerifier::from_bytes(&pub_key_bytes).unwrap();
        assert!(verifier.verifying_key.to_bytes() == pub_key_bytes);

        // 可以从 base64 创建
        let verifier2 = AclVerifier::from_hex(&pub_key_b64).unwrap();
        assert_eq!(
            verifier2.verifying_key.to_bytes(),
            verifier.verifying_key.to_bytes()
        );
    }
}
