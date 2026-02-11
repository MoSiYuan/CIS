//! P2P 集成测试
//!
//! 测试 P2P 网络的完整功能流程。

use cis_core::p2p::{ConnectionManager, Message, ConnectionState};
use cis_core::p2p::crypto::keys::NodeKeyPair;
use cis_core::p2p::crypto::noise::{NoiseHandshake, NoiseTransport};
use tokio::sync::mpsc;
use std::time::Duration;
use x25519_dalek::StaticSecret;

/// 测试完整的密钥生成和签名流程
#[test]
fn test_full_key_lifecycle() {
    // 生成密钥对
    let keys = NodeKeyPair::generate();
    
    // 签名消息
    let message = b"Important message";
    let signature = keys.sign(message);
    
    // 验证签名
    assert!(keys.verify(message, &signature).is_ok());
    
    // 使用外部密钥验证
    assert!(NodeKeyPair::verify_with_key(
        keys.ed25519_public(),
        message,
        &signature
    ).is_ok());
}

/// 测试助记词派生和恢复
#[test]
fn test_mnemonic_recovery() {
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    
    // 第一次派生
    let keys1 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();
    
    // 第二次派生（应该得到相同的密钥）
    let keys2 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();
    
    // 验证密钥相同
    assert_eq!(
        keys1.ed25519_public().as_bytes(),
        keys2.ed25519_public().as_bytes()
    );
    
    // 验证 X25519 密钥也相同
    assert_eq!(
        keys1.x25519_public().as_bytes(),
        keys2.x25519_public().as_bytes()
    );
}

/// 测试 Noise 完整握手和加密通信
#[test]
fn test_noise_full_communication() {
    // 创建静态密钥
    let static_a = StaticSecret::new(rand::thread_rng());
    let static_b = StaticSecret::new(rand::thread_rng());
    
    // 创建握手状态
    let mut initiator = NoiseHandshake::new_initiator(&static_a.to_bytes()).unwrap();
    let mut responder = NoiseHandshake::new_responder(&static_b.to_bytes()).unwrap();
    
    let mut buf1 = [0u8; 1024];
    let mut buf2 = [0u8; 1024];
    let mut payload_buf = [0u8; 1024];
    
    // XX 握手流程
    // -> e
    let len = initiator.write_message(&[], &mut buf1).unwrap();
    responder.read_message(&buf1[..len], &mut payload_buf).unwrap();
    
    // <- e, ee, s, es
    let len = responder.write_message(&[], &mut buf2).unwrap();
    initiator.read_message(&buf2[..len], &mut payload_buf).unwrap();
    
    // -> s, se
    let len = initiator.write_message(&[], &mut buf1).unwrap();
    responder.read_message(&buf1[..len], &mut payload_buf).unwrap();
    
    // 转换为传输模式
    let mut transport_a = initiator.into_transport().unwrap();
    let mut transport_b = responder.into_transport().unwrap();
    
    // 双向通信测试
    // A -> B
    let msg1 = b"Hello from initiator";
    let mut enc1 = [0u8; 1024];
    let mut dec1 = [0u8; 1024];
    let len = transport_a.encrypt(msg1, &mut enc1).unwrap();
    let len = transport_b.decrypt(&enc1[..len], &mut dec1).unwrap();
    assert_eq!(&dec1[..len], msg1);
    
    // B -> A
    let msg2 = b"Hello from responder";
    let mut enc2 = [0u8; 1024];
    let mut dec2 = [0u8; 1024];
    let len = transport_b.encrypt(msg2, &mut enc2).unwrap();
    let len = transport_a.decrypt(&enc2[..len], &mut dec2).unwrap();
    assert_eq!(&dec2[..len], msg2);
}

/// 测试连接管理器完整生命周期
#[tokio::test]
async fn test_connection_manager_lifecycle() {
    let manager = ConnectionManager::new();
    
    // 添加连接
    let (tx1, mut rx1) = mpsc::channel(100);
    let handle1 = manager.add_connection("peer-1".to_string(), tx1).await;
    
    // 验证连接状态
    assert_eq!(handle1.node_id, "peer-1");
    
    // 发送消息
    let msg = Message::Ping;
    manager.send_to("peer-1", msg.clone()).await.unwrap();
    
    // 验证接收
    let received = rx1.recv().await.unwrap();
    assert!(matches!(received, Message::Ping));
    
    // 添加更多连接
    let (tx2, mut rx2) = mpsc::channel(100);
    manager.add_connection("peer-2".to_string(), tx2).await;
    
    // 广播消息
    let results = manager.broadcast(Message::Pong).await;
    assert_eq!(results.len(), 2);
    
    // 验证都收到
    assert!(matches!(rx1.recv().await.unwrap(), Message::Pong));
    assert!(matches!(rx2.recv().await.unwrap(), Message::Pong));
    
    // 移除连接
    manager.remove_connection("peer-1").await.unwrap();
    
    // 验证连接列表
    let connections = manager.list_connections().await;
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].0, "peer-2");
}

