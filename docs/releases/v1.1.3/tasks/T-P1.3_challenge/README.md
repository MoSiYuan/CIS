# T-P1.3: Matrix Challenge Response

**优先级**: 🟡 P1  
**预估时间**: 4h  
**依赖**: Noise protocol  
**分配**: Agent-B

---

## 问题描述

WebSocket 连接的 challenge response 是 placeholder。

**问题文件**: `cis-core/src/matrix/websocket/client.rs:583`

**当前代码**:
```rust
// Create challenge response (placeholder)
let response = b"placeholder";
```

---

## 修复方案

实现 Noise protocol 握手:

```rust
use snow::NoiseBuilder;

pub async fn noise_handshake(&mut self) -> Result<()> {
    let builder = NoiseBuilder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s");
    let static_key = self.load_static_key().await?;
    
    let noise = builder.local_private_key(&static_key).build_initiator()?;
    
    // -> e
    let mut buf = [0u8; 1024];
    let len = noise.write_message(&[], &mut buf)?;
    self.send(&buf[..len]).await?;
    
    // <- e, ee, s, es
    let msg = self.recv().await?;
    let mut noise = noise.read_message(&msg, &mut buf)?;
    
    Ok(())
}
```

---

## 验收标准

- [ ] 实现 Noise protocol 握手
- [ ] 使用 X25519 密钥交换
- [ ] 加密通信
