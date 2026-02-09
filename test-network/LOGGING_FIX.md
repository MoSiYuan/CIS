# CIS Pairing 日志增强与 Bug 修复

## 修复时间
2026-02-09 11:35

## 问题诊断

### 发现的 Bug
**UDP 响应未发送**：在 `handle_message` 函数中，收到 `PAIR_REQ` 后：
- ✅ 打印了收到请求的日志
- ❌ **创建了响应字符串但未实际发送！**
- ✅ 直接返回了成功结果

### 代码对比

**修复前：**
```rust
let _response = "PAIR_ACK|PENDING".to_string();
// 没有发送代码！
```

**修复后：**
```rust
let response = format!("PAIR_ACK|SUCCESS|node1|{}", addr);
socket.send_to(response.as_bytes(), addr).await?;
```

## 日志增强

### 添加的日志级别

#### INFO 级别
- `[PAIRING] Generating pairing code for node: {}`
- `[PAIRING] Generated code: {}`
- `[PAIRING] Session created. Active sessions: {}`
- `[PAIRING] Verifying code: {} from requester: {}`
- `[PAIRING] Code verified: {}`
- `[PAIRING] Pairing acknowledged`
- `[PAIRING] Requesting pairing with code {} to {}`
- `[PAIRING] Response sent: {} bytes`

#### DEBUG 级别
- `[PAIRING] Received {} bytes from {}`
- `[PAIRING] Message: {}`
- `[PAIRING] Handling message from {}`
- `[PAIRING] Parsing response`

#### WARN 级别
- `[PAIRING] Code not found: {}`
- `[PAIRING] Code expired: {}`
- `[PAIRING] Code already used: {}`
- `[PAIRING] Too many attempts for code: {}`
- `[PAIRING] Failed to send response: {}`

#### ERROR 级别
- `[PAIRING] Failed to bind socket: {}`
- `[PAIRING] Listen timeout`
- `[PAIRING] Timeout waiting for response`
- `[PAIRING] Invalid UTF-8 message`
- `[PAIRING] Invalid response format`

## 关键修复点

### 1. 发送 UDP 响应
```rust
async fn handle_message(&self, msg: &str, expected_code: &str, addr: SocketAddr, socket: &UdpSocket) 
    -> Result<Option<PairingResult>> 
{
    match parts[0] {
        "PAIR_REQ" => {
            // ... 验证代码 ...
            
            // 修复：实际发送 UDP 响应
            let response = format!("PAIR_ACK|SUCCESS|node1|{}", addr);
            tracing::info!("[PAIRING] Sending response to {}: {}", addr, response);
            
            match socket.send_to(response.as_bytes(), addr).await {
                Ok(n) => tracing::info!("[PAIRING] Response sent: {} bytes", n),
                Err(e) => tracing::error!("[PAIRING] Failed to send response: {}", e),
            }
            
            Ok(Some(PairingResult { ... }))
        }
        // ...
    }
}
```

### 2. 详细的请求/响应日志
```rust
pub async fn request_pairing(&self, code: &str, target_addr: SocketAddr, local_node: PairingNodeInfo) 
    -> Result<PairingResult> 
{
    tracing::info!("[PAIRING] Requesting pairing with code {} to {}", code, target_addr);
    
    // 发送请求
    let request = format!("PAIR_REQ|{}|{}", code, local_node.node_id);
    tracing::info!("[PAIRING] Sending request: {}", request);
    
    // 等待响应
    match timeout(Duration::from_secs(30), socket.recv_from(&mut buf)).await {
        Ok(Ok((len, addr))) => {
            tracing::info!("[PAIRING] Received response from {}: {}", addr, msg);
            // ...
        }
        Err(_) => {
            tracing::error!("[PAIRING] Timeout waiting for response");
            // ...
        }
    }
}
```

## 文件修改

| 文件 | 修改内容 |
|------|----------|
| `cis-core/src/network/pairing.rs` | 添加详细日志 + 修复 UDP 响应发送 |

## 构建状态

- **开始时间**: 2026-02-09 11:39
- **预计完成**: 11:50-11:55
- **日志文件**: `/tmp/cis-build-v2.log`
- **构建进程**: PID 49625

## 测试计划

构建完成后执行：
```bash
cd test-network

# 1. 启动环境
docker-compose -f docker-compose.real.yml up -d

# 2. 生成配对码
docker exec cis-node1 cis-node pair generate

# 3. 查看日志
docker logs cis-node1 2>&1 | grep "\[PAIRING\]"

# 4. 加入网络
docker exec cis-node2 cis-node pair join <CODE> --address 172.30.1.11:6768

# 5. 检查组网结果
docker logs cis-node1 2>&1 | tail -20
docker logs cis-node2 2>&1 | tail -20
```

## 预期结果

修复后应该看到：
1. node1 日志: `[PAIRING] PAIR_REQ received...`
2. node1 日志: `[PAIRING] Sending response to...`
3. node1 日志: `[PAIRING] Response sent: XX bytes`
4. node2 日志: `[PAIRING] Received response...`
5. 组网成功！
