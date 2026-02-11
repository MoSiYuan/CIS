//! 加密传输层测试
//!
//! 测试 QUIC 传输层的加密连接和安全特性。

use super::transport::{QuicTransport, TransportConfig};
use std::time::Duration;
use std::sync::Once;

static INIT_CRYPTO: Once = Once::new();

fn init_crypto() {
    INIT_CRYPTO.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// 测试传输配置默认值
#[test]
fn test_transport_config_default() {
    let config = TransportConfig::default();
    
    assert_eq!(config.connection_timeout, Duration::from_secs(10));
    assert_eq!(config.heartbeat_interval, Duration::from_secs(30));
    assert_eq!(config.heartbeat_timeout, Duration::from_secs(10));
    assert_eq!(config.max_concurrent_streams, 100);
    assert_eq!(config.receive_buffer_size, 65536);
    assert_eq!(config.send_buffer_size, 65536);
}

/// 测试传输配置克隆
#[test]
fn test_transport_config_clone() {
    let config = TransportConfig::default();
    let cloned = config.clone();
    
    assert_eq!(config.connection_timeout, cloned.connection_timeout);
    assert_eq!(config.heartbeat_interval, cloned.heartbeat_interval);
    assert_eq!(config.max_concurrent_streams, cloned.max_concurrent_streams);
}

/// 测试自定义传输配置
#[test]
fn test_transport_config_custom() {
    let config = TransportConfig {
        connection_timeout: Duration::from_secs(5),
        heartbeat_interval: Duration::from_secs(15),
        heartbeat_timeout: Duration::from_secs(5),
        max_concurrent_streams: 50,
        receive_buffer_size: 32768,
        send_buffer_size: 32768,
    };
    
    assert_eq!(config.connection_timeout, Duration::from_secs(5));
    assert_eq!(config.max_concurrent_streams, 50);
}

/// 测试连接信息结构
#[test]
fn test_connection_info_clone() {
    use std::net::SocketAddr;
    
    let info = super::transport::ConnectionInfo {
        node_id: "test-node".to_string(),
        address: "127.0.0.1:8080".parse().unwrap(),
        connected_at: std::time::Instant::now(),
        bytes_sent: 1000,
        bytes_received: 2000,
    };
    
    let cloned = info.clone();
    assert_eq!(info.node_id, cloned.node_id);
    assert_eq!(info.address, cloned.address);
    assert_eq!(info.bytes_sent, cloned.bytes_sent);
    assert_eq!(info.bytes_received, cloned.bytes_received);
}

/// 测试无效地址绑定失败
#[tokio::test]
async fn test_bind_invalid_address() {
    let result = QuicTransport::bind("invalid-address", "test-node").await;
    assert!(result.is_err());
}

/// 测试重复绑定相同端口
#[tokio::test]
#[ignore = "Requires network environment"]
async fn test_bind_duplicate_port() {
    init_crypto();
    let transport1 = QuicTransport::bind("127.0.0.1:0", "node-1").await;
    assert!(transport1.is_ok());
    
    // 使用相同地址应该失败
    let addr = transport1.as_ref().unwrap().listen_addr;
    let transport2 = QuicTransport::bind(&addr.to_string(), "node-2").await;
    assert!(transport2.is_err());
    
    // 清理
    let _ = transport1.unwrap().shutdown().await;
}

/// 测试传输层关闭
#[tokio::test]
async fn test_transport_shutdown() {
    let transport = QuicTransport::bind("127.0.0.1:0", "test-node").await.unwrap();
    
    let result = transport.shutdown().await;
    assert!(result.is_ok());
}

/// 测试连接到无效地址
#[tokio::test]
async fn test_connect_invalid_address() {
    let transport = QuicTransport::bind("127.0.0.1:0", "test-node").await.unwrap();
    
    let result = transport.connect_str("invalid-address").await;
    assert!(result.is_err());
    
    let _ = transport.shutdown().await;
}

/// 测试连接超时配置
#[tokio::test]
async fn test_connection_timeout() {
    init_crypto();
    let config = TransportConfig {
        connection_timeout: Duration::from_millis(100),
        ..Default::default()
    };
    
    let transport = QuicTransport::bind_with_config("127.0.0.1:0", "test-node", config).await.unwrap();
    
    // 尝试连接到一个不存在的服务（应该超时）
    let result = transport.connect("test", "127.0.0.1:1".parse().unwrap()).await;
    assert!(result.is_err());
    
    let _ = transport.shutdown().await;
}

/// 测试空连接列表
#[tokio::test]
async fn test_empty_connection_list() {
    let transport = QuicTransport::bind("127.0.0.1:0", "test-node").await.unwrap();
    
    let connections = transport.list_connections().await;
    assert!(connections.is_empty());
    
    let count = transport.active_connection_count().await;
    assert_eq!(count, 0);
    
    let _ = transport.shutdown().await;
}

/// 测试传输层创建（向后兼容）
#[tokio::test]
async fn test_transport_new_compat() {
    let transport = QuicTransport::new("127.0.0.1:0").await;
    assert!(transport.is_ok());
    
    let _ = transport.unwrap().shutdown().await;
}

/// 测试连接信息统计
#[test]
fn test_connection_info_stats() {
    let info = super::transport::ConnectionInfo {
        node_id: "test".to_string(),
        address: "127.0.0.1:8080".parse().unwrap(),
        connected_at: std::time::Instant::now(),
        bytes_sent: 0,
        bytes_received: 0,
    };
    
    assert_eq!(info.bytes_sent, 0);
    assert_eq!(info.bytes_received, 0);
}

/// 测试最大并发流配置
#[test]
fn test_max_concurrent_streams_bounds() {
    // 测试最大并发流数的边界值
    let config_low = TransportConfig {
        max_concurrent_streams: 1,
        ..Default::default()
    };
    assert_eq!(config_low.max_concurrent_streams, 1);
    
    let config_high = TransportConfig {
        max_concurrent_streams: 10000,
        ..Default::default()
    };
    assert_eq!(config_high.max_concurrent_streams, 10000);
}

/// 测试缓冲区大小配置
#[test]
fn test_buffer_size_config() {
    let config_small = TransportConfig {
        receive_buffer_size: 1024,
        send_buffer_size: 1024,
        ..Default::default()
    };
    assert_eq!(config_small.receive_buffer_size, 1024);
    assert_eq!(config_small.send_buffer_size, 1024);
    
    let config_large = TransportConfig {
        receive_buffer_size: 1024 * 1024,
        send_buffer_size: 1024 * 1024,
        ..Default::default()
    };
    assert_eq!(config_large.receive_buffer_size, 1024 * 1024);
    assert_eq!(config_large.send_buffer_size, 1024 * 1024);
}

/// 测试心跳间隔配置
#[test]
fn test_heartbeat_config() {
    let config = TransportConfig {
        heartbeat_interval: Duration::from_secs(60),
        heartbeat_timeout: Duration::from_secs(30),
        ..Default::default()
    };
    
    assert_eq!(config.heartbeat_interval, Duration::from_secs(60));
    assert_eq!(config.heartbeat_timeout, Duration::from_secs(30));
}

/// 测试断开不存在的连接
#[tokio::test]
async fn test_disconnect_nonexistent() {
    let transport = QuicTransport::bind("127.0.0.1:0", "test-node").await.unwrap();
    
    // 断开不存在的连接应该返回 Ok
    let result = transport.disconnect("nonexistent").await;
    assert!(result.is_ok());
    
    let _ = transport.shutdown().await;
}

/// 测试发送到不存在的节点
#[tokio::test]
async fn test_send_to_nonexistent_node() {
    let transport = QuicTransport::bind("127.0.0.1:0", "test-node").await.unwrap();
    
    let result = transport.send("nonexistent", b"test").await;
    assert!(result.is_err());
    
    let _ = transport.shutdown().await;
}

/// 测试打开流到不存在的节点
#[tokio::test]
async fn test_open_stream_nonexistent() {
    let transport = QuicTransport::bind("127.0.0.1:0", "test-node").await.unwrap();
    
    let result = transport.open_stream("nonexistent").await;
    assert!(result.is_err());
    
    let _ = transport.shutdown().await;
}
