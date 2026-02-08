//! # WebSocket Integration Tests
//!
//! Comprehensive integration tests for WebSocket functionality including:
//! - Connection establishment and teardown
//! - Message sending and receiving
//! - Concurrent connection handling
//! - Reconnection behavior
//! - Error handling

use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::Message;

// Import from cis_core - these are the actual types we need to test
use cis_core::network::websocket::{WsConnectionConfig, WsNetworkMessage, ErrorCode};

/// Test helper to create a test WebSocket server
async fn create_test_server(port: u16) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;
    
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    
    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _peer_addr)) => {
                    tokio::spawn(async move {
                        match accept_async(stream).await {
                            Ok(ws_stream) => {
                                let (mut sender, mut receiver) = ws_stream.split();
                                
                                // Echo server - just echo back any text messages
                                while let Some(Ok(msg)) = receiver.next().await {
                                    if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                                        // Echo back with "echo: " prefix
                                        let _ = sender.send(
                                            tokio_tungstenite::tungstenite::Message::Text(
                                                format!("echo: {}", text)
                                            )
                                        ).await;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("WebSocket accept error: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Accept error: {}", e);
                }
            }
        }
    });
    
    // Give server time to start
    sleep(Duration::from_millis(100)).await;
    
    Ok(handle)
}

/// Test WebSocket connection configuration
#[tokio::test]
async fn test_websocket_connection_config() {
    let config = WsConnectionConfig::new()
        .with_connection_timeout(Duration::from_secs(30))
        .with_heartbeat_interval(Duration::from_secs(5))
        .with_auto_reconnect(true)
        .with_max_reconnect_attempts(5);
    
    // Config should be created successfully
    assert_eq!(config.connection_timeout, Duration::from_secs(30));
    assert_eq!(config.heartbeat_interval, Duration::from_secs(5));
    assert!(config.auto_reconnect);
    assert_eq!(config.max_reconnect_attempts, 5);
}

/// Test message serialization and deserialization
#[tokio::test]
async fn test_websocket_message_roundtrip() {
    // Test Ping message
    let ping = WsNetworkMessage::Ping {
        ping_id: 1,
        timestamp: 1234567890,
    };
    
    let json = serde_json::to_string(&ping).expect("Failed to serialize ping");
    let decoded: WsNetworkMessage = serde_json::from_str(&json).expect("Failed to deserialize ping");
    
    match decoded {
        WsNetworkMessage::Ping { ping_id, timestamp } => {
            assert_eq!(ping_id, 1);
            assert_eq!(timestamp, 1234567890);
        }
        _ => panic!("Expected Ping message"),
    }
    
    // Test Data message with binary payload
    let data = WsNetworkMessage::Data {
        payload: vec![0u8, 1, 2, 3, 255],
        sequence: 42,
    };
    
    let json = serde_json::to_string(&data).expect("Failed to serialize data");
    let decoded: WsNetworkMessage = serde_json::from_str(&json).expect("Failed to deserialize data");
    
    match decoded {
        WsNetworkMessage::Data { payload, sequence } => {
            assert_eq!(payload, vec![0u8, 1, 2, 3, 255]);
            assert_eq!(sequence, 42);
        }
        _ => panic!("Expected Data message"),
    }
    
    // Test Error message
    let error = WsNetworkMessage::Error {
        code: ErrorCode::AuthFailed,
        message: "Authentication failed".to_string(),
    };
    
    let json = serde_json::to_string(&error).expect("Failed to serialize error");
    let decoded: WsNetworkMessage = serde_json::from_str(&json).expect("Failed to deserialize error");
    
    match decoded {
        WsNetworkMessage::Error { code, message } => {
            assert!(matches!(code, ErrorCode::AuthFailed));
            assert_eq!(message, "Authentication failed");
        }
        _ => panic!("Expected Error message"),
    }
}

/// Test WebSocket connection establishment
#[tokio::test]
async fn test_websocket_connection_establishment() {
    // Start a test server on a random port
    let port = 19001;
    let _server = create_test_server(port).await.expect("Failed to create test server");
    
    // Give server more time to start
    sleep(Duration::from_millis(200)).await;
    
    // Connect to the server
    let url = format!("ws://127.0.0.1:{}", port);
    let connect_result = timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(&url)
    ).await;
    
    assert!(connect_result.is_ok(), "Connection should succeed within timeout");
    assert!(connect_result.unwrap().is_ok(), "Connection should be established");
}

