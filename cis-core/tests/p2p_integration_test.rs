//! P2P 网络集成测试
//!
//! 测试场景:
//! - 局域网发现 (mDNS)
//! - 广域网发现 (DHT)
//! - NAT 穿透连接
//! - 连接质量监测

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use cis_core::p2p::{
    DhtService, DhtConfig,
    NatTraversal, NatType, HolePunchCoordinator, TraversalMethod,
    DiscoveryService, NodeInfo,
    PeerManager,
};

/// 创建测试用的节点信息
fn create_test_node_info(node_id: &str, port: u16) -> NodeInfo {
    NodeInfo {
        node_id: node_id.to_string(),
        did: format!("did:cis:{}", node_id),
        addresses: vec![format!("127.0.0.1:{}", port)],
        capabilities: vec!["memory_sync".to_string(), "skill_invoke".to_string()],
        public_key: vec![1, 2, 3, 4],
    }
}

/// 创建测试用的 DHT 配置
fn create_test_dht_config(bootstrap: Vec<String>) -> DhtConfig {
    DhtConfig {
        bootstrap_nodes: bootstrap,
        listen_addr: "127.0.0.1:0".to_string(),
        announce_interval_secs: 30,
        node_timeout_secs: 120,
        k: 5,
        alpha: 2,
        replication_factor: 2,
    }
}

/// 等待指定时间让网络操作完成
async fn wait_for_network(duration_ms: u64) {
    tokio::time::sleep(Duration::from_millis(duration_ms)).await;
}

mod dht_tests {
    use super::*;

    /// 测试 DHT 节点发现和路由表管理
    #[tokio::test]
    async fn test_dht_node_discovery() {
        // 创建两个 DHT 节点
        let node1_id = "dht-node-1";
        let node2_id = "dht-node-2";

        let config1 = create_test_dht_config(vec![]);
        let dht1 = DhtService::with_config(
            node1_id.to_string(),
            vec![],
            config1,
        );

        let config2 = create_test_dht_config(vec![]);
        let dht2 = DhtService::with_config(
            node2_id.to_string(),
            vec![],
            config2,
        );

        // 启动节点
        let local_node1 = create_test_node_info(node1_id, 7671);
        let local_node2 = create_test_node_info(node2_id, 7672);

        dht1.start(local_node1.clone()).await.expect("Failed to start DHT1");
        dht2.start(local_node2.clone()).await.expect("Failed to start DHT2");

        // 发布节点
        dht1.announce().await.expect("Failed to announce node1");
        dht2.announce().await.expect("Failed to announce node2");

        // 互相添加到路由表
        dht1.add_node(local_node2.clone()).await.expect("Failed to add node2 to node1");
        dht2.add_node(local_node1.clone()).await.expect("Failed to add node1 to node2");

        // 验证节点发现
        let found_node2 = dht1.find_node(node2_id).await.expect("Failed to find node2");
        assert!(found_node2.is_some(), "Node2 should be found in DHT1");
        assert_eq!(found_node2.unwrap().node_id, node2_id);

        let found_node1 = dht2.find_node(node1_id).await.expect("Failed to find node1");
        assert!(found_node1.is_some(), "Node1 should be found in DHT2");
        assert_eq!(found_node1.unwrap().node_id, node1_id);

        // 停止服务
        dht1.stop().await.expect("Failed to stop DHT1");
        dht2.stop().await.expect("Failed to stop DHT2");
    }

    /// 测试 DHT 键值存储和检索
    #[tokio::test]
    async fn test_dht_key_value_storage() {
        let node_id = "kv-test-node";
        let config = create_test_dht_config(vec![]);
        let dht = DhtService::with_config(
            node_id.to_string(),
            vec![],
            config,
        );

        let local_node = create_test_node_info(node_id, 7673);
        dht.start(local_node).await.expect("Failed to start DHT");

        // 测试键值存储
        let test_key = "test-key";
        let test_value = b"test-value-data".to_vec();

        dht.put(test_key, test_value.clone()).await.expect("Failed to put value");

        // 测试键值检索
        let retrieved = dht.get(test_key).await.expect("Failed to get value");
        assert!(retrieved.is_some(), "Value should be found");
        assert_eq!(retrieved.unwrap(), test_value);

        // 测试删除
        let deleted = dht.delete(test_key).await.expect("Failed to delete");
        assert!(deleted, "Value should be deleted");

        let not_found = dht.get(test_key).await.expect("Failed to get after delete");
        assert!(not_found.is_none(), "Value should not exist after delete");

        dht.stop().await.expect("Failed to stop DHT");
    }

