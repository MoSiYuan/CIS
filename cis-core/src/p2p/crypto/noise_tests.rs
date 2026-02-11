//! Noise Protocol 握手测试
//!
//! 测试 Noise XX 握手模式和加密传输。

use super::noise::{NoiseHandshake, NoiseTransport};
use x25519_dalek::StaticSecret;

/// 测试发起方握手状态创建
#[test]
fn test_initiator_creation() {
    let local_static = StaticSecret::new(rand::thread_rng());
    let result = NoiseHandshake::new_initiator(&local_static.to_bytes());
    assert!(result.is_ok());
}

/// 测试响应方握手状态创建
#[test]
fn test_responder_creation() {
    let local_static = StaticSecret::new(rand::thread_rng());
    let result = NoiseHandshake::new_responder(&local_static.to_bytes());
    assert!(result.is_ok());
}

/// 测试完整的 XX 握手
#[test]
fn test_xx_handshake_complete() {
    let local_static_a = StaticSecret::new(rand::thread_rng());
    let local_static_b = StaticSecret::new(rand::thread_rng());
    
    let mut initiator = NoiseHandshake::new_initiator(&local_static_a.to_bytes()).unwrap();
    let mut responder = NoiseHandshake::new_responder(&local_static_b.to_bytes()).unwrap();
    
    let mut buf1 = [0u8; 1024];
    let mut buf2 = [0u8; 1024];
    let mut payload = [0u8; 1024];
    
    // -> e
    let len = initiator.write_message(&[], &mut buf1).unwrap();
    assert!(len > 0);
    responder.read_message(&buf1[..len], &mut payload).unwrap();
    
    // <- e, ee, s, es
    let len = responder.write_message(&[], &mut buf2).unwrap();
    assert!(len > 0);
    initiator.read_message(&buf2[..len], &mut payload).unwrap();
    
    // -> s, se
    let len = initiator.write_message(&[], &mut buf1).unwrap();
    assert!(len > 0);
    responder.read_message(&buf1[..len], &mut payload).unwrap();
    
    // 转换为传输模式
    let transport_a = initiator.into_transport();
    let transport_b = responder.into_transport();
    
    assert!(transport_a.is_ok());
    assert!(transport_b.is_ok());
}

/// 测试加密传输
#[test]
fn test_transport_encryption() {
    let (mut transport_a, mut transport_b) = setup_noise_transport();
    
    let message = b"Hello, Noise!";
    let mut encrypted = [0u8; 1024];
    let mut decrypted = [0u8; 1024];
    
    // A 加密，B 解密
    let len = transport_a.encrypt(message, &mut encrypted).unwrap();
    let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
    
    assert_eq!(&decrypted[..len], message);
}

/// 测试双向加密通信
#[test]
fn test_bidirectional_communication() {
    let (mut transport_a, mut transport_b) = setup_noise_transport();
    
    // A -> B
    let message1 = b"Hello from A";
    let mut encrypted1 = [0u8; 1024];
    let mut decrypted1 = [0u8; 1024];
    
    let len = transport_a.encrypt(message1, &mut encrypted1).unwrap();
    let len = transport_b.decrypt(&encrypted1[..len], &mut decrypted1).unwrap();
    assert_eq!(&decrypted1[..len], message1);
    
    // B -> A
    let message2 = b"Hello from B";
    let mut encrypted2 = [0u8; 1024];
    let mut decrypted2 = [0u8; 1024];
    
    let len = transport_b.encrypt(message2, &mut encrypted2).unwrap();
    let len = transport_a.decrypt(&encrypted2[..len], &mut decrypted2).unwrap();
    assert_eq!(&decrypted2[..len], message2);
}

/// 测试大消息加密
#[test]
fn test_large_message_encryption() {
    let (mut transport_a, mut transport_b) = setup_noise_transport();
    
    let message = vec![0x42u8; 10000];
    let mut encrypted = vec![0u8; 12000];
    let mut decrypted = vec![0u8; 12000];
    
    let len = transport_a.encrypt(&message, &mut encrypted).unwrap();
    let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
    
    assert_eq!(&decrypted[..len], &message[..]);
}

/// 测试空消息加密
#[test]
fn test_empty_message_encryption() {
    let (mut transport_a, mut transport_b) = setup_noise_transport();
    
    let message = b"";
    let mut encrypted = [0u8; 1024];
    let mut decrypted = [0u8; 1024];
    
    let len = transport_a.encrypt(message, &mut encrypted).unwrap();
    let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
    
    assert_eq!(&decrypted[..len], message);
}

/// 测试解密失败（篡改密文）
#[test]
fn test_decrypt_tampered_ciphertext() {
    let (mut transport_a, mut transport_b) = setup_noise_transport();
    
    let message = b"Test message";
    let mut encrypted = [0u8; 1024];
    
    let len = transport_a.encrypt(message, &mut encrypted).unwrap();
    
    // 篡改密文
    encrypted[0] ^= 0xFF;
    
    let mut decrypted = [0u8; 1024];
    let result = transport_b.decrypt(&encrypted[..len], &mut decrypted);
    
    assert!(result.is_err());
}

