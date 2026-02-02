//! Noise Protocol XX 模式握手
//! 
//! XX: 无已知公钥，双向认证

use snow::{Builder, TransportState};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use futures::{SinkExt, StreamExt};
use tracing::debug;

/// Noise Protocol 错误类型
#[derive(Debug, thiserror::Error)]
pub enum NoiseError {
    #[error("Snow error: {0}")]
    SnowError(String),
    
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    
    #[error("Handshake failed: {0}")]
    HandshakeFailed(String),
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Invalid pattern")]
    InvalidPattern,
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("Decryption error: {0}")]
    DecryptionError(String),
}

impl From<snow::Error> for NoiseError {
    fn from(err: snow::Error) -> Self {
        NoiseError::SnowError(err.to_string())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for NoiseError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        NoiseError::WebSocketError(err.to_string())
    }
}

/// Noise Protocol XX 模式握手
/// 
/// XX 模式特点:
/// - 双方都没有预先知道对方的公钥
/// - 双向认证
/// - 适合 P2P 场景
pub struct NoiseHandshake {
    static_key: Vec<u8>,
    pattern: String,
}

impl NoiseHandshake {
    /// Noise XX 模式默认参数
    const DEFAULT_PATTERN: &'static str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
    
    /// 创建新的握手实例
    /// 
    /// # Arguments
    /// - `static_key` - 本地静态密钥 (32 bytes for X25519)
    pub fn new(static_key: Vec<u8>) -> Self {
        Self {
            static_key,
            pattern: Self::DEFAULT_PATTERN.to_string(),
        }
    }
    
    /// 使用自定义模式创建握手实例
    pub fn with_pattern(static_key: Vec<u8>, pattern: impl Into<String>) -> Self {
        Self {
            static_key,
            pattern: pattern.into(),
        }
    }
    