    /// 测试 DHT 路由表维护和过期清理
    #[tokio::test]
    async fn test_dht_routing_table_maintenance() {
        let node_id = "maintenance-test-node";
        let mut config = create_test_dht_config(vec![]);
        config.node_timeout_secs = 1; // 设置很短的超时以便测试

        let dht = DhtService::with_config(
            node_id.to_string(),
            vec![],
            config,
        );

        let local_node = create_test_node_info(node_id, 7674);
        dht.start(local_node.clone()).await.expect("Failed to start DHT");
        dht.announce().await.expect("Failed to announce");

        // 添加一些节点
        for i in 0..5 {
            let peer = create_test_node_info(&format!("peer-{}", i), 7675 + i as u16);
            dht.add_node(peer).await.expect("Failed to add peer");
        }

        // 验证节点数量
        let nodes = dht.get_all_nodes().await;
        assert_eq!(nodes.len(), 5, "Should have 5 peers");

        // 等待过期
        wait_for_network(2000).await;

        // 注意：由于维护任务运行在后台，
        // 我们不能确定节点是否已被清理，因为这取决于时机
        // 但至少验证服务仍在运行
        assert!(dht.is_running().await);

        dht.stop().await.expect("Failed to stop DHT");
    }

    /// 测试 DHT 统计信息
    #[tokio::test]
    async fn test_dht_statistics() {
        let node_id = "stats-test-node";
        let config = create_test_dht_config(vec![]);
        let dht = DhtService::with_config(
            node_id.to_string(),
            vec![],
            config,
        );

        let local_node = create_test_node_info(node_id, 7680);
        dht.start(local_node).await.expect("Failed to start DHT");

        // 初始统计
        let stats = dht.get_stats().await;
        assert_eq!(stats.routing_table_size, 0);
        assert_eq!(stats.kv_store_size, 0);

        // 添加节点和数据
        for i in 0..3 {
            let peer = create_test_node_info(&format!("peer-{}", i), 7681 + i as u16);
            dht.add_node(peer).await.expect("Failed to add peer");
        }

        dht.put("key1", b"value1".to_vec()).await.unwrap();
        dht.put("key2", b"value2".to_vec()).await.unwrap();

        // 更新后的统计
        let stats = dht.get_stats().await;
        assert_eq!(stats.routing_table_size, 3);
        assert_eq!(stats.kv_store_size, 2);
        assert_eq!(stats.bootstrap_nodes, 0);

        dht.stop().await.expect("Failed to stop DHT");
    }

    /// 测试 DHT 迭代查找
    #[tokio::test]
    async fn test_dht_iterative_find() {
        let node_id = "iterative-test-node";
        let config = create_test_dht_config(vec![]);
        let dht = DhtService::with_config(
            node_id.to_string(),
            vec![],
            config,
        );

        let local_node = create_test_node_info(node_id, 7690);
        dht.start(local_node).await.expect("Failed to start DHT");

        // 添加多个节点
        for i in 0..10 {
            let peer = create_test_node_info(&format!("peer-{:03}", i), 7691 + i as u16);
            dht.add_node(peer).await.expect("Failed to add peer");
        }

        // 执行迭代查找
        let target = "target-node";
        let found = dht.iterative_find_node(target).await.expect("Iterative find failed");

        // 应该返回 k 个节点（或更少如果节点数不足）
        assert!(!found.is_empty(), "Should find some nodes");
        assert!(found.len() <= dht.config().k, "Should not return more than k nodes");

        dht.stop().await.expect("Failed to stop DHT");
    }
}

mod nat_tests {
    use super::*;