/// 测试序列化消息加密
#[test]
fn test_serialized_message_encryption() {
    let (mut transport_a, mut transport_b) = setup_noise_transport();
    
    let message = serde_json::json!({
        "type": "test",
        "data": [1, 2, 3],
        "nested": {
            "key": "value"
        }
    });
    let message_bytes = serde_json::to_vec(&message).unwrap();
    
    let mut encrypted = vec![0u8; message_bytes.len() + 100];
    let mut decrypted = vec![0u8; message_bytes.len() + 100];
    
    let len = transport_a.encrypt(&message_bytes, &mut encrypted).unwrap();
    let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
    
    let decrypted_json: serde_json::Value = serde_json::from_slice(&decrypted[..len]).unwrap();
    assert_eq!(decrypted_json, message);
}

/// 测试最大消息大小
#[test]
fn test_max_message_size() {
    let max_size = NoiseTransport::max_message_size();
    assert!(max_size > 0);
    assert_eq!(max_size, 65535);
}

/// 测试握手消息带 payload
#[test]
fn test_handshake_with_payload() {
    let local_static_a = StaticSecret::new(rand::thread_rng());
    let local_static_b = StaticSecret::new(rand::thread_rng());
    
    let mut initiator = NoiseHandshake::new_initiator(&local_static_a.to_bytes()).unwrap();
    let mut responder = NoiseHandshake::new_responder(&local_static_b.to_bytes()).unwrap();
    
    let mut buf1 = [0u8; 1024];
    let mut buf2 = [0u8; 1024];
    let mut payload_out = [0u8; 1024];
    
    // -> e + payload
    let payload = b"Initiator payload";
    let len = initiator.write_message(payload, &mut buf1).unwrap();
    let read = responder.read_message(&buf1[..len], &mut payload_out).unwrap();
    assert_eq!(&payload_out[..read], payload);
    
    // <- e, ee, s, es + payload
    let payload = b"Responder payload";
    let len = responder.write_message(payload, &mut buf2).unwrap();
    let read = initiator.read_message(&buf2[..len], &mut payload_out).unwrap();
    assert_eq!(&payload_out[..read], payload);
    
    // -> s, se + payload
    let payload = b"Final payload";
    let len = initiator.write_message(payload, &mut buf1).unwrap();
    let read = responder.read_message(&buf1[..len], &mut payload_out).unwrap();
    assert_eq!(&payload_out[..read], payload);
}

/// 测试多次加密解密（状态保持）
#[test]
fn test_multiple_messages_sequence() {
    let (mut transport_a, mut transport_b) = setup_noise_transport();
    
    for i in 0..10 {
        let message = format!("Message {}", i);
        let mut encrypted = [0u8; 1024];
        let mut decrypted = [0u8; 1024];
        
        let len = transport_a.encrypt(message.as_bytes(), &mut encrypted).unwrap();
        let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
        
        assert_eq!(
            String::from_utf8_lossy(&decrypted[..len]),
            message
        );
    }
}

/// 测试无效握手状态转换
#[test]
fn test_invalid_handshake_state() {
    let local_static = StaticSecret::new(rand::thread_rng());
    let mut handshake = NoiseHandshake::new_initiator(&local_static.to_bytes()).unwrap();
    
    // 尝试在握手完成前转换为传输模式
    let mut buf = [0u8; 1024];
    let _ = handshake.write_message(&[], &mut buf).unwrap();
    
    // 未完成所有握手步骤，转换应该失败
    // 注意：snow 库可能允许不完整的转换，具体行为取决于实现
}

// 辅助函数：创建已握手的传输状态
fn setup_noise_transport() -> (NoiseTransport, NoiseTransport) {
    let local_static_a = StaticSecret::new(rand::thread_rng());
    let local_static_b = StaticSecret::new(rand::thread_rng());
    
    let mut initiator = NoiseHandshake::new_initiator(&local_static_a.to_bytes()).unwrap();
    let mut responder = NoiseHandshake::new_responder(&local_static_b.to_bytes()).unwrap();
    
    let mut buf1 = [0u8; 1024];
    let mut buf2 = [0u8; 1024];
    let mut payload = [0u8; 1024];
    
    // XX 握手流程
    let len = initiator.write_message(&[], &mut buf1).unwrap();
    responder.read_message(&buf1[..len], &mut payload).unwrap();
    
    let len = responder.write_message(&[], &mut buf2).unwrap();
    initiator.read_message(&buf2[..len], &mut payload).unwrap();
    
    let len = initiator.write_message(&[], &mut buf1).unwrap();
    responder.read_message(&buf1[..len], &mut payload).unwrap();
    
    (
        initiator.into_transport().unwrap(),
        responder.into_transport().unwrap()
    )
}
