//! Kademlia 节点 ID 实现
//!
//! 160-bit 节点 ID，用于 Kademlia 路由。

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use rand::Rng;
use std::fmt;

/// 节点 ID - 160-bit 标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId([u8; super::constants::ID_LENGTH]);

impl NodeId {
    /// 创建新的随机节点 ID
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; super::constants::ID_LENGTH];
        rng.fill(&mut bytes);
        Self(bytes)
    }

    /// 从字节数组创建节点 ID
    pub fn from_bytes(bytes: [u8; super::constants::ID_LENGTH]) -> Self {
        Self(bytes)
    }

    /// 从公钥创建节点 ID（使用 SHA-256 哈希）
    pub fn from_public_key(public_key: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        let result = hasher.finalize();
        
        let mut bytes = [0u8; super::constants::ID_LENGTH];
        bytes.copy_from_slice(&result[..super::constants::ID_LENGTH]);
        Self(bytes)
    }

    /// 获取字节数组
    pub fn as_bytes(&self) -> &[u8; super::constants::ID_LENGTH] {
        &self.0
    }

    /// 获取指定索引的比特位
    /// 
    /// # Arguments
    /// * `index` - 比特位索引 (0-159)
    /// 
    /// # Returns
    /// * `true` - 该位为 1
    /// * `false` - 该位为 0
    pub fn bit(&self, index: usize) -> bool {
        assert!(index < super::constants::ID_BITS, "bit index out of range");
        let byte_index = index / 8;
        let bit_index = 7 - (index % 8); // 大端序
        (self.0[byte_index] >> bit_index) & 1 == 1
    }

    /// 计算与另一个节点 ID 的 XOR 距离
    pub fn distance(&self, other: &NodeId) -> super::distance::Distance {
        super::distance::Distance::between(self, other)
    }

    /// 计算 bucket 索引（用于路由表）
    /// 
    /// 返回最高不同位的索引（0-159），如果相同则返回 ID_BITS
    pub fn bucket_index(&self, other: &NodeId) -> usize {
        for i in 0..super::constants::ID_BITS {
            if self.bit(i) != other.bit(i) {
                return i;
            }
        }
        super::constants::ID_BITS
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl AsRef<[u8]> for NodeId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_random() {
        let id1 = NodeId::random();
        let id2 = NodeId::random();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_node_id_from_bytes() {
        let bytes = [0x12u8; super::super::constants::ID_LENGTH];
        let id = NodeId::from_bytes(bytes);
        assert_eq!(id.as_bytes(), &bytes);
    }

    #[test]
    fn test_node_id_bit() {
        let mut bytes = [0u8; super::super::constants::ID_LENGTH];
        bytes[0] = 0b1000_0000; // 第 0 位为 1
        bytes[1] = 0b0000_0001; // 第 15 位为 1
        let id = NodeId::from_bytes(bytes);
        
        assert!(id.bit(0));
        assert!(!id.bit(1));
        assert!(!id.bit(7));
        assert!(id.bit(15));
    }

    #[test]
    fn test_node_id_bucket_index() {
        let mut bytes1 = [0u8; super::super::constants::ID_LENGTH];
        let mut bytes2 = [0u8; super::super::constants::ID_LENGTH];
        bytes1[0] = 0b1000_0000; // 第 0 位为 1
        bytes2[0] = 0b0000_0000; // 第 0 位为 0
        
        let id1 = NodeId::from_bytes(bytes1);
        let id2 = NodeId::from_bytes(bytes2);
        
        assert_eq!(id1.bucket_index(&id2), 0);
        assert_eq!(id2.bucket_index(&id1), 0);
    }

    #[test]
    fn test_node_id_display() {
        let id = NodeId::from_bytes([0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
                                      0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                                      0x00, 0x00, 0x00, 0x00]);
        let s = id.to_string();
        assert!(s.starts_with("123456789abcdef0"));
    }
}