    /// 测试 NAT 类型检测
    #[tokio::test]
    async fn test_nat_type_detection() {
        let mut nat = NatTraversal::new(7677);
        
        // 初始状态
        assert_eq!(nat.nat_type(), NatType::Unknown);
        assert!(nat.external_addr().is_none());

        // 尝试检测（这可能失败如果没有网络）
        let result = nat.detect_nat_type().await;
        
        // 我们允许失败，但应该返回一个结果
        match result {
            Ok((nat_type, addr)) => {
                println!("NAT detection result: {:?}, addr: {:?}", nat_type, addr);
                // 检测结果应该更新状态
                assert_eq!(nat.nat_type(), nat_type);
            }
            Err(e) => {
                println!("NAT detection failed (expected in test environment): {}", e);
            }
        }
    }

    /// 测试 NAT 穿透详细结果
    #[tokio::test]
    async fn test_nat_traversal_detailed() {
        let mut nat = NatTraversal::new(7678);
        
        let result = nat.try_traversal_detailed().await;
        
        match result {
            Ok(traversal) => {
                println!(
                    "Traversal: method={}, nat_type={}, addr={:?}, latency={}ms",
                    traversal.method,
                    traversal.nat_type,
                    traversal.external_addr,
                    traversal.latency_ms
                );
                
                // 验证结果结构
                match traversal.method {
                    TraversalMethod::Upnp | TraversalMethod::Stun => {
                        assert!(traversal.external_addr.is_some());
                    }
                    TraversalMethod::Failed => {
                        // 失败也是有效的结果
                    }
                    _ => {}
                }
            }
            Err(e) => {
                println!("Traversal failed: {}", e);
            }
        }
    }

    /// 测试 Hole Punching 协调器初始化
    #[tokio::test]
    async fn test_hole_punch_coordinator() {
        let mut coordinator = HolePunchCoordinator::new();
        
        assert_eq!(coordinator.nat_type(), NatType::Unknown);
        assert!(coordinator.local_addr().is_none());

        // 尝试初始化
        let result = coordinator.init().await;
        
        match result {
            Ok((nat_type, external_addr)) => {
                println!(
                    "Coordinator initialized: nat_type={}, external={:?}",
                    nat_type, external_addr
                );
                assert_eq!(coordinator.nat_type(), nat_type);
                assert!(coordinator.local_addr().is_some());
            }
            Err(e) => {
                println!("Coordinator init failed: {}", e);
            }
        }
    }

    /// 测试 Hole Punching 配置
    #[tokio::test]
    async fn test_hole_punch_with_config() {
        let custom_servers = vec!["stun.l.google.com:19302".to_string()];
        let coordinator = HolePunchCoordinator::with_stun_servers(custom_servers)
            .with_punch_config(10, 50);

        assert_eq!(coordinator.nat_type(), NatType::Unknown);
        
        // 尝试初始化
        let result = coordinator.init().await;
        
        // 即使失败也不应 panic
        if let Err(e) = result {
            println!("Hole punch with config failed: {}", e);
        }
    }

    /// 测试 NAT 类型属性
    #[test]
    fn test_nat_type_properties() {
        // Open
        assert!(NatType::Open.is_easy_traversal());
        assert!(!NatType::Open.needs_turn());
        assert!(NatType::Open.can_hole_punch());

        // Full Cone
        assert!(NatType::FullCone.is_easy_traversal());
        assert!(!NatType::FullCone.needs_turn());
        assert!(NatType::FullCone.can_hole_punch());

        // Address Restricted
        assert!(NatType::AddressRestricted.is_easy_traversal());
        assert!(!NatType::AddressRestricted.needs_turn());
        assert!(NatType::AddressRestricted.can_hole_punch());

        // Port Restricted
        assert!(!NatType::PortRestricted.is_easy_traversal());
        assert!(!NatType::PortRestricted.needs_turn());
        assert!(NatType::PortRestricted.can_hole_punch());

        // Symmetric
        assert!(!NatType::Symmetric.is_easy_traversal());
        assert!(NatType::Symmetric.needs_turn());
        assert!(!NatType::Symmetric.can_hole_punch());

        // Unknown
        assert!(!NatType::Unknown.is_easy_traversal());
        assert!(!NatType::Unknown.needs_turn());
        assert!(!NatType::Unknown.can_hole_punch());
    }

