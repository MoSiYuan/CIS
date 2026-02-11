//! P2P 密钥管理测试
//!
//! 测试 NodeKeyPair 的密钥生成、签名验证和助记词派生功能。

use super::keys::NodeKeyPair;
use ed25519_dalek::Signature;

/// 测试密钥对生成
#[test]
fn test_key_generation_basic() {
    let keys = NodeKeyPair::generate();
    let data = b"test message";
    let sig = keys.sign(data);
    assert!(keys.verify(data, &sig).is_ok());
}

/// 测试相同种子生成相同密钥对
#[test]
fn test_key_generation_deterministic() {
    let seed = [0x42u8; 64];
    let keys1 = NodeKeyPair::from_seed(&seed).unwrap();
    let keys2 = NodeKeyPair::from_seed(&seed).unwrap();
    
    assert_eq!(
        keys1.ed25519_public().as_bytes(),
        keys2.ed25519_public().as_bytes()
    );
}

/// 测试不同种子生成不同密钥对
#[test]
fn test_key_generation_different_seeds() {
    let seed1 = [0x01u8; 64];
    let seed2 = [0x02u8; 64];
    let keys1 = NodeKeyPair::from_seed(&seed1).unwrap();
    let keys2 = NodeKeyPair::from_seed(&seed2).unwrap();
    
    assert_ne!(
        keys1.ed25519_public().as_bytes(),
        keys2.ed25519_public().as_bytes()
    );
}

/// 测试无效种子长度（太短）
#[test]
fn test_key_generation_invalid_seed_too_short() {
    let short_seed = [0x42u8; 32]; // 只有 32 字节，需要 64 字节
    let result = NodeKeyPair::from_seed(&short_seed);
    assert!(result.is_err());
}

/// 测试签名和验证成功
#[test]
fn test_sign_and_verify_success() {
    let keys = NodeKeyPair::generate();
    let message = b"Hello, CIS!";
    
    let signature = keys.sign(message);
    let result = keys.verify(message, &signature);
    
    assert!(result.is_ok());
}

/// 测试签名验证失败（篡改消息）
#[test]
fn test_sign_and_verify_tampered_message() {
    let keys = NodeKeyPair::generate();
    let message = b"Hello, CIS!";
    let tampered_message = b"Hello, CIS?";
    
    let signature = keys.sign(message);
    let result = keys.verify(tampered_message, &signature);
    
    assert!(result.is_err());
}

/// 测试签名验证失败（错误签名）
#[test]
fn test_verify_invalid_signature() {
    let keys = NodeKeyPair::generate();
    let message = b"Hello, CIS!";
    
    // 创建一个无效签名
    let invalid_sig = Signature::from_bytes(&[0u8; 64]);
    let result = keys.verify(message, &invalid_sig);
    
    assert!(result.is_err());
}

/// 测试助记词派生密钥
#[test]
fn test_mnemonic_derivation_basic() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let keys1 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();
    let keys2 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();
    
    // 相同助记词应生成相同密钥
    assert_eq!(
        keys1.ed25519_public().as_bytes(),
        keys2.ed25519_public().as_bytes()
    );
}

/// 测试不同助记词生成不同密钥
#[test]
fn test_mnemonic_different_phrases() {
    let mnemonic1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mnemonic2 = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
    
    let keys1 = NodeKeyPair::from_mnemonic(mnemonic1).unwrap();
    let keys2 = NodeKeyPair::from_mnemonic(mnemonic2).unwrap();
    
    assert_ne!(
        keys1.ed25519_public().as_bytes(),
        keys2.ed25519_public().as_bytes()
    );
}

/// 测试无效助记词
#[test]
fn test_mnemonic_invalid() {
    let invalid_mnemonic = "this is not a valid mnemonic phrase";
    let result = NodeKeyPair::from_mnemonic(invalid_mnemonic);
    assert!(result.is_err());
}

/// 测试 X25519 公钥派生
#[test]
fn test_x25519_public_key_derivation() {
    let keys = NodeKeyPair::generate();
    let x25519_pub = keys.x25519_public();
    
    // X25519 公钥应该是 32 字节
    let pubkey_bytes = x25519_pub.as_bytes();
    assert_eq!(pubkey_bytes.len(), 32);
}

/// 测试外部密钥验证
#[test]
fn test_verify_with_external_key() {
    let keys = NodeKeyPair::generate();
    let message = b"Test message";
    let signature = keys.sign(message);
    
    // 使用公钥验证签名
    let result = NodeKeyPair::verify_with_key(
        keys.ed25519_public(),
        message,
        &signature
    );
    
    assert!(result.is_ok());
}

/// 测试外部密钥验证失败
#[test]
fn test_verify_with_wrong_key() {
    let keys1 = NodeKeyPair::generate();
    let keys2 = NodeKeyPair::generate();
    let message = b"Test message";
    let signature = keys1.sign(message);
    
    // 使用错误的公钥验证
    let result = NodeKeyPair::verify_with_key(
        keys2.ed25519_public(),
        message,
        &signature
    );
    
    assert!(result.is_err());
}

/// 测试批量签名验证
#[test]
fn test_batch_sign_verify() {
    let keys = NodeKeyPair::generate();
    let messages: Vec<Vec<u8>> = (0..100)
        .map(|i| format!("message {}", i).into_bytes())
        .collect();
    
    for msg in &messages {
        let sig = keys.sign(msg);
        assert!(keys.verify(msg, &sig).is_ok());
    }
}

/// 测试空消息签名
#[test]
fn test_sign_empty_message() {
    let keys = NodeKeyPair::generate();
    let message = b"";
    
    let signature = keys.sign(message);
    let result = keys.verify(message, &signature);
    
    assert!(result.is_ok());
}

/// 测试大消息签名
#[test]
fn test_sign_large_message() {
    let keys = NodeKeyPair::generate();
    let message = vec![0x42u8; 10000];
    
    let signature = keys.sign(&message);
    let result = keys.verify(&message, &signature);
    
    assert!(result.is_ok());
}
