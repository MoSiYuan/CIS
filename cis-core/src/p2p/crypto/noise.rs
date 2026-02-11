//! Noise Protocol 实现

use snow::{Builder, HandshakeState, TransportState};
use snow::params::NoiseParams;
use crate::error::{CisError, Result};

/// Noise XX pattern 参数
const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Noise 握手状态
pub struct NoiseHandshake {
    state: HandshakeState,
}

impl NoiseHandshake {
    /// 创建发起方握手状态
    pub fn new_initiator(local_static: &[u8; 32]) -> Result<Self> {
        let params: NoiseParams = NOISE_PARAMS.parse()
            .map_err(|e| CisError::crypto(format!("Invalid noise params: {}", e)))?;
        
        let builder = Builder::new(params);
        let state = builder.local_private_key(local_static)
            .build_initiator()
            .map_err(|e| CisError::crypto(format!("Failed to build initiator: {}", e)))?;
        
        Ok(Self { state })
    }
    
    /// 创建响应方握手状态
    pub fn new_responder(local_static: &[u8; 32]) -> Result<Self> {
        let params: NoiseParams = NOISE_PARAMS.parse()
            .map_err(|e| CisError::crypto(format!("Invalid noise params: {}", e)))?;
        
        let builder = Builder::new(params);
        let state = builder.local_private_key(local_static)
            .build_responder()
            .map_err(|e| CisError::crypto(format!("Failed to build responder: {}", e)))?;
        
        Ok(Self { state })
    }
    
    /// 写入握手消息
    pub fn write_message(&mut self, payload: &[u8], out: &mut [u8]) -> Result<usize> {
        self.state.write_message(payload, out)
            .map_err(|e| CisError::crypto(format!("Handshake write failed: {}", e)))
    }
    
    /// 读取握手消息
    pub fn read_message(&mut self, data: &[u8], out: &mut [u8]) -> Result<usize> {
        self.state.read_message(data, out)
            .map_err(|e| CisError::crypto(format!("Handshake read failed: {}", e)))
    }
    
    /// 转换为传输模式
    pub fn into_transport(self) -> Result<NoiseTransport> {
        let transport = self.state.into_transport_mode()
            .map_err(|e| CisError::crypto(format!("Failed to enter transport mode: {}", e)))?;
        
        Ok(NoiseTransport { state: transport })
    }
}

/// 加密传输状态
pub struct NoiseTransport {
    state: TransportState,
}

impl NoiseTransport {
    /// 加密数据
    pub fn encrypt(&mut self, plaintext: &[u8], out: &mut [u8]) -> Result<usize> {
        self.state.write_message(plaintext, out)
            .map_err(|e| CisError::crypto(format!("Encryption failed: {}", e)))
    }
    
    /// 解密数据
    pub fn decrypt(&mut self, ciphertext: &[u8], out: &mut [u8]) -> Result<usize> {
        self.state.read_message(ciphertext, out)
            .map_err(|e| CisError::crypto(format!("Decryption failed: {}", e)))
    }
    
    /// 获取最大消息大小
    pub const fn max_message_size() -> usize {
        65535
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::StaticSecret;
    use rand::rngs::OsRng;
    
    #[test]
    fn test_noise_handshake() {
        let local_static_a = StaticSecret::random_from_rng(&mut OsRng);
        let local_static_b = StaticSecret::random_from_rng(&mut OsRng);
        
        let mut initiator = NoiseHandshake::new_initiator(&local_static_a.to_bytes()).unwrap();
        let mut responder = NoiseHandshake::new_responder(&local_static_b.to_bytes()).unwrap();
        
        let mut buf1 = [0u8; 1024];
        let mut buf2 = [0u8; 1024];
        let mut payload = [0u8; 1024];
        
        // -> e
        let len = initiator.write_message(&[], &mut buf1).unwrap();
        responder.read_message(&buf1[..len], &mut payload).unwrap();
        
        // <- e, ee, s, es
        let len = responder.write_message(&[], &mut buf2).unwrap();
        initiator.read_message(&buf2[..len], &mut payload).unwrap();
        
        // -> s, se
        let len = initiator.write_message(&[], &mut buf1).unwrap();
        responder.read_message(&buf1[..len], &mut payload).unwrap();
        
        // 转换为传输模式
        let mut transport_a = initiator.into_transport().unwrap();
        let mut transport_b = responder.into_transport().unwrap();
        
        // 测试加密通信
        let message = b"Hello, Noise!";
        let mut encrypted = [0u8; 1024];
        let mut decrypted = [0u8; 1024];
        
        let len = transport_a.encrypt(message, &mut encrypted).unwrap();
        let len = transport_b.decrypt(&encrypted[..len], &mut decrypted).unwrap();
        
        assert_eq!(&decrypted[..len], message);
    }
}