/// 测试连接超时检测
#[tokio::test]
async fn test_connection_timeout_detection() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    let handle = manager.add_connection("peer-1".to_string(), tx).await;
    
    // 初始状态
    let state = handle.state.read().await.clone();
    assert_eq!(state, ConnectionState::Connected);
    
    // 模拟一段时间不活跃（在实际测试中，这需要等待超时）
    // 这里我们只是验证结构正确
    let last_active = *handle.last_active.read().await;
    assert!(last_active.elapsed() < Duration::from_secs(1));
}

/// 测试并发连接操作
#[tokio::test]
async fn test_concurrent_connections() {
    let manager = Arc::new(ConnectionManager::new());
    let mut handles = vec![];
    
    // 并发添加 50 个连接
    for i in 0..50 {
        let manager = manager.clone();
        let handle = tokio::spawn(async move {
            let (tx, _rx) = mpsc::channel(100);
            manager.add_connection(format!("peer-{}", i), tx).await
        });
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await.unwrap();
    }
    
    // 验证连接数
    let connections = manager.list_connections().await;
    assert_eq!(connections.len(), 50);
}

/// 测试密钥对签名验证链
#[test]
fn test_signature_verification_chain() {
    let keys = NodeKeyPair::generate();
    let messages: Vec<Vec<u8>> = (0..100)
        .map(|i| format!("Message {}", i).into_bytes())
        .collect();
    
    let mut signatures = Vec::new();
    
    // 签名所有消息
    for msg in &messages {
        signatures.push(keys.sign(msg));
    }
    
    // 验证所有签名
    for (i, (msg, sig)) in messages.iter().zip(signatures.iter()).enumerate() {
        assert!(
            keys.verify(msg, sig).is_ok(),
            "Failed to verify signature for message {}",
            i
        );
    }
}

/// 测试错误路径：无效签名
#[test]
fn test_invalid_signature_rejection() {
    let keys = NodeKeyPair::generate();
    let message = b"Original message";
    let signature = keys.sign(message);
    
    // 篡改消息
    let tampered = b"Tampered message";
    assert!(keys.verify(tampered, &signature).is_err());
    
    // 篡改签名（在实际中很难构造有效但错误的签名，这里测试异常路径）
    // 使用不同的密钥验证
    let other_keys = NodeKeyPair::generate();
    assert!(other_keys.verify(message, &signature).is_err());
}

/// 测试错误路径：连接到不存在节点
#[tokio::test]
async fn test_send_to_nonexistent_peer() {
    let manager = ConnectionManager::new();
    
    let result = manager.send_to("nonexistent", Message::Ping).await;
    assert!(result.is_err());
}

/// 测试错误路径：重复注册
#[tokio::test]
async fn test_duplicate_registration() {
    let manager = ConnectionManager::new();
    let (tx, _rx) = mpsc::channel(100);
    
    manager.add_connection("peer-1".to_string(), tx.clone()).await;
    
    // 重复添加（应该覆盖）
    manager.add_connection("peer-1".to_string(), tx).await;
    
    let connections = manager.list_connections().await;
    assert_eq!(connections.len(), 1);
}

/// 测试大规模消息加密
#[test]
fn test_large_message_encryption() {
    let (mut transport_a, mut transport_b) = setup_noise_pair();
    
    // 1MB 消息
    let message = vec![0x42u8; 1024 * 1024];
    let mut encrypted = vec![0u8; message.len() + 1000];
    let mut decrypted = vec![0u8; message.len() + 1000];
    
    let len = transport_a.encrypt(&message, &mut encrypted).unwrap();
    let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
    
    assert_eq!(&decrypted[..len], &message[..]);
}

/// 测试序列化数据加密
#[test]
fn test_json_message_encryption() {
    let (mut transport_a, mut transport_b) = setup_noise_pair();
    
    let data = serde_json::json!({
        "type": "test",
        "nested": {
            "array": [1, 2, 3],
            "string": "value"
        },
        "number": 42
    });
    
    let message = serde_json::to_vec(&data).unwrap();
    let mut encrypted = vec![0u8; message.len() + 1000];
    let mut decrypted = vec![0u8; message.len() + 1000];
    
    let len = transport_a.encrypt(&message, &mut encrypted).unwrap();
    let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
    
    let decoded: serde_json::Value = serde_json::from_slice(&decrypted[..len]).unwrap();
    assert_eq!(decoded, data);
}

/// 辅助函数：创建 Noise 传输对
fn setup_noise_pair() -> (NoiseTransport, NoiseTransport) {
    let static_a = StaticSecret::new(rand::thread_rng());
    let static_b = StaticSecret::new(rand::thread_rng());
    
    let mut initiator = NoiseHandshake::new_initiator(&static_a.to_bytes()).unwrap();
    let mut responder = NoiseHandshake::new_responder(&static_b.to_bytes()).unwrap();
    
    let mut buf1 = [0u8; 1024];
    let mut buf2 = [0u8; 1024];
    let mut payload = [0u8; 1024];
    
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
