# D05: P2P 传输加密设计

> 任务: P0-4 P2P 传输加密  
> 负责人: 开发 B  
> 工期: Week 5-6 (5天)  
> 状态: 设计中  
> 依赖: D01 配置抽象

---

## 目标

为 P2P 网络通信添加 Noise Protocol 加密，确保传输安全。

---

## 当前问题

```rust
// ❌ 明文传输 - p2p/transport.rs
// 当前 QUIC 连接未启用加密
let endpoint = Endpoint::server(server_config, bind_addr)?;
// 证书是自签名的，且未做 DHT 密钥交换
```

---

## 设计方案

### 协议栈

```
┌─────────────────────────────────────────┐
│           CIS P2P 协议栈                │
├─────────────────────────────────────────┤
│  应用层: P2P Message (序列化)            │
├─────────────────────────────────────────┤
│  加密层: Noise Protocol (XX pattern)     │
│  - 握手: XK1 (静态密钥 + 临时密钥)        │
│  - 传输: ChaCha20-Poly1305               │
├─────────────────────────────────────────┤
│  传输层: QUIC (基于 UDP)                 │
│  - 多路复用                             │
│  - 拥塞控制                             │
├─────────────────────────────────────────┤
│  网络层: UDP                             │
└─────────────────────────────────────────┘
```

---

### 密钥管理

```rust
// p2p/crypto/keys.rs

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer, Verifier};
use x25519_dalek::{StaticSecret, PublicKey as X25519PublicKey};

/// 节点密钥对
pub struct NodeKeyPair {
    /// Ed25519 签名密钥 (用于 DID 身份)
    ed25519: Keypair,
    /// X25519 加密密钥 (用于 Noise 握手)
    x25519: StaticSecret,
}

impl NodeKeyPair {
    /// 从 DID 助记词生成
    pub fn from_mnemonic(mnemonic: &str) -> Result<Self> {
        let seed = Self::mnemonic_to_seed(mnemonic)?;
        
        // Ed25519 签名密钥
        let ed25519_secret = SecretKey::from_bytes(&seed[..32])?;
        let ed25519_public = PublicKey::from(&ed25519_secret);
        let ed25519 = Keypair { secret: ed25519_secret, public: ed25519_public };
        
        // X25519 加密密钥
        let x25519 = StaticSecret::from(<[u8; 32]>::try_from(&seed[32..64]).unwrap());
        
        Ok(Self { ed25519, x25519 })
    }
    
    /// 获取 X25519 公钥 (用于 Noise 握手)
    pub fn x25519_public(&self) -> X25519PublicKey {
        X25519PublicKey::from(&self.x25519)
    }
    
    /// 签名数据
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.ed25519.sign(data)
    }
    
    /// 验证签名
    pub fn verify(&self, data: &[u8], signature: &Signature) -> Result<()> {
        self.ed25519.public.verify(data, signature)
            .map_err(|e| Error::signature_invalid(e.to_string()))
    }
}
```

---

### Noise 协议实现