    /// 作为发起方（客户端）握手
    /// 
    /// XX 模式握手流程:
    /// 1. -> e (发送 ephemeral key)
    /// 2. <- e, ee, s, es (接收响应)
    /// 3. -> s, se (发送静态 key)
    pub async fn initiator_handshake(
        &self,
        stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<TransportState, NoiseError> {
        debug!("Starting Noise XX handshake as initiator");
        
        let builder = Builder::new(self.pattern.parse().map_err(|_| NoiseError::InvalidPattern)?);
        
        let mut handshake = builder
            .local_private_key(&self.static_key)
            .build_initiator()?;
        
        let mut msg_buffer = vec![0u8; 65535];
        
        // -> e (发送 ephemeral key)
        debug!("Step 1: Sending ephemeral key");
        let len = handshake.write_message(&[], &mut msg_buffer)?;
        stream.send(Message::Binary(msg_buffer[..len].to_vec())).await?;
        
        // <- e, ee, s, es (接收响应)
        debug!("Step 2: Waiting for responder's ephemeral and static keys");
        let response = stream.next().await
            .ok_or(NoiseError::ConnectionClosed)??;
        
        match response {
            Message::Binary(data) => {
                handshake.read_message(&data, &mut msg_buffer)?;
            }
            _ => return Err(NoiseError::HandshakeFailed(
                "Expected binary message in handshake".to_string()
            )),
        }
        
        // -> s, se (发送静态 key)
        debug!("Step 3: Sending static key");
        let len = handshake.write_message(&[], &mut msg_buffer)?;
        stream.send(Message::Binary(msg_buffer[..len].to_vec())).await?;
        
        // 握手完成，转换为传输模式
        debug!("Noise handshake completed as initiator");
        Ok(handshake.into_transport_mode()?)
    }
    
    /// 作为响应方（服务器）握手
    /// 
    /// XX 模式握手流程:
    /// 1. <- e (接收 ephemeral key)
    /// 2. -> e, ee, s, es (发送响应)
    /// 3. <- s, se (接收静态 key)
    pub async fn responder_handshake(
        &self,
        stream: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<TransportState, NoiseError> {
        debug!("Starting Noise XX handshake as responder");
        
        let builder = Builder::new(self.pattern.parse().map_err(|_| NoiseError::InvalidPattern)?);
        
        let mut handshake = builder
            .local_private_key(&self.static_key)
            .build_responder()?;
        
        let mut msg_buffer = vec![0u8; 65535];
        
        // <- e (接收 ephemeral key)
        debug!("Step 1: Receiving ephemeral key");
        let request = stream.next().await
            .ok_or(NoiseError::ConnectionClosed)??;
        
        match request {
            Message::Binary(data) => {
                handshake.read_message(&data, &mut msg_buffer)?;
            }
            _ => return Err(NoiseError::HandshakeFailed(
                "Expected binary message in handshake".to_string()
            )),
        }
        
        // -> e, ee, s, es (发送响应)
        debug!("Step 2: Sending ephemeral and static keys");
        let len = handshake.write_message(&[], &mut msg_buffer)?;
        stream.send(Message::Binary(msg_buffer[..len].to_vec())).await?;
        
        // <- s, se (接收静态 key)
        debug!("Step 3: Receiving initiator's static key");
        let response = stream.next().await
            .ok_or(NoiseError::ConnectionClosed)??;
        
        match response {
            Message::Binary(data) => {
                handshake.read_message(&data, &mut msg_buffer)?;
            }
            _ => return Err(NoiseError::HandshakeFailed(
                "Expected binary message in handshake".to_string()
            )),
        }
        
        // 握手完成
        debug!("Noise handshake completed as responder");
        Ok(handshake.into_transport_mode()?)
    }
}

/// 加密传输包装器
/// 
/// 使用 Noise Protocol 的 TransportState 进行消息的加密和解密
pub struct NoiseTransport {
    state: TransportState,
}

impl NoiseTransport {
    /// 从 TransportState 创建新的加密传输实例
    pub fn new(state: TransportState) -> Self {
        Self { state }
    }
    
    /// 加密并发送消息
    /// 
    /// # Arguments
    /// - `plaintext` - 明文数据
    /// 
    /// # Returns
    /// - 加密后的密文（包含认证标签）
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, NoiseError> {
        // Noise Protocol 使用 16 字节认证标签 (ChaChaPoly1305)
        let mut ciphertext = vec![0u8; plaintext.len() + 16];
        self.state.write_message(plaintext, &mut ciphertext)?;
        Ok(ciphertext)
    }
    
    /// 解密接收到的消息
    /// 
    /// # Arguments
    /// - `ciphertext` - 密文数据（包含认证标签）
    /// 
    /// # Returns
    /// - 解密后的明文
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let mut plaintext = vec![0u8; ciphertext.len()];
        let len = self.state.read_message(ciphertext, &mut plaintext)?;
        plaintext.truncate(len);
        Ok(plaintext)
    }
    
    /// 获取底层 TransportState 的可变引用
    pub fn state_mut(&mut self) -> &mut TransportState {
        &mut self.state
    }
    
    /// 获取底层 TransportState 的引用
    pub fn state(&self) -> &TransportState {
        &self.state
    }
}

/// Noise 密钥生成工具
pub mod keys {
    use rand::rngs::OsRng;
    use rand::RngCore;
    
    /// 生成 X25519 私钥 (32 bytes)
    pub fn generate_private_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }
    
    /// 从种子生成确定性私钥
    pub fn derive_private_key(seed: &[u8]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(seed);
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_private_key() {
        let key1 = keys::generate_private_key();
        let key2 = keys::generate_private_key();
        
        // 密钥应该是 32 字节
        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);
        
        // 随机生成的密钥应该不同
        assert_ne!(key1, key2);
    }
    
    #[test]
    fn test_derive_private_key() {
        let seed = b"test seed for deterministic key derivation";
        let key1 = keys::derive_private_key(seed);
        let key2 = keys::derive_private_key(seed);
        
        // 相同种子应该生成相同密钥
        assert_eq!(key1, key2);
        
        // 不同种子应该生成不同密钥
        let key3 = keys::derive_private_key(b"different seed");
        assert_ne!(key1, key3);
    }
    
    #[test]
    fn test_noise_pattern() {
        // 验证默认模式
        let handshake = NoiseHandshake::new(vec![0u8; 32]);
        assert_eq!(handshake.pattern, NoiseHandshake::DEFAULT_PATTERN);
    }
}