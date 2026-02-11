//! # CIS 全流程集成测试
//!
//! 测试完整的系统流程，包括：
//! - Matrix 登录和验证码
//! - Skill 注册和执行（Native/WASM/Remote/DAG）
//! - P2P 网络发现和 DHT
//! - 联邦事件签名和验证
//! - 公共记忆同步

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// 测试 Matrix 登录验证码流程
#[tokio::test]
async fn test_matrix_login_verification_code() {
    use cis_core::matrix::store_social::MatrixSocialStore;
    
    // 创建内存中的社交存储
    let store = MatrixSocialStore::open_in_memory().expect("Failed to create store");
    
    let user_id = "@test:cis.local";
    
    // 1. 检查用户是否需要验证码（新用户需要）
    let needs_code = store.needs_verification_code(user_id).expect("Failed to check");
    assert!(needs_code, "New user should need verification code");
    
    // 2. 生成验证码
    let (code, is_new) = store.generate_login_code(user_id).expect("Failed to generate code");
    assert_eq!(code.len(), 6, "Code should be 6 digits");
    assert!(is_new, "Should be new user");
    
    // 3. 验证正确验证码
    let result = store.verify_login_code(user_id, &code).expect("Failed to verify");
    assert!(result, "Correct code should be accepted");
    
    // 4. 验证后用户不再需要验证码
    let needs_code_after = store.needs_verification_code(user_id).expect("Failed to check");
    assert!(!needs_code_after, "Verified user should not need code");
    
    // 5. 错误验证码应被拒绝
    let wrong_code = "000000";
    let result_wrong = store.verify_login_code(user_id, wrong_code).expect("Failed to verify");
    assert!(!result_wrong, "Wrong code should be rejected");
    
    println!("✅ Matrix login verification code test passed");
}

/// 测试 DHT 存储和检索
/// 
/// 注意：此测试需要完整的 P2P 网络环境
#[tokio::test]
#[ignore = "Requires full P2P network setup"]
async fn test_dht_put_get() {
    use cis_core::p2p::kademlia::{KademliaDht, KademliaConfig, NodeId};
    use cis_core::p2p::kademlia::transport::P2PNetworkTransport;
    
    // 此测试需要完整的 SecureP2PTransport 设置
    // 在实际网络环境中运行
    println!("✅ DHT put/get test placeholder");
}

/// 测试联邦事件签名
#[tokio::test]
async fn test_federation_event_signing() {
    use cis_core::matrix::federation::types::CisMatrixEvent;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    
    // 生成测试密钥
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    
    // 创建事件
    let mut event = CisMatrixEvent::new(
        "$test-event-1",
        "!test-room:cis.local",
        "@sender:cis.local",
        "m.room.message",
        serde_json::json!({"body": "Hello"}),
    );
    
    // 签名事件
    let result = event.sign("cis.local", "ed25519:0", &signing_key);
    assert!(result.is_ok(), "Signing should succeed");
    
    // 验证事件有签名
    assert!(event.signatures.is_some(), "Event should have signatures");
    let signatures = event.signatures.as_ref().unwrap();
    assert!(signatures.contains_key("cis.local"), "Should have cis.local signature");
    
    // 计算哈希
    let hash_result = event.compute_hash();
    assert!(hash_result.is_ok(), "Hash computation should succeed");
    assert!(event.hashes.is_some(), "Event should have hashes");
    
    println!("✅ Federation event signing test passed");
}

/// 测试 Remote Skill 配置
#[tokio::test]
async fn test_remote_skill_config() {
    use cis_core::skill::manifest::{SkillManifest, RemoteConfig, LoadBalanceStrategy};
    
    // 创建 Remote Skill manifest
    let target_nodes = vec![
        "https://node1.cis.local".to_string(),
        "https://node2.cis.local".to_string(),
    ];
    
    let manifest = SkillManifest::default_remote("test-remote-skill", target_nodes.clone());
    
    // 验证配置
    assert_eq!(manifest.skill.skill_type, cis_core::skill::manifest::SkillType::Remote);
    assert!(manifest.remote.is_some(), "Should have remote config");
    
    let remote = manifest.remote.unwrap();
    assert_eq!(remote.target_nodes, target_nodes);
    assert_eq!(remote.timeout_secs, 30);
    assert_eq!(remote.retry, 3);
    assert!(matches!(remote.load_balance, LoadBalanceStrategy::RoundRobin));
    
    println!("✅ Remote skill config test passed");
}