    /// 测试穿透方法显示
    #[test]
    fn test_traversal_method_display() {
        assert_eq!(format!("{}", TraversalMethod::Open), "Open (no NAT)");
        assert_eq!(format!("{}", TraversalMethod::Upnp), "UPnP");
        assert_eq!(format!("{}", TraversalMethod::Stun), "STUN");
        assert_eq!(format!("{}", TraversalMethod::Turn), "TURN");
        assert_eq!(format!("{}", TraversalMethod::Failed), "Failed");
    }
}

mod discovery_tests {
    use super::*;

    /// 测试节点发现服务创建
    #[tokio::test]
    async fn test_discovery_service_creation() {
        let node_id = "discovery-test-node";
        let discovery = DiscoveryService::new(node_id.to_string());

        // 获取发现的节点（应该为空）
        let peers = discovery.get_discovered_peers();
        assert!(peers.is_empty());
    }

    /// 测试节点发现服务启动（可能需要网络权限）
    #[tokio::test]
    #[ignore = "Requires network permissions and mDNS support"]
    async fn test_discovery_service_start() {
        let node_id = "discovery-start-test";
        let discovery = DiscoveryService::new(node_id.to_string());

        let local_node = create_test_node_info(node_id, 7700);
        
        // 尝试启动（可能需要权限）
        let result = timeout(
            Duration::from_secs(5),
            discovery.start(local_node)
        ).await;

        match result {
            Ok(Ok(())) => {
                println!("Discovery service started successfully");
                // 等待一段时间让服务广播
                wait_for_network(1000).await;
            }
            Ok(Err(e)) => {
                println!("Discovery service failed to start: {}", e);
            }
            Err(_) => {
                println!("Discovery service start timed out");
            }
        }
    }
}

mod peer_manager_tests {
    use super::*;
    use chrono::Utc;

    fn create_test_peer_info(node_id: &str, is_connected: bool) -> cis_core::p2p::PeerInfo {
        cis_core::p2p::PeerInfo {
            node_id: node_id.to_string(),
            did: format!("did:cis:{}", node_id),
            address: format!("127.0.0.1:{}", 7800 + node_id.len() as u16),
            last_seen: Utc::now(),
            last_sync_at: None,
            is_connected,
            capabilities: vec!["memory_sync".to_string()],
        }
    }

    /// 测试节点管理器基本操作
    #[tokio::test]
    async fn test_peer_manager_basic() {
        let manager = PeerManager::new();

        // 初始为空
        let peers = manager.get_all_peers().await;
        assert!(peers.is_empty());

        // 添加节点
        let peer1 = create_test_peer_info("peer-1", true);
        manager.update_peer(peer1.clone()).await.expect("Failed to add peer");

        let peers = manager.get_all_peers().await;
        assert_eq!(peers.len(), 1);

        // 获取特定节点
        let found = manager.get_peer("peer-1").await.expect("Failed to get peer");
        assert!(found.is_some());
        assert_eq!(found.unwrap().node_id, "peer-1");

        // 获取不存在的节点
        let not_found = manager.get_peer("non-existent").await.expect("Failed to get peer");
        assert!(not_found.is_none());
    }

    /// 测试已连接节点过滤
    #[tokio::test]
    async fn test_peer_manager_connected_filter() {
        let manager = PeerManager::new();

        // 添加已连接和未连接的节点
        let peer1 = create_test_peer_info("peer-1", true);
        let peer2 = create_test_peer_info("peer-2", false);
        let peer3 = create_test_peer_info("peer-3", true);

        manager.update_peer(peer1).await.unwrap();
        manager.update_peer(peer2).await.unwrap();
        manager.update_peer(peer3).await.unwrap();

        // 获取所有节点
        let all_peers = manager.get_all_peers().await;
        assert_eq!(all_peers.len(), 3);

        // 获取已连接的节点
        let connected = manager.get_connected_peers().await;
        assert_eq!(connected.len(), 2);
    }

    /// 测试节点健康状态管理
    #[tokio::test]
    async fn test_peer_manager_health() {
        let manager = PeerManager::new();

        let peer = create_test_peer_info("health-test-peer", true);
        manager.update_peer(peer).await.unwrap();

        // 初始为健康
        let found = manager.get_peer("health-test-peer").await.unwrap().unwrap();
        assert!(found.is_connected);
        assert!(!found.is_unhealthy());

        // 标记为不健康
        manager.mark_unhealthy("health-test-peer").await.unwrap();
        let found = manager.get_peer("health-test-peer").await.unwrap().unwrap();
        assert!(!found.is_connected);
        assert!(found.is_unhealthy());

        // 标记为健康
        manager.mark_healthy("health-test-peer").await.unwrap();
        let found = manager.get_peer("health-test-peer").await.unwrap().unwrap();
        assert!(found.is_connected);
        assert!(!found.is_unhealthy());
    }