/// Test WebSocket message sending and receiving
#[tokio::test]
async fn test_websocket_message_send_receive() {
    
    let port = 19002;
    let _server = create_test_server(port).await.expect("Failed to create test server");
    sleep(Duration::from_millis(200)).await;
    
    let url = format!("ws://127.0.0.1:{}", port);
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect");
    
    // Send a test message
    let test_msg = r#"{"type": "test", "data": "hello"}"#;
    ws_stream.send(Message::Text(test_msg.to_string())).await.expect("Failed to send");
    
    // Receive echo response
    let response = timeout(Duration::from_secs(5), ws_stream.next())
        .await
        .expect("Timeout waiting for response")
        .expect("Stream ended")
        .expect("WebSocket error");
    
    if let Message::Text(text) = response {
        assert!(text.contains("echo:"), "Should receive echo response");
        assert!(text.contains("test"), "Response should contain original message");
    } else {
        panic!("Expected text message");
    }
}

/// Test concurrent WebSocket connections
#[tokio::test]
async fn test_concurrent_websocket_connections() {
    
    let port = 19003;
    let _server = create_test_server(port).await.expect("Failed to create test server");
    sleep(Duration::from_millis(200)).await;
    
    let url = format!("ws://127.0.0.1:{}", port);
    let num_connections = 5;
    let mut handles = vec![];
    
    // Spawn multiple concurrent connections
    for i in 0..num_connections {
        let url = url.clone();
        let handle = tokio::spawn(async move {
            let (mut ws_stream, _) = tokio_tungstenite::connect_async(&url)
                .await
                .expect("Failed to connect");
            
            // Send message with connection ID
            let msg = format!(r#"{{"conn_id": {}}}"#, i);
            ws_stream.send(Message::Text(msg)).await.expect("Failed to send");
            
            // Wait for echo
            let response = timeout(Duration::from_secs(5), ws_stream.next())
                .await
                .expect("Timeout")
                .expect("Stream ended")
                .expect("WebSocket error");
            
            if let Message::Text(text) = response {
                assert!(text.contains(&format!("conn_id\": {}", i)));
                i
            } else {
                panic!("Expected text message");
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all connections to complete
    let results = futures::future::join_all(handles).await;
    
    // Verify all connections succeeded
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok(), "Connection {} should succeed", i);
        assert_eq!(result.as_ref().unwrap(), &i, "Connection {} should return its ID", i);
    }
}

/// Test WebSocket reconnection behavior
#[tokio::test]
async fn test_websocket_reconnection_logic() {
    use cis_core::network::websocket::WsClient;
    
    // Create client with reconnection enabled
    let config = WsConnectionConfig::new()
        .with_auto_reconnect(true)
        .with_max_reconnect_attempts(3)
        .with_connection_timeout(Duration::from_secs(1));
    
    let client = WsClient::with_config("test-node", config);
    
    // Try to connect to a non-existent server (should fail)
    let result = client.connect("ws://127.0.0.1:59999").await;
    assert!(result.is_err(), "Connection to non-existent server should fail");
}

/// Test WebSocket error handling
#[tokio::test]
async fn test_websocket_error_handling() {
    // Test various error scenarios
    
    // 1. Invalid URL format
    let result = tokio_tungstenite::connect_async("not-a-valid-url").await;
    assert!(result.is_err(), "Invalid URL should fail");
    
    // 2. Connection refused
    let result = timeout(
        Duration::from_secs(2),
        tokio_tungstenite::connect_async("ws://127.0.0.1:59998")
    ).await;
    
    // Either timeout or connection refused error
    match result {
        Err(_) => (), // Timeout is acceptable
        Ok(Err(_)) => (), // Connection error is also acceptable
        Ok(Ok(_)) => panic!("Should not connect to non-existent server"),
    }
}

/// Test WebSocket message serialization errors
#[tokio::test]
async fn test_message_serialization_error_handling() {
    // Test invalid JSON
    let invalid_json = "{not valid json}";
    let result: Result<WsNetworkMessage, _> = serde_json::from_str(invalid_json);
    assert!(result.is_err(), "Invalid JSON should fail to parse");
    
    // Test valid JSON with invalid message type
    let unknown_type = r#"{"type": "unknown_type", "data": "test"}"#;
    let result: Result<WsNetworkMessage, _> = serde_json::from_str(unknown_type);
    assert!(result.is_err(), "Unknown message type should fail to parse");
}

/// Test connection state transitions
#[tokio::test]
async fn test_connection_state_transitions() {
    use cis_core::network::websocket::{WsConnection, ConnectionState};
    
    let (tx, _rx) = mpsc::unbounded_channel();
    let conn = Arc::new(WsConnection::new("test-conn", tx));
    
    // Initial state
    assert!(matches!(conn.state().await, ConnectionState::Connecting));
    
    // Transition to connected
    conn.set_state(ConnectionState::Connected).await;
    assert!(matches!(conn.state().await, ConnectionState::Connected));
    
    // Transition to disconnected
    conn.set_state(ConnectionState::Disconnected).await;
    assert!(matches!(conn.state().await, ConnectionState::Disconnected));
    
    // Transition through reconnecting
    conn.set_state(ConnectionState::Reconnecting).await;
    assert!(matches!(conn.state().await, ConnectionState::Reconnecting));
}



/// Test heartbeat mechanism
#[tokio::test]
async fn test_heartbeat_mechanism() {
    use cis_core::network::websocket::WsNetworkMessage;
    
    // Test ping message creation
    let ping = WsNetworkMessage::Ping {
        ping_id: 1,
        timestamp: current_timestamp(),
    };
    
    let json = serde_json::to_string(&ping).expect("Failed to serialize ping");
    assert!(json.contains("ping"));
    assert!(json.contains("ping_id"));
    
    // Test pong message creation
    let pong = WsNetworkMessage::Pong {
        ping_id: 1,
        timestamp: current_timestamp(),
    };
    
    let json = serde_json::to_string(&pong).expect("Failed to serialize pong");
    assert!(json.contains("pong"));
}

/// Test concurrent message sending
#[tokio::test]
async fn test_concurrent_message_sending() {
    use cis_core::network::websocket::WsConnection;
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    let (tx, mut rx) = mpsc::unbounded_channel::<WsNetworkMessage>();
    let conn = Arc::new(WsConnection::new("test-conn", tx));
    
    let counter = Arc::new(AtomicUsize::new(0));
    let num_messages = 100;
    
    // Spawn multiple senders
    let mut handles = vec![];
    for i in 0..num_messages {
        let conn = conn.clone();
        let counter = counter.clone();
        
        let handle = tokio::spawn(async move {
            let msg = WsNetworkMessage::Data {
                payload: vec![i as u8],
                sequence: i as u64,
            };
            
            if conn.send(msg).is_ok() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        
        handles.push(handle);
    }
    
    // Receive all messages
    let receive_handle = tokio::spawn(async move {
        let mut received = 0;
        while let Some(_msg) = timeout(Duration::from_secs(5), rx.recv()).await.ok().flatten() {
            received += 1;
            if received >= num_messages {
                break;
            }
        }
        received
    });
    
    // Wait for all sends to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    let received = receive_handle.await.unwrap();
    assert_eq!(received, num_messages);
    assert_eq!(counter.load(Ordering::SeqCst), num_messages);
}

/// Test connection idle timeout
#[tokio::test]
async fn test_connection_idle_timeout() {
    use cis_core::network::websocket::WsConnection;
    
    let (tx, _rx) = mpsc::unbounded_channel();
    let conn = Arc::new(WsConnection::new("test-conn", tx));
    
    // Initial idle time should be very small
    let initial_idle = conn.idle_duration().await;
    assert!(initial_idle < Duration::from_millis(100));
    
    // Wait a bit
    sleep(Duration::from_millis(50)).await;
    
    // Idle time should have increased
    let later_idle = conn.idle_duration().await;
    assert!(later_idle >= Duration::from_millis(50));
    
    // Update activity
    conn.update_activity().await;
    
    // Idle time should be reset
    let after_update = conn.idle_duration().await;
    assert!(after_update < Duration::from_millis(10));
}

/// Test handshake message
#[tokio::test]
async fn test_handshake_message() {
    let handshake = WsNetworkMessage::Handshake {
        version: 1,
        node_id: "test-node".to_string(),
        timestamp: 1234567890,
    };
    
    let json = serde_json::to_string(&handshake).expect("Failed to serialize handshake");
    let decoded: WsNetworkMessage = serde_json::from_str(&json).expect("Failed to deserialize");
    
    match decoded {
        WsNetworkMessage::Handshake { version, node_id, timestamp } => {
            assert_eq!(version, 1);
            assert_eq!(node_id, "test-node");
            assert_eq!(timestamp, 1234567890);
        }
        _ => panic!("Expected Handshake message"),
    }
}

/// Test close message
#[tokio::test]
async fn test_close_message() {
    let close = WsNetworkMessage::Close {
        reason: "normal closure".to_string(),
    };
    
    let json = serde_json::to_string(&close).expect("Failed to serialize close");
    let decoded: WsNetworkMessage = serde_json::from_str(&json).expect("Failed to deserialize");
    
    match decoded {
        WsNetworkMessage::Close { reason } => {
            assert_eq!(reason, "normal closure");
        }
        _ => panic!("Expected Close message"),
    }
}

/// Helper function to get current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