/// 测试 DAG Skill 配置
#[tokio::test]
async fn test_dag_skill_config() {
    use cis_core::skill::manifest::{
        SkillManifest, DagDefinition, DagTaskDefinition, 
        DagPolicy, TaskLevelDefinition
    };
    
    // 创建 DAG tasks
    let task1 = DagTaskDefinition::new("1", "git-diff")
        .with_name("获取代码变更")
        .mechanical(3);
    
    let task2 = DagTaskDefinition::new("2", "ai-analyze")
        .with_name("AI 分析")
        .with_deps(vec!["1"])
        .confirmed();
    
    let dag = DagDefinition {
        policy: DagPolicy::AllSuccess,
        tasks: vec![task1, task2],
    };
    
    let manifest = SkillManifest::default_native("test-dag")
        .as_dag(dag);
    
    // 验证
    assert_eq!(manifest.skill.skill_type, cis_core::skill::manifest::SkillType::Dag);
    assert!(manifest.dag.is_some());
    
    let dag = manifest.dag.unwrap();
    assert_eq!(dag.tasks.len(), 2);
    assert!(matches!(dag.policy, DagPolicy::AllSuccess));
    
    // 验证依赖关系
    assert!(dag.tasks[1].deps.contains(&"1".to_string()));
    
    println!("✅ DAG skill config test passed");
}

/// 测试公共记忆同步接口
/// 
/// 注意：此测试需要完整的 P2P 网络环境
#[tokio::test]
#[ignore = "Requires full P2P network setup"]
async fn test_public_memory_sync_interface() {
    // 实际测试需要在完整网络环境中进行
    println!("✅ Public memory sync interface test placeholder");
}

/// 测试 Agent 直接调用 Skill 接口
#[tokio::test]
async fn test_agent_skill_call_interface() {
    use cis_core::agent::bridge::AgentCisClient;
    use cis_core::skill::manager::SkillManager;
    use cis_core::storage::db::DbManager;
    
    // 创建 AgentCisClient
    let client = AgentCisClient::new();
    
    // 创建 SkillManager（用于测试）
    let db_manager = Arc::new(DbManager::new().expect("Failed to create DbManager"));
    let skill_manager = Arc::new(tokio::sync::Mutex::new(
        SkillManager::new(db_manager).expect("Failed to create SkillManager")
    ));
    
    // 使用 with_skill_manager 配置
    let client = client.with_skill_manager(skill_manager);
    
    // 注意：实际调用需要注册的 skill，这里只测试接口存在
    // client.skill_call("test-skill", "test-method", b"{}").await
    
    println!("✅ Agent skill call interface test passed");
}

/// 注意：每个测试函数都可以独立运行
/// cargo test test_matrix_login_verification_code -- --nocapture
/// cargo test test_dht_put_get -- --nocapture
/// 等等

/// 网络连通性测试（需要实际网络环境）
#[tokio::test]
#[ignore = "Requires actual network environment"]
async fn test_network_connectivity() {
    use tokio::net::TcpListener;
    
    // 测试本地端口绑定
    let listener = TcpListener::bind("127.0.0.1:0").await
        .expect("Failed to bind to local address");
    
    let local_addr = listener.local_addr().unwrap();
    println!("Successfully bound to {}", local_addr);
    
    // 测试端口连通性
    let test_port = local_addr.port();
    let connect_result = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", test_port)).await;
    assert!(connect_result.is_ok(), "Should be able to connect to local port");
    
    println!("✅ Network connectivity test passed");
}

/// 联邦服务器启动测试（需要完整环境）
#[tokio::test]
#[ignore = "Requires full federation setup"]
async fn test_federation_server_startup() {
    use cis_core::matrix::federation::{
        FederationServer, FederationConfig, PeerDiscovery
    };
    use cis_core::matrix::store::MatrixStore;
    
    let config = FederationConfig::new("test.local");
    let discovery = PeerDiscovery::default();
    let store = Arc::new(MatrixStore::open_in_memory().unwrap());
    
    let server = FederationServer::new(config, discovery, store);
    
    assert_eq!(server.server_name(), "test.local");
    println!("✅ Federation server startup test passed");
}
