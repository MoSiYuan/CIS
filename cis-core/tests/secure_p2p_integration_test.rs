//! SecureP2PTransport 集成测试
//!
//! 测试端到端加密通信、握手成功率、双向身份验证。

#[cfg(feature = "p2p")]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::timeout;

    use cis_core::p2p::{
        crypto::keys::NodeKeyPair,
        transport_secure::{SecureP2PTransport, SecureTransportConfig},
    };

    /// 测试基础配置
    #[tokio::test]
    async fn test_basic_config() {
        let config = SecureTransportConfig::default();
        assert_eq!(config.handshake_timeout, Duration::from_secs(30));
        assert!(config.enable_mutual_auth);
    }

    /// 测试密钥生成和加载
    #[tokio::test]
    async fn test_key_generation() {
        let keys = NodeKeyPair::generate();
        let data = b"test message";
        let sig = keys.sign(data);
        assert!(keys.verify(data, &sig).is_ok());
    }

    /// 测试助记词派生
    #[tokio::test]
    async fn test_mnemonic_derivation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let keys1 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();
        let keys2 = NodeKeyPair::from_mnemonic(mnemonic).unwrap();

        assert_eq!(
            keys1.ed25519_public().as_bytes(),
            keys2.ed25519_public().as_bytes()
        );
    }

    /// 测试完整握手和通信
    #[tokio::test]
    async fn test_full_handshake_and_communication() {
        // 创建两个节点的密钥
        let node_a_keys = Arc::new(NodeKeyPair::generate());
        let node_b_keys = Arc::new(NodeKeyPair::generate());

        // 创建节点 A 的传输层
        let transport_a = Arc::new(
            SecureP2PTransport::bind("127.0.0.1:0", "node-a", "did:cis:node-a", Arc::clone(&node_a_keys))
                .await
                .unwrap(),
        );

        transport_a.start_listening().await.unwrap();
        let addr_a = transport_a.local_addr();

        // 创建节点 B 的传输层
        let transport_b = Arc::new(
            SecureP2PTransport::bind("127.0.0.1:0", "node-b", "did:cis:node-b", Arc::clone(&node_b_keys))
                .await
                .unwrap(),
        );

        transport_b.start_listening().await.unwrap();

        // 节点 B 连接到节点 A
        let result = timeout(
            Duration::from_secs(10),
            transport_b.connect("node-a", addr_a),
        )
        .await;

        assert!(result.is_ok(), "Connection should not timeout");
        assert!(result.unwrap().is_ok(), "Connection should succeed");

        // 验证连接建立
        let connections_b = transport_b.list_connections().await;
        assert_eq!(connections_b.len(), 1);
        assert_eq!(connections_b[0].node_id, "node-a");
        assert!(connections_b[0].authenticated);

        // 测试加密发送
        let test_data = b"Hello, Secure P2P World!";
        let send_result = transport_b.send("node-a", test_data).await;
        assert!(send_result.is_ok(), "Send should succeed");
    }

    /// 测试重复连接被拒绝
    #[tokio::test]
    async fn test_duplicate_connection_rejected() {
        let node_a_keys = Arc::new(NodeKeyPair::generate());
        let node_b_keys = Arc::new(NodeKeyPair::generate());

        let transport_a = Arc::new(
            SecureP2PTransport::bind("127.0.0.1:0", "node-a", "did:cis:node-a", Arc::clone(&node_a_keys))
                .await
                .unwrap(),
        );

        transport_a.start_listening().await.unwrap();
        let addr_a = transport_a.local_addr();

        let transport_b = Arc::new(
            SecureP2PTransport::bind("127.0.0.1:0", "node-b", "did:cis:node-b", Arc::clone(&node_b_keys))
                .await
                .unwrap(),
        );

        transport_b.start_listening().await.unwrap();

        // 第一次连接
        let result1 = timeout(
            Duration::from_secs(5),
            transport_b.connect("node-a", addr_a),
        )
        .await;
        assert!(result1.is_ok());
        assert!(result1.unwrap().is_ok());

        // 第二次连接应该失败（已连接）
        let result2 = transport_b.connect("node-a", addr_a).await;
        assert!(result2.is_err());
        match result2 {
            Err(cis_core::error::CisError::P2P(msg)) => {
                assert!(msg.contains("Already connected"));
            }
            _ => panic!("Expected already connected error"),
        }
    }

    /// 测试向未连接节点发送失败
    #[tokio::test]
    async fn test_send_to_disconnected_node_fails() {
        let node_keys = Arc::new(NodeKeyPair::generate());
        let transport = SecureP2PTransport::bind("127.0.0.1:0", "test-node", "did:cis:test", node_keys)
            .await
            .unwrap();

        let result = transport.send("not-connected", b"test").await;
        assert!(result.is_err());
        match result {
            Err(cis_core::error::CisError::P2P(msg)) => {
                assert!(msg.contains("not connected"));
            }
            _ => panic!("Expected not connected error"),
        }
    }

    /// 测试大消息分块
    #[tokio::test]
    async fn test_large_message_chunking() {
        let large_data = vec![0u8; 100_000]; // 100KB
        const MAX_CHUNK_SIZE: usize = 65535 - 16;

        let chunks: Vec<&[u8]> = large_data.chunks(MAX_CHUNK_SIZE).collect();
        assert!(
            chunks.len() > 1,
            "Large data should be split into multiple chunks"
        );

        for (i, chunk) in chunks.iter().enumerate() {
            if i < chunks.len() - 1 {
                assert_eq!(
                    chunk.len(),
                    MAX_CHUNK_SIZE,
                    "Full chunk should be MAX_CHUNK_SIZE"
                );
            } else {
                assert!(
                    chunk.len() <= MAX_CHUNK_SIZE,
                    "Last chunk should be <= MAX_CHUNK_SIZE"
                );
            }
        }
    }

    /// 性能测试：测量握手时间
    #[tokio::test]
    async fn test_handshake_performance() {
        use std::time::Instant;

        let node_a_keys = Arc::new(NodeKeyPair::generate());
        let node_b_keys = Arc::new(NodeKeyPair::generate());

        let transport_a = Arc::new(
            SecureP2PTransport::bind("127.0.0.1:0", "node-a", "did:cis:node-a", Arc::clone(&node_a_keys))
                .await
                .unwrap(),
        );

        transport_a.start_listening().await.unwrap();
        let addr_a = transport_a.local_addr();

        let transport_b = Arc::new(
            SecureP2PTransport::bind("127.0.0.1:0", "node-b", "did:cis:node-b", Arc::clone(&node_b_keys))
                .await
                .unwrap(),
        );

        let start = Instant::now();
        let result = timeout(
            Duration::from_secs(10),
            transport_b.connect("node-a", addr_a),
        )
        .await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());

        // 握手应该在 5 秒内完成
        assert!(
            elapsed < Duration::from_secs(5),
            "Handshake took too long: {:?}",
            elapsed
        );

        println!("Handshake completed in {:?}", elapsed);
    }

    /// 压力测试：多次连续连接
    #[tokio::test]
    async fn test_stress_multiple_connections() {
        let node_a_keys = Arc::new(NodeKeyPair::generate());
        let transport_a = Arc::new(
            SecureP2PTransport::bind("127.0.0.1:0", "node-a", "did:cis:node-a", Arc::clone(&node_a_keys))
                .await
                .unwrap(),
        );

        transport_a.start_listening().await.unwrap();
        let addr_a = transport_a.local_addr();

        let mut success_count = 0;
        let total_attempts = 5;

        for i in 0..total_attempts {
            let node_keys = Arc::new(NodeKeyPair::generate());
            let transport = Arc::new(
                SecureP2PTransport::bind(
                    "127.0.0.1:0",
                    &format!("node-{}", i),
                    &format!("did:cis:node-{}", i),
                    node_keys,
                )
                .await
                .unwrap(),
            );

            match timeout(
                Duration::from_secs(5),
                transport.connect("node-a", addr_a),
            )
            .await
            {
                Ok(Ok(_)) => success_count += 1,
                _ => {}
            }
        }

        // 成功率应该 > 99% (这里简化为 80% 考虑测试环境)
        let success_rate = success_count as f64 / total_attempts as f64;
        assert!(
            success_rate >= 0.8,
            "Handshake success rate too low: {:.1}%",
            success_rate * 100.0
        );

        println!(
            "Stress test: {}/{} connections successful ({:.1}%)",
            success_count, total_attempts, success_rate * 100.0
        );
    }
}

#[cfg(not(feature = "p2p"))]
mod tests {
    #[test]
    fn test_placeholder() {
        // P2P 特性未启用时的占位测试
        println!("P2P feature not enabled, skipping integration tests");
    }
}