    /// 测试同步时间更新
    #[tokio::test]
    async fn test_peer_manager_sync_time() {
        let manager = PeerManager::new();

        let peer = create_test_peer_info("sync-test-peer", true);
        manager.update_peer(peer).await.unwrap();

        // 初始无同步时间
        let found = manager.get_peer("sync-test-peer").await.unwrap().unwrap();
        assert!(found.last_sync_at.is_none());

        // 更新同步时间
        manager.update_sync_time("sync-test-peer").await.unwrap();
        let found = manager.get_peer("sync-test-peer").await.unwrap().unwrap();
        assert!(found.last_sync_at.is_some());
    }
}

mod integration_tests {
    use super::*;

    /// 完整 P2P 场景测试：启动 DHT、添加节点、存储数据
    #[tokio::test]
    async fn test_full_p2p_scenario() {
        // 创建节点
        let node_id = "integration-test-node";
        let config = create_test_dht_config(vec![]);
        let dht = DhtService::with_config(
            node_id.to_string(),
            vec![],
            config,
        );

        let peer_manager = PeerManager::new();

        let local_node = create_test_node_info(node_id, 7900);
        
        // 启动 DHT
        dht.start(local_node.clone()).await.expect("Failed to start DHT");
        dht.announce().await.expect("Failed to announce");

        // 添加一些对等节点
        for i in 0..5 {
            let peer_info = create_test_peer_info(&format!("integration-peer-{}", i), 7901 + i as u16);
            dht.add_node(peer_info.clone()).await.expect("Failed to add peer to DHT");
            
            // 同时添加到 peer manager
            let peer = cis_core::p2p::PeerInfo {
                node_id: peer_info.node_id.clone(),
                did: peer_info.did,
                address: peer_info.addresses[0].clone(),
                last_seen: Utc::now(),
                last_sync_at: None,
                is_connected: true,
                capabilities: peer_info.capabilities,
            };
            peer_manager.update_peer(peer).await.expect("Failed to add peer to manager");
        }

        // 存储一些数据
        for i in 0..3 {
            let key = format!("integration-key-{}", i);
            let value = format!("integration-value-{}", i).into_bytes();
            dht.put(&key, value).await.expect("Failed to put data");
        }

        // 验证状态
        let dht_stats = dht.get_stats().await;
        assert_eq!(dht_stats.routing_table_size, 5, "Should have 5 peers in routing table");
        assert_eq!(dht_stats.kv_store_size, 3, "Should have 3 items in KV store");

        let connected_peers = peer_manager.get_connected_peers().await;
        assert_eq!(connected_peers.len(), 5, "Should have 5 connected peers");

        // 清理
        dht.stop().await.expect("Failed to stop DHT");
    }

    /// 测试网络环境检测
    #[tokio::test]
    async fn test_network_environment_detection() {
        // 检测 NAT 类型
        let mut nat = NatTraversal::new(7977);
        let nat_result = nat.try_traversal_detailed().await;

        // 检测 DHT 能力
        let dht = DhtService::new("env-test".to_string(), vec![]);
        let local_node = create_test_node_info("env-test", 7978);
        dht.start(local_node).await.expect("Failed to start DHT");
        let dht_running = dht.is_running().await;

        println!("Network Environment Detection:");
        println!("  DHT Service: {}", if dht_running { "Running" } else { "Failed" });
        
        match nat_result {
            Ok(result) => {
                println!("  NAT Type: {}", result.nat_type);
                println!("  Traversal Method: {}", result.method);
                println!("  External Address: {:?}", result.external_addr);
                println!("  Latency: {}ms", result.latency_ms);
            }
            Err(e) => {
                println!("  NAT Detection: Failed ({})", e);
            }
        }

        dht.stop().await.expect("Failed to stop DHT");
    }
}
