//! P2P 加密密钥管理

use ed25519_dalek::{Signer, SigningKey, VerifyingKey, Signature, Verifier};
use x25519_dalek::{StaticSecret, PublicKey as X25519PublicKey};
use crate::error::{CisError, Result};
use rand::rngs::OsRng;

/// 节点密钥对
pub struct NodeKeyPair {
    /// Ed25519 签名密钥 (用于 DID 身份)
    ed25519_signing: SigningKey,
    /// X25519 加密密钥 (用于 Noise 握手)
    x25519: StaticSecret,
}

impl std::fmt::Debug for NodeKeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeKeyPair")
            .field("ed25519_public", &self.ed25519_public())
            .field("x25519_public", &self.x25519_public())
            .finish()
    }
}

impl NodeKeyPair {
    /// 从种子生成密钥对
    pub fn from_seed(seed: &[u8; 64]) -> Result<Self> {
        // Ed25519 签名密钥
        let ed25519_seed: [u8; 32] = seed[..32].try_into()
            .map_err(|_| CisError::crypto("Invalid Ed25519 seed length"))?;
        let ed25519_signing = SigningKey::from_bytes(&ed25519_seed);
        
        // X25519 加密密钥
        let x25519_seed: [u8; 32] = seed[32..64].try_into()
            .map_err(|_| CisError::crypto("Invalid X25519 seed length"))?;
        let x25519 = StaticSecret::from(x25519_seed);
        
        Ok(Self { ed25519_signing, x25519 })
    }
    
    /// 从 DID 助记词生成
    pub fn from_mnemonic(mnemonic: &str) -> Result<Self> {
        use bip39::{Mnemonic, Language};
        
        let mn = Mnemonic::parse_in(Language::English, mnemonic)
            .map_err(|e| CisError::crypto(format!("Invalid mnemonic: {}", e)))?;
        
        // 使用 BIP39 种子生成
        let seed = mn.to_seed("cis");
        let seed_bytes: [u8; 64] = seed[..64].try_into()
            .map_err(|_| CisError::crypto("Seed length mismatch"))?;
        
        Self::from_seed(&seed_bytes)
    }
    
    /// 生成新的随机密钥对
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let ed25519_signing = SigningKey::generate(&mut rng);
        let x25519 = StaticSecret::random_from_rng(&mut rng);
        
        Self { ed25519_signing, x25519 }
    }
    
    /// 获取 Ed25519 验证密钥（公钥）
    pub fn ed25519_public(&self) -> VerifyingKey {
        self.ed25519_signing.verifying_key()
    }
    
    /// 获取 X25519 公钥 (用于 Noise 握手)
    pub fn x25519_public(&self) -> X25519PublicKey {
        X25519PublicKey::from(&self.x25519)
    }
    
    /// 获取 X25519 私钥引用
    pub fn x25519_secret(&self) -> &StaticSecret {
        &self.x25519
    }
    
    /// 签名数据
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.ed25519_signing.sign(data)
    }
    
    /// 验证签名
    pub fn verify(&self, data: &[u8], signature: &Signature) -> Result<()> {
        self.ed25519_signing.verifying_key().verify(data, signature)
            .map_err(|e| CisError::crypto(format!("Signature verification failed: {}", e)))
    }
    
    /// 验证他人签名
    pub fn verify_with_key(public_key: &VerifyingKey, data: &[u8], signature: &Signature) -> Result<()> {
        public_key.verify(data, signature)
            .map_err(|e| CisError::crypto(format!("Signature verification failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_generation() {
        let keys = NodeKeyPair::generate();
        let data = b"test message";
        let sig = keys.sign(data);
        assert!(keys.verify(data, &sig).is_ok());
    }
    
    #[test]
    fn test_mnemonic_derivation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let keys1 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();
        let keys2 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();
        
        // 相同助记词应生成相同密钥
        assert_eq!(keys1.ed25519_public().as_bytes(), keys2.ed25519_public().as_bytes());
    }
}
