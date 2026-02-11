//! Kademlia XOR 距离实现
//!
//! XOR 距离是 Kademlia 路由的核心度量。

use super::node_id::NodeId;
use std::cmp::Ordering;

/// XOR 距离 - 160-bit 无符号整数
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Distance([u8; super::constants::ID_LENGTH]);

impl Distance {
    /// 计算两个节点 ID 之间的 XOR 距离
    pub fn between(a: &NodeId, b: &NodeId) -> Self {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();
        let mut result = [0u8; super::constants::ID_LENGTH];
        
        for i in 0..super::constants::ID_LENGTH {
            result[i] = a_bytes[i] ^ b_bytes[i];
        }
        
        Self(result)
    }

    /// 获取字节数组
    pub fn as_bytes(&self) -> &[u8; super::constants::ID_LENGTH] {
        &self.0
    }

    /// 获取最高位的 1 的位置（用于确定 bucket 索引）
    /// 
    /// 返回值范围：0-159 或 None（如果距离为 0）
    pub fn leading_zeros(&self) -> Option<usize> {
        for i in 0..super::constants::ID_BITS {
            let byte_index = i / 8;
            let bit_index = 7 - (i % 8);
            if (self.0[byte_index] >> bit_index) & 1 == 1 {
                return Some(i);
            }
        }
        None // 距离为 0
    }

    /// 检查距离是否为 0（两个节点 ID 相同）
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }

    /// 获取 bucket 索引
    /// 
    /// 与 NodeId::bucket_index 等价
    pub fn bucket_index(&self) -> Option<usize> {
        self.leading_zeros()
    }
}

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        // 按大端序比较（最高有效字节在前）
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Distance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_between() {
        let a = NodeId::from_bytes([0x00; super::super::constants::ID_LENGTH]);
        let b = NodeId::from_bytes([0xFF; super::super::constants::ID_LENGTH]);
        
        let dist = Distance::between(&a, &b);
        assert_eq!(dist.as_bytes(), &[0xFF; super::super::constants::ID_LENGTH]);
        
        // XOR 是对称的
        let dist2 = Distance::between(&b, &a);
        assert_eq!(dist, dist2);
    }

    #[test]
    fn test_distance_zero() {
        let a = NodeId::random();
        let dist = Distance::between(&a, &a);
        assert!(dist.is_zero());
        assert_eq!(dist.leading_zeros(), None);
    }

    #[test]
    fn test_distance_ordering() {
        // 距离越远，数值越大（大端序）
        let a = NodeId::from_bytes([0x00; super::super::constants::ID_LENGTH]);
        let b = NodeId::from_bytes([0x00; super::super::constants::ID_LENGTH]);
        let mut c = [0x00; super::super::constants::ID_LENGTH];
        c[0] = 0x01;
        let c = NodeId::from_bytes(c);
        let mut d = [0x00; super::super::constants::ID_LENGTH];
        d[0] = 0xFF;
        let d = NodeId::from_bytes(d);
        
        let dist_ab = Distance::between(&a, &b); // 0
        let dist_ac = Distance::between(&a, &c); // 1
        let dist_ad = Distance::between(&a, &d); // 255
        
        assert!(dist_ab < dist_ac);
        assert!(dist_ac < dist_ad);
    }

    #[test]
    fn test_leading_zeros() {
        let mut bytes = [0u8; super::super::constants::ID_LENGTH];
        bytes[0] = 0b1000_0000; // 第 0 位为 1
        let dist = Distance(bytes);
        assert_eq!(dist.leading_zeros(), Some(0));
        
        let mut bytes = [0u8; super::super::constants::ID_LENGTH];
        bytes[0] = 0b0000_0001; // 第 7 位为 1
        let dist = Distance(bytes);
        assert_eq!(dist.leading_zeros(), Some(7));
        
        let mut bytes = [0u8; super::super::constants::ID_LENGTH];
        bytes[1] = 0b1000_0000; // 第 8 位为 1
        let dist = Distance(bytes);
        assert_eq!(dist.leading_zeros(), Some(8));
    }
}
