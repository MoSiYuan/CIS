# WebSocket Test DAGs

This directory contains DAG (Directed Acyclic Graph) configurations for testing WebSocket functionality in CIS.

## Test DAGs

### 1. websocket_basic.toml
Tests basic WebSocket communication:
- Connection establishment
- Handshake exchange
- Ping/Pong heartbeat
- Data message sending/receiving
- Graceful connection close

**Usage:**
```bash
cis test run test_dag/websocket/websocket_basic.toml
```

### 2. websocket_reconnect.toml
Tests WebSocket reconnection behavior:
- Automatic reconnection configuration
- Connection state transitions during reconnect
- Message sending after reconnection
- Exponential backoff

**Usage:**
```bash
cis test run test_dag/websocket/websocket_reconnect.toml
```

### 3. websocket_concurrent.toml
Tests concurrent WebSocket connections:
- Multiple simultaneous connections (3+)
- Concurrent message sending
- Message ordering verification
- Resource cleanup after concurrent usage

**Usage:**
```bash
cis test run test_dag/websocket/websocket_concurrent.toml
```

### 4. websocket_error.toml
Tests WebSocket error handling:
- Invalid URL handling
- Connection refused scenarios
- Invalid message format handling
- Timeout scenarios
- Authentication failures
- Network error simulation

**Usage:**
```bash
cis test run test_dag/websocket/websocket_error.toml
```

## Prerequisites

1. WebSocket server must be running on `ws://localhost:6768` (default port)
2. Test framework must support WebSocket node types
3. Sufficient system resources for concurrent tests

## Test Configuration

Each DAG supports the following common configuration options:

### websocket_client node
- `url`: WebSocket server URL
- `timeout`: Connection timeout in seconds
- `connection_id`: Unique identifier for the connection
- `auto_reconnect`: Enable automatic reconnection
- `max_reconnect_attempts`: Maximum reconnection attempts

### websocket_message node
- `connection_id`: Target connection (for multi-connection tests)
- `message_type`: Type of message (handshake, ping, pong, data, auth)
- `payload`: Message payload for data messages
- `sequence`: Sequence number for ordering
- `timeout`: Receive timeout in seconds

### websocket_assert node
- `expected_state`: Expected connection state
- `error_type`: Expected error type
- `active_connections`: List of expected active connections
- `message_count`: Expected message count

## Expected Results

All tests should:
- Complete without panics
- Verify expected connection states
- Properly clean up resources
- Report detailed test results

## Troubleshooting

1. **Connection refused**: Ensure WebSocket server is running
2. **Timeout errors**: Check network connectivity and server responsiveness
3. **Concurrent test failures**: Verify sufficient file descriptors and ports available