```rust
// p2p/crypto/noise.rs

use snow::{Builder, HandshakeState, TransportState};
use snow::params::NoiseParams;

/// Noise XX pattern (双方都有静态密钥)
const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

pub struct NoiseHandshake {
    state: HandshakeState,
    local_static: StaticSecret,
    local_ephemeral: Option<EphemeralSecret>,
}

impl NoiseHandshake {
    pub fn new_initiator(local_static: StaticSecret) -> Result<Self>    {
        let params: NoiseParams = NOISE_PARAMS.parse()?;
        let builder = Builder::new(params);
        
        let state = builder
            .local_private_key(&local_static.to_bytes())
            .build_initiator()?;
        
        Ok(Self {
            state,
            local_static,
            local_ephemeral: None,
        })
    }
    
    pub fn new_responder(local_static: StaticSecret) -> Result<Self> {
        let params: NoiseParams = NOISE_PARAMS.parse()?;
        let builder = Builder::new(params);
        
        let state = builder
            .local_private_key(&local_static.to_bytes())
            .build_responder()?;
        
        Ok(Self {
            state,
            local_static,
            local_ephemeral: None,
        })
    }
    
    /// 发送握手消息
    pub fn write_message(&mut self, payload: &[u8], out: &mut [u8]) -> Result<usize> {
        let len = self.state.write_message(payload, out)?;
        Ok(len)
    }
    
    /// 读取握手消息
    pub fn read_message(&mut self, data: &[u8], out: &mut [u8]) -> Result<usize> {
        let len = self.state.read_message(data, out)?;
        Ok(len)
    }
    
    /// 完成握手，进入传输模式
    pub fn into_transport(self) -> Result<NoiseTransport> {
        let transport = self.state.into_transport_mode()?;
        Ok(NoiseTransport { state: transport })
    }
}

/// 加密传输状态
pub struct NoiseTransport {
    state: TransportState,
}

impl NoiseTransport {
    /// 加密发送
    pub fn encrypt(&mut self, plaintext: &[u8], out: &mut [u8]) -> Result<usize> {
        let len = self.state.write_message(plaintext, out)?;
        Ok(len)
    }
    
    /// 解密接收
    pub fn decrypt(&mut self, ciphertext: &[u8], out: &mut [u8]) -> Result<usize> {
        let len = self.state.read_message(ciphertext, out)?;
        Ok(len)
    }
}
```

---

### 集成到 QUIC

```rust
// p2p/transport_secure.rs

use quinn::{Endpoint, ServerConfig, ClientConfig};

/// 安全的 P2P 传输
pub struct SecureP2PTransport {
    endpoint: Endpoint,
    key_pair: NodeKeyPair,
    noise_sessions: Arc<RwLock<HashMap<ConnectionId, NoiseTransport>>>,
}

impl SecureP2PTransport {
    pub async fn new(config: &P2PConfig, key_pair: NodeKeyPair) -> Result<Self> {
        // 创建 QUIC endpoint
        let endpoint = Self::create_endpoint(config).await?;
        
        Ok(Self {
            endpoint,
            key_pair,
            noise_sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// 接受连接并执行 Noise 握手
    pub async fn accept(&self) -> Result<SecureConnection> {
        // 1. 接受 QUIC 连接
        let quic_conn = self.endpoint.accept().await
            .ok_or(Error::connection_closed())?;
        let quic_conn = quic_conn.await?;
        
        // 2. 在 QUIC Stream 上执行 Noise 握手
        let (mut send, mut recv) = quic_conn.open_bi().await?;
        
        let mut handshake = NoiseHandshake::new_responder(
            self.key_pair.x25519.clone()
        )?;
        
        // 握手消息交换...
        let transport = self.perform_handshake(&mut handshake, &mut send, &mut recv).await?;
        
        // 3. 保存会话
        let conn_id = quic_conn.stable_id();
        self.noise_sessions.write().await.insert(conn_id, transport);
        
        Ok(SecureConnection {
            quic: quic_conn,
            conn_id,
            noise_sessions: self.noise_sessions.clone(),
        })
    }
    
    /// 连接并执行 Noise 握手
    pub async fn connect(&self, addr: SocketAddr) -> Result<SecureConnection> {
        // 1. 建立 QUIC 连接
        let quic_conn = self.endpoint.connect(addr, "cis")?.await?;
        
        // 2. 在 QUIC Stream 上执行 Noise 握手
        let (mut send, mut recv) = quic_conn.open_bi().await?;
        
        let mut handshake = NoiseHandshake::new_initiator(
            self.key_pair.x25519.clone()
        )?;
        
        let transport = self.perform_handshake(&mut handshake, &mut send, &mut recv).await?;
        
        // 3. 保存会话
        let conn_id = quic_conn.stable_id();
        self.noise_sessions.write().await.insert(conn_id, transport);
        
        Ok(SecureConnection {
            quic: quic_conn,
            conn_id,
            noise_sessions: self.noise_sessions.clone(),
        })
    }
    
    /// 执行 Noise 握手
    async fn perform_handshake(
        &self,
        handshake: &mut NoiseHandshake,
        send: &mut SendStream,
        recv: &mut RecvStream,
    ) -> Result<NoiseTransport> {
        let mut buffer = vec![0u8; 65535];
        
        // XX pattern 握手通常需要 3 条消息
        for _ in 0..3 {
            // 发送握手消息
            let len = handshake.write_message(&[], &mut buffer)?;
            send.write_all(&buffer[..len]).await?;
            
            // 接收握手消息
            let len = recv.read(&mut buffer).await?
                .ok_or(Error::connection_closed())?;
            handshake.read_message(&buffer[..len], &mut [])?;
        }
        
        Ok(handshake.into_transport()?)
    }
}

/// 安全连接
pub struct SecureConnection {
    quic: Connection,
    conn_id: ConnectionId,
    noise_sessions: Arc<RwLock<HashMap<ConnectionId, NoiseTransport>>>,
}

impl SecureConnection {
    /// 加密发送
    pub async fn send(&self, data: &[u8]) -> Result<()> {
        let mut transport = self.noise_sessions.write().await;
        let session = transport.get_mut(&self.conn_id)
            .ok_or(Error::session_not_found())?;
        
        // 加密
        let mut encrypted = vec![0u8; data.len() + 16];  // 16 bytes for auth tag
        let len = session.encrypt(data, &mut encrypted)?;
        
        // 在 QUIC Stream 上发送
        let (mut send, _) = self.quic.open_bi().await?;
        send.write_all(&encrypted[..len]).await?;
        
        Ok(())
    }
    
    /// 解密接收
    pub async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        let mut transport = self.noise_sessions.write().await;
        let session = transport.get_mut(&self.conn_id)
            .ok_or(Error::session_not_found())?;
        
        // 从 QUIC Stream 接收
        let (_, mut recv) = self.quic.accept_bi().await?;
        let mut encrypted = vec![0u8; buf.len() + 16];
        let len = recv.read(&mut encrypted).await?
            .ok_or(Error::connection_closed())?;
        
        // 解密
        let plaintext_len = session.decrypt(&encrypted[..len], buf)?;
        
        Ok(plaintext_len)
    }
}
```

