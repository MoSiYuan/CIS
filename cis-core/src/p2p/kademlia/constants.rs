//! Kademlia DHT 常量定义
//!
//! 定义 Kademlia 协议的核心参数。

/// Kademlia 的 K 参数 - 每个 bucket 最多存储的节点数
pub const K: usize = 20;

/// 并行查询参数 α - 同时查询的节点数
pub const ALPHA: usize = 3;

/// 节点 ID 长度（字节）- 160 bits = 20 bytes
pub const ID_LENGTH: usize = 20;

/// 节点 ID 长度（比特）
pub const ID_BITS: usize = ID_LENGTH * 8;

/// Bucket 数量 - 等于 ID 比特数
pub const NUM_BUCKETS: usize = ID_BITS;

/// 请求超时时间（毫秒）
pub const REQUEST_TIMEOUT_MS: u64 = 3000;

/// 最大迭代查找次数
pub const MAX_LOOKUP_ITERATIONS: usize = 10;

/// 刷新间隔（秒）
pub const BUCKET_REFRESH_INTERVAL_SECS: u64 = 3600;

/// 节点过期时间（秒）
pub const NODE_EXPIRATION_SECS: u64 = 86400;

/// 值存储复制因子
pub const REPLICATION_FACTOR: usize = K;

/// 值过期时间（秒）
pub const VALUE_EXPIRATION_SECS: u64 = 86400;