---

## 握手流程

```
发起方                          响应方
   │                               │
   │ ───── Noise msg 1 ─────────▶ │
   │ (e)                          │
   │                              │
   │ ◀───── Noise msg 2 ───────── │
   │                              │ (e, ee, s, es)
   │                              │
   │ ───── Noise msg 3 ─────────▶ │
   │ (s, se)                      │
   │                              │
   │ ═════ 加密通道建立 ════════ │
   │                              │
```

**XX Pattern**:
- 双方都有静态密钥
- 互相验证身份
- 前向安全

---

## 任务清单

- [ ] 创建 `p2p/crypto/` 模块
- [ ] 实现 `NodeKeyPair`
- [ ] 集成 Noise Protocol (`snow` crate)
- [ ] 实现 `SecureP2PTransport`
- [ ] 实现 Noise 握手流程
- [ ] 集成到现有 P2P 网络
- [ ] 更新配置支持加密开关
- [ ] Wireshark 抓包测试
- [ ] 性能测试

---

## 验收标准

```bash
# 测试 1: 加密连接建立
cis node connect <peer-id>
# Wireshark 抓包应显示乱码（非明文）

# 测试 2: 身份验证
# 使用错误密钥连接
# 预期: 握手失败

# 测试 3: 性能测试
# 加密 vs 明文吞吐量对比
# 预期: 加密开销 < 20%

# 测试 4: 长期运行
# 持续传输 1 小时
# 预期: 连接稳定，无内存泄漏
```

---

## 依赖

- D01 配置抽象
- `snow` crate (Noise Protocol)
- `ed25519-dalek` (签名)
- `x25519-dalek` (密钥交换)

---

*设计创建日期: 2026-02-10*
