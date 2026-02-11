# CIS 项目 Kademlia DHT 实现方案

## 概述

本文档为 CIS 项目设计完整的 Kademlia DHT 实现方案，替代当前的简化 DHT 实现。

---

## 1. Kademlia 核心概念

### 1.1 160-bit Node ID

Kademlia 使用 160 位（20 字节）的节点 ID，通常通过对节点公钥进行 SHA-1 哈希得到。

```rust
/// Kademlia 节点 ID (160-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId([u8; 20]);

impl NodeId {
    /// 从公钥生成节点 ID (SHA-256 取前 20 字节)
    pub fn from_public_key(public_key: &[u8]) -> Self {
        use sha2::{Sha256, Digest};
        let hash = Sha256::digest(public_key);
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&hash[..20]);
        Self(bytes)
    }
    
    /// 生成随机节点 ID
    pub fn random() -> Self {
        let mut bytes = [0u8; 20];
        rand::thread_rng().fill(&mut bytes);
        Self(bytes)
    }
    
    /// 获取原始字节
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
    
    /// 获取位长度中的最高有效位的位置（用于确定 bucket 索引）
    pub fn leading_zeros(&self) -> u32 {
        self.0.iter().map(|b| b.leading_zeros()).sum()
    }
}
```

### 1.2 XOR 距离度量

Kademlia 使用 XOR 作为距离度量，这是其核心创新。XOR 距离具有以下数学特性：
- 对称性: `d(x,y) = d(y,x)`
- 非负性: `d(x,y) >= 0`
- 三角不等式: `d(x,z) <= d(x,y) + d(y,z)`
- 唯一性: `d(x,y) = 0` 当且仅当 `x = y`

```rust
/// XOR 距离 (256-bit 以支持未来扩展)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XorDistance([u8; 32]);

impl XorDistance {
    /// 计算两个节点 ID 的 XOR 距离
    pub fn between(a: &NodeId, b: &NodeId) -> Self {
        let mut result = [0u8; 32];
        // 前 20 字节计算 XOR，其余补 0
        for i in 0..20 {
            result[i] = a.as_bytes()[i] ^ b.as_bytes()[i];
        }
        Self(result)
    }
    
    /// 获取距离的最高有效位位置（0-255）
    /// 用于确定 K-bucket 索引: index = 255 - leading_zeros
    pub fn bucket_index(&self) -> Option<usize> {
        for (i, byte) in self.0.iter().enumerate() {
            if *byte != 0 {
                let bit_index = byte.leading_zeros() as usize;
                return Some((255 - 7 - i * 8) + bit_index);
            }
        }
        None // 距离为 0（同一节点）
    }
    
    /// 获取 bucket 索引 (0-159 对应 Kademlia 的 160 个 buckets)
    pub fn k_bucket_index(&self) -> Option<usize> {
        self.bucket_index().map(|idx| 159 - idx.min(159))
    }
}
```

### 1.3 K-buckets (K=20)

Kademlia 的路由表由最多 160 个 k-buckets 组成，每个 bucket 最多存储 K（通常 20）个节点。

```rust
/// Kademlia 参数常量
pub mod constants {
    /// 每个 bucket 的最大节点数 (K)
    pub const K: usize = 20;
    /// 并行查询数 (α)
    pub const ALPHA: usize = 3;
    /// 节点 ID 位数
    pub const KEY_SIZE: usize = 160;
    /// 数据复制因子
    pub const REPLICATION_FACTOR: usize = 3;
    /// Bucket 刷新间隔（秒）
    pub const BUCKET_REFRESH_INTERVAL_SECS: u64 = 3600;
    /// 节点过期时间（秒）
    pub const NODE_TIMEOUT_SECS: u64 = 86400; // 24 小时
    /// 请求超时时间（毫秒）
    pub const REQUEST_TIMEOUT_MS: u64 = 10000;
}

/// K-bucket 条目状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    /// 节点正常
    Connected,
    /// 节点可疑（需要 ping 确认）
    Questionable,
    /// 节点已断开
    Disconnected,
}

/// K-bucket 条目
#[derive(Debug, Clone)]
pub struct KBucketEntry {
    /// 节点 ID
    pub node_id: NodeId,
    /// 网络地址
    pub address: SocketAddr,
    /// DID
    pub did: String,
    /// 最后看到时间
    pub last_seen: Instant,
    /// 最后 ping 时间
    pub last_pinged: Option<Instant>,
    /// 节点状态
    pub status: NodeStatus,
    /// 失败计数
    pub fail_count: u32,
    /// RTT (往返时间，毫秒)
    pub rtt_ms: Option<u64>,
}

impl KBucketEntry {
    pub fn new(node_id: NodeId, address: SocketAddr, did: String) -> Self {
        Self {
            node_id,
            address,
            did,
            last_seen: Instant::now(),
            last_pinged: None,
            status: NodeStatus::Connected,
            fail_count: 0,
            rtt_ms: None,
        }
    }
    
    /// 更新最后看到时间
    pub fn update_seen(&mut self) {
        self.last_seen = Instant::now();
        self.status = NodeStatus::Connected;
        self.fail_count = 0;
    }
    
    /// 记录 ping 结果
    pub fn record_ping(&mut self, success: bool, rtt_ms: Option<u64>) {
        self.last_pinged = Some(Instant::now());
        if success {
            self.status = NodeStatus::Connected;
            self.fail_count = 0;
            self.rtt_ms = rtt_ms;
        } else {
            self.fail_count += 1;
            if self.fail_count >= 3 {
                self.status = NodeStatus::Disconnected;
            } else {
                self.status = NodeStatus::Questionable;
            }
        }
    }
    
    /// 检查是否需要 ping
    pub fn needs_ping(&self, timeout: Duration) -> bool {
        match self.status {
            NodeStatus::Connected => {
                self.last_seen.elapsed() > timeout
            }
            NodeStatus::Questionable => {
                self.last_pinged.map_or(true, |t| t.elapsed() > timeout / 2)
            }
            NodeStatus::Disconnected => false
        }
    }
}

/// K-bucket 实现（使用 LRU 策略）
pub struct KBucket {
    /// 节点条目（按最后看到时间排序，最新的在后面）
    entries: VecDeque<KBucketEntry>,
    /// 替换缓存（最近被替换的节点）
    replacement_cache: VecDeque<KBucketEntry>,
    /// 最后更新时间
    last_updated: Instant,
}

impl KBucket {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(constants::K),
            replacement_cache: VecDeque::with_capacity(constants::K),
            last_updated: Instant::now(),
        }
    }
    
    /// 插入或更新节点
    /// 返回需要 ping 的旧节点（如果 bucket 满了）
    pub fn insert(&mut self, entry: KBucketEntry) -> Option<KBucketEntry> {
        // 检查是否已存在
        if let Some(pos) = self.entries.iter().position(|e| e.node_id == entry.node_id) {
            // 移动到队列尾部（最近使用）
            let mut existing = self.entries.remove(pos).unwrap();
            existing.update_seen();
            self.entries.push_back(existing);
            return None;
        }
        
        // Bucket 未满，直接添加
        if self.entries.len() < constants::K {
            self.entries.push_back(entry);
            self.last_updated = Instant::now();
            return None;
        }
        
        // Bucket 满了，检查替换缓存
        if let Some(pos) = self.replacement_cache.iter().position(|e| e.node_id == entry.node_id) {
            self.replacement_cache.remove(pos);
        }
        
        // 返回最旧的节点，需要 ping 确认
        let oldest = self.entries.front().cloned();
        self.replacement_cache.push_back(entry);
        
        // 限制替换缓存大小
        while self.replacement_cache.len() > constants::K {
            self.replacement_cache.pop_front();
        }
        
        oldest
    }
    
    /// 移除节点
    pub fn remove(&mut self, node_id: &NodeId) -> Option<KBucketEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.node_id == *node_id) {
            let removed = self.entries.remove(pos);
            
            // 尝试从替换缓存中补充
            if let Some(replacement) = self.replacement_cache.pop_back() {
                self.entries.push_back(replacement);
            }
            
            return removed;
        }
        None
    }
    
    /// 获取所有节点（按最近使用排序）
    pub fn nodes(&self) -> Vec<KBucketEntry> {
        self.entries.iter().cloned().collect()
    }
    
    /// 获取节点数量
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    
    /// 检查 bucket 是否已满
    pub fn is_full(&self) -> bool {
        self.entries.len() >= constants::K
    }
    
    /// 获取最后更新时间
    pub fn last_updated(&self) -> Instant {
        self.last_updated
    }
}
```

### 1.4 并行异步查找 (α=3)

Kademlia 使用 α 参数控制并行查询数量，默认 α=3。

```rust
/// 查询配置
#[derive(Debug, Clone)]
pub struct QueryConfig {
    /// 并行查询数 (α)
    pub parallelism: usize,
    /// 查询超时时间
    pub timeout: Duration,
    /// 最大查询迭代次数
    pub max_iterations: usize,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            parallelism: constants::ALPHA,
            timeout: Duration::from_millis(constants::REQUEST_TIMEOUT_MS),
            max_iterations: 10,
        }
    }
}
```

---

## 2. 关键算法

### 2.1 节点查找算法 (Iterative Lookup)

```rust
/// 查找结果
#[derive(Debug, Clone)]
pub struct LookupResult {
    /// 找到的最近节点
    pub closest_nodes: Vec<KBucketEntry>,
    /// 是否找到目标
    pub found_target: bool,
    /// 查询迭代次数
    pub iterations: usize,
    /// 查询的节点数量
    pub queried_count: usize,
}

/// 节点查找算法伪代码
async fn iterative_lookup(
    &self,
    target: &NodeId,
    find_value: bool,
) -> Result<LookupResult> {
    let config = QueryConfig::default();
    
    // 1. 从本地路由表获取初始 k 个最近节点
    let mut closest = self.routing_table.closest_nodes(target, constants::K);
    let mut queried = HashSet::new();
    let mut iterations = 0;
    let mut found_value = None;
    
    loop {
        iterations += 1;
        if iterations > config.max_iterations {
            break;
        }
        
        // 2. 选择 α 个未查询的最近节点
        let candidates: Vec<_> = closest
            .iter()
            .filter(|n| !queried.contains(&n.node_id))
            .take(config.parallelism)
            .cloned()
            .collect();
        
        if candidates.is_empty() {
            break; // 没有更多节点可以查询
        }
        
        // 3. 并行发送查询请求
        let queries: Vec<_> = candidates
            .iter()
            .map(|node| self.send_query(node, target, find_value))
            .collect();
        
        let results = join_all(queries).await;
        
        // 4. 处理响应
        for (node, result) in candidates.iter().zip(results) {
            queried.insert(node.node_id.clone());
            
            match result {
                Ok(QueryResponse::Nodes(nodes)) => {
                    // 将新节点加入候选列表
                    for new_node in nodes {
                        if !queried.contains(&new_node.node_id) {
                            self.routing_table.update_node(new_node.clone());
                            closest.push(new_node);
                        }
                    }
                }
                Ok(QueryResponse::Value(value)) => {
                    found_value = Some(value);
                    break;
                }
                Err(e) => {
                    tracing::warn!("Query to {} failed: {}", node.node_id, e);
                }
            }
        }
        
        if found_value.is_some() {
            break;
        }
        
        // 5. 按距离排序，保留最近的 k 个
        closest.sort_by_key(|n| XorDistance::between(&n.node_id, target));
        closest.truncate(constants::K);
        
        // 6. 检查是否收敛（没有更近的节点发现）
        let all_queried = closest.iter().all(|n| queried.contains(&n.node_id));
        if all_queried {
            break; // 已经查询了所有最近节点
        }
    }
    
    Ok(LookupResult {
        closest_nodes: closest,
        found_target: found_value.is_some(),
        iterations,
        queried_count: queried.len(),
    })
}
```

### 2.2 值存储算法 (STORE)

```rust
/// 存储选项
#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// TTL (生存时间)
    pub ttl: Option<Duration>,
    /// 是否要求确认
    pub require_ack: bool,
    /// 重试次数
    pub retry_count: u32,
}

impl Default for StoreOptions {
    fn default() -> Self {
        Self {
            ttl: None,
            require_ack: true,
            retry_count: 3,
        }
    }
}

/// 存储算法伪代码
async fn store_value(
    &self,
    key: &[u8],
    value: Vec<u8>,
    options: StoreOptions,
) -> Result<StoreResult> {
    // 1. 计算 key 的节点 ID
    let key_id = NodeId::from_bytes(key)?;
    
    // 2. 查找最近的 k 个节点
    let target = key_id;
    let lookup_result = self.iterative_lookup(&target, false).await?;
    
    // 3. 向最近的 REPLICATION_FACTOR 个节点存储
    let storage_nodes = lookup_result.closest_nodes
        .into_iter()
        .take(constants::REPLICATION_FACTOR)
        .collect::<Vec<_>>();
    
    let mut success_count = 0;
    let mut failed_nodes = Vec::new();
    
    for node in &storage_nodes {
        match self.send_store_request(node, key, &value, options.ttl).await {
            Ok(_) => {
                success_count += 1;
                tracing::debug!("Stored to {}", node.node_id);
            }
            Err(e) => {
                failed_nodes.push(node.node_id.clone());
                tracing::warn!("Failed to store to {}: {}", node.node_id, e);
            }
        }
    }
    
    // 4. 同时存储到本地
    self.local_storage.put(key, value, options.ttl).await?;
    
    Ok(StoreResult {
        stored_count: success_count,
        target_count: storage_nodes.len(),
        failed_nodes,
    })
}
```

### 2.3 值查找算法 (FIND_VALUE)

```rust
/// 查找值结果
#[derive(Debug, Clone)]
pub enum FindValueResult {
    /// 找到值
    Found(Vec<u8>),
    /// 返回最近的 k 个节点
    ClosestNodes(Vec<KBucketEntry>),
}

/// 查找值算法伪代码
async fn find_value(&self, key: &[u8]) -> Result<FindValueResult> {
    // 1. 首先检查本地存储
    if let Some(value) = self.local_storage.get(key).await? {
        return Ok(FindValueResult::Found(value));
    }
    
    // 2. 计算 key 的节点 ID
    let key_id = NodeId::from_bytes(key)?;
    
    // 3. 迭代查找
    let lookup_result = self.iterative_lookup(&key_id, true).await?;
    
    // 4. 如果查找过程中发现值，返回它
    // （iterative_lookup 会在 find_value=true 时返回找到的值）
    
    // 5. 否则返回最近的节点列表
    Ok(FindValueResult::ClosestNodes(lookup_result.closest_nodes))
}
```

### 2.4 Bucket 刷新策略

```rust
/// Bucket 刷新管理器
pub struct BucketRefresher {
    routing_table: Arc<RoutingTable>,
    last_refreshed: RwLock<[Instant; constants::KEY_SIZE]>,
    refresh_interval: Duration,
}

impl BucketRefresher {
    /// 启动后台刷新任务
    pub async fn start_refresh_task(&self) {
        let mut interval = tokio::time::interval(self.refresh_interval);
        
        loop {
            interval.tick().await;
            
            // 检查每个 bucket 是否需要刷新
            for bucket_index in 0..constants::KEY_SIZE {
                if self.should_refresh_bucket(bucket_index).await {
                    if let Err(e) = self.refresh_bucket(bucket_index).await {
                        tracing::warn!("Failed to refresh bucket {}: {}", bucket_index, e);
                    }
                }
            }
        }
    }
    
    /// 检查 bucket 是否需要刷新
    async fn should_refresh_bucket(&self, bucket_index: usize) -> bool {
        let last_refreshed = self.last_refreshed.read().await[bucket_index];
        
        // 如果 bucket 为空且超过刷新间隔，需要刷新
        if self.routing_table.bucket_is_empty(bucket_index) {
            return last_refreshed.elapsed() > self.refresh_interval;
        }
        
        // 如果 bucket 中的节点都过期了，需要刷新
        let nodes = self.routing_table.bucket_nodes(bucket_index);
        let all_stale = nodes.iter().all(|n| {
            n.status != NodeStatus::Connected || n.last_seen.elapsed() > Duration::from_secs(constants::NODE_TIMEOUT_SECS)
        });
        
        all_stale || last_refreshed.elapsed() > self.refresh_interval * 24
    }
    
    /// 刷新指定 bucket
    async fn refresh_bucket(&self, bucket_index: usize) -> Result<()> {
        // 1. 生成 bucket 范围内的随机节点 ID
        let random_id = self.generate_random_id_in_bucket(bucket_index);
        
        // 2. 查找这个随机 ID
        let result = self.iterative_lookup(&random_id, false).await?;
        
        // 3. 更新最后刷新时间
        self.last_refreshed.write().await[bucket_index] = Instant::now();
        
        tracing::info!(
            "Refreshed bucket {}: found {} nodes",
            bucket_index,
            result.closest_nodes.len()
        );
        
        Ok(())
    }
    
    /// 生成 bucket 范围内的随机节点 ID
    fn generate_random_id_in_bucket(&self, bucket_index: usize) -> NodeId {
        let mut rng = rand::thread_rng();
        let mut bytes = [0u8; 20];
        rng.fill(&mut bytes);
        
        // 确保生成的 ID 落在指定 bucket 范围内
        // 通过设置高位来实现
        let byte_index = bucket_index / 8;
        let bit_index = 7 - (bucket_index % 8);
        
        // 设置前缀，使得 XOR 距离落在目标 bucket
        bytes[byte_index] |= 1 << bit_index;
        
        NodeId(bytes)
    }
}
```

---

## 3. 核心数据结构定义

### 3.1 协议消息格式

```rust
/// Kademlia RPC 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KademliaMessage {
    /// PING 请求
    Ping {
        /// 发送者节点 ID
        sender_id: NodeId,
        /// 发送者地址
        sender_addr: SocketAddr,
        /// 随机数（用于验证响应）
        nonce: u64,
        /// 时间戳
        timestamp: u64,
    },
    
    /// PONG 响应
    Pong {
        /// 响应者节点 ID
        sender_id: NodeId,
        /// 对应的 ping nonce
        nonce: u64,
        /// 响应者知道的节点列表
        nodes: Vec<NodeInfoMsg>,
    },
    
    /// FIND_NODE 请求
    FindNode {
        /// 发送者节点 ID
        sender_id: NodeId,
        /// 目标节点 ID
        target_id: NodeId,
        /// 随机数
        nonce: u64,
    },
    
    /// FIND_NODE 响应
    FoundNode {
        /// 响应者节点 ID
        sender_id: NodeId,
        /// 对应的 nonce
        nonce: u64,
        /// 最近的 k 个节点
        nodes: Vec<NodeInfoMsg>,
    },
    
    /// FIND_VALUE 请求
    FindValue {
        /// 发送者节点 ID
        sender_id: NodeId,
        /// 键
        key: Vec<u8>,
        /// 随机数
        nonce: u64,
    },
    
    /// FIND_VALUE 响应（找到值）
    FoundValue {
        /// 响应者节点 ID
        sender_id: NodeId,
        /// 对应的 nonce
        nonce: u64,
        /// 值
        value: Vec<u8>,
        /// TTL
        ttl_secs: Option<u64>,
    },
    
    /// FIND_VALUE 响应（未找到值，返回节点）
    FoundValueNodes {
        /// 响应者节点 ID
        sender_id: NodeId,
        /// 对应的 nonce
        nonce: u64,
        /// 最近的 k 个节点
        nodes: Vec<NodeInfoMsg>,
    },
    
    /// STORE 请求
    Store {
        /// 发送者节点 ID
        sender_id: NodeId,
        /// 键
        key: Vec<u8>,
        /// 值
        value: Vec<u8>,
        /// TTL（秒）
        ttl_secs: Option<u64>,
        /// 时间戳
        timestamp: u64,
    },
    
    /// STORE 确认
    StoreAck {
        /// 响应者节点 ID
        sender_id: NodeId,
        /// 键
        key: Vec<u8>,
        /// 状态
        status: StoreStatus,
    },
}

/// 节点信息消息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfoMsg {
    pub node_id: NodeId,
    pub address: SocketAddr,
    pub did: String,
}

/// 存储状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StoreStatus {
    Success,
    Error { message: String },
}
```

### 3.2 路由表实现

```rust
/// Kademlia 路由表
pub struct RoutingTable {
    /// 本地节点 ID
    local_id: NodeId,
    /// K-buckets (索引 0 对应最远的 bucket)
    buckets: RwLock<[KBucket; constants::KEY_SIZE]>,
}

impl RoutingTable {
    pub fn new(local_id: NodeId) -> Self {
        // 初始化 160 个空 bucket
        let buckets: [KBucket; constants::KEY_SIZE] = 
            std::array::from_fn(|_| KBucket::new());
        
        Self {
            local_id,
            buckets: RwLock::new(buckets),
        }
    }
    
    /// 计算节点 ID 对应的 bucket 索引
    fn bucket_index(&self, node_id: &NodeId) -> usize {
        let distance = XorDistance::between(&self.local_id, node_id);
        distance.k_bucket_index().unwrap_or(constants::KEY_SIZE - 1)
    }
    
    /// 添加或更新节点
    pub async fn update_node(&self, entry: KBucketEntry) -> Option<KBucketEntry> {
        let bucket_idx = self.bucket_index(&entry.node_id);
        let mut buckets = self.buckets.write().await;
        buckets[bucket_idx].insert(entry)
    }
    
    /// 移除节点
    pub async fn remove_node(&self, node_id: &NodeId) -> Option<KBucketEntry> {
        let bucket_idx = self.bucket_index(node_id);
        let mut buckets = self.buckets.write().await;
        buckets[bucket_idx].remove(node_id)
    }
    
    /// 获取最近的 k 个节点
    pub fn closest_nodes(&self, target: &NodeId, count: usize) -> Vec<KBucketEntry> {
        let mut all_nodes: Vec<_> = self.all_nodes();
        
        // 按距离排序
        all_nodes.sort_by_key(|n| XorDistance::between(&n.node_id, target));
        
        // 返回最近的 count 个
        all_nodes.into_iter().take(count).collect()
    }
    
    /// 获取所有节点
    fn all_nodes(&self) -> Vec<KBucketEntry> {
        let buckets = self.buckets.blocking_read();
        buckets.iter()
            .flat_map(|b| b.nodes())
            .collect()
    }
    
    /// 获取指定 bucket 是否为空
    pub fn bucket_is_empty(&self, index: usize) -> bool {
        let buckets = self.buckets.blocking_read();
        buckets.get(index).map_or(true, |b| b.is_empty())
    }
    
    /// 获取指定 bucket 的节点
    pub fn bucket_nodes(&self, index: usize) -> Vec<KBucketEntry> {
        let buckets = self.buckets.blocking_read();
        buckets.get(index).map_or_else(Vec::new, |b| b.nodes())
    }
}
```

---

## 4. 关键接口设计 (Rust Traits)

### 4.1 DHT 服务 Trait

```rust
/// DHT 服务接口
#[async_trait]
pub trait DhtService: Send + Sync {
    /// 获取本地节点 ID
    fn local_node_id(&self) -> NodeId;
    
    /// 启动 DHT 服务
    async fn start(&self, bootstrap_nodes: Vec<SocketAddr>) -> Result<()>;
    
    /// 停止 DHT 服务
    async fn stop(&self) -> Result<()>;
    
    /// 存储键值对
    async fn put(&self, key: &[u8], value: Vec<u8>, options: StoreOptions) -> Result<StoreResult>;
    
    /// 查找值
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    
    /// 查找节点
    async fn find_node(&self, target_id: &NodeId) -> Result<Vec<KBucketEntry>>;
    
    /// 获取路由表统计
    async fn routing_table_stats(&self) -> RoutingTableStats;
    
    /// 订阅 DHT 事件
    fn subscribe_events(&self) -> mpsc::Receiver<DhtEvent>;
}

/// DHT 服务引用类型
pub type DhtServiceRef = Arc<dyn DhtService>;
```

### 4.2 网络传输 Trait

```rust
/// DHT 网络传输接口
#[async_trait]
pub trait DhtTransport: Send + Sync {
    /// 发送消息到指定节点
    async fn send_to(
        &self,
        node_id: &NodeId,
        address: SocketAddr,
        message: &KademliaMessage,
    ) -> Result<()>;
    
    /// 发送消息并等待响应
    async fn send_request(
        &self,
        node_id: &NodeId,
        address: SocketAddr,
        message: &KademliaMessage,
        timeout: Duration,
    ) -> Result<KademliaMessage>;
    
    /// 监听传入消息
    async fn listen<F>(&self, handler: F) -> Result<()>
    where
        F: Fn(NodeId, KademliaMessage) -> BoxFuture<'static, Option<KademliaMessage>> + Send + Sync;
}
```

### 4.3 存储 Trait

```rust
/// DHT 本地存储接口
#[async_trait]
pub trait DhtStorage: Send + Sync {
    /// 存储值
    async fn put(&self, key: &[u8], value: Vec<u8>, ttl: Option<Duration>) -> Result<()>;
    
    /// 获取值
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    
    /// 删除值
    async fn delete(&self, key: &[u8]) -> Result<bool>;
    
    /// 获取过期键列表
    async fn expired_keys(&self) -> Result<Vec<Vec<u8>>>;
    
    /// 清理过期数据
    async fn cleanup(&self) -> Result<usize>;
}
```

---

## 5. CIS 适配考虑

### 5.1 与现有 P2PNetwork 集成

```rust
/// DHT 服务实现，集成到 P2PNetwork
pub struct KademliaDhtService {
    /// 本地节点 ID
    local_id: NodeId,
    /// 路由表
    routing_table: Arc<RoutingTable>,
    /// 本地存储
    storage: Arc<dyn DhtStorage>,
    /// 网络传输（复用 P2PNetwork 的连接）
    transport: Arc<dyn DhtTransport>,
    /// 配置
    config: DhtConfig,
    /// 运行状态
    running: AtomicBool,
    /// 事件发送器
    event_sender: broadcast::Sender<DhtEvent>,
    /// 查询管理器
    query_manager: Arc<QueryManager>,
}

impl KademliaDhtService {
    /// 从 P2PNetwork 创建 DHT 服务
    pub fn from_p2p_network(
        network: &P2PNetwork,
        config: DhtConfig,
    ) -> Result<Self> {
        // 从 P2PNetwork 获取节点密钥生成 NodeId
        let node_keys = network.node_keys();
        let local_id = NodeId::from_public_key(&node_keys.public_key_to_bytes());
        
        // 创建路由表
        let routing_table = Arc::new(RoutingTable::new(local_id));
        
        // 创建存储
        let storage = Arc::new(MemoryStorage::new());
        
        // 创建传输层（包装 P2PNetwork）
        let transport = Arc::new(P2PNetworkTransport::new(network));
        
        let (event_sender, _) = broadcast::channel(100);
        
        Ok(Self {
            local_id,
            routing_table,
            storage,
            transport,
            config,
            running: AtomicBool::new(false),
            event_sender,
            query_manager: Arc::new(QueryManager::new()),
        })
    }
}

#[async_trait]
impl DhtService for KademliaDhtService {
    fn local_node_id(&self) -> NodeId {
        self.local_id
    }
    
    async fn start(&self, bootstrap_nodes: Vec<SocketAddr>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        
        // 启动消息监听
        let transport = Arc::clone(&self.transport);
        let handler = self.create_message_handler();
        tokio::spawn(async move {
            transport.listen(handler).await;
        });
        
        // 连接 bootstrap 节点
        for addr in bootstrap_nodes {
            self.bootstrap_connect(addr).await?;
        }
        
        // 启动后台任务
        self.start_background_tasks();
        
        Ok(())
    }
    
    // ... 其他方法实现
}
```

### 5.2 与现有 DhtOps 接口兼容性

```rust
/// 适配器：将新的 Kademlia DHT 服务适配到旧的 DhtOperations 接口
pub struct DhtOpsAdapter {
    inner: Arc<dyn DhtService>,
}

impl DhtOpsAdapter {
    pub fn new(dht_service: Arc<dyn DhtService>) -> Self {
        Self { inner: dht_service }
    }
}

impl DhtOpsAdapter {
    /// 兼容旧的 put 接口
    pub async fn put(&self, key: &str, value: &str) -> Result<DhtResult> {
        let key_bytes = key.as_bytes();
        let value_bytes = value.as_bytes().to_vec();
        
        match self.inner.put(key_bytes, value_bytes, StoreOptions::default()).await {
            Ok(result) => {
                if result.stored_count > 0 {
                    Ok(DhtResult::PutSuccess)
                } else {
                    Ok(DhtResult::Error {
                        message: "Failed to store to any node".to_string(),
                    })
                }
            }
            Err(e) => Ok(DhtResult::Error {
                message: e.to_string(),
            }),
        }
    }
    
    /// 兼容旧的 get 接口
    pub async fn get(&self, key: &str) -> Result<DhtResult> {
        match self.inner.get(key.as_bytes()).await {
            Ok(Some(value)) => {
                let value_str = String::from_utf8_lossy(&value).to_string();
                Ok(DhtResult::GetSuccess { value: value_str })
            }
            Ok(None) => Ok(DhtResult::NotFound),
            Err(e) => Ok(DhtResult::Error {
                message: e.to_string(),
            }),
        }
    }
    
    /// 兼容旧的 find_node 接口
    pub async fn find_node(&self, target_id: &str) -> Result<DhtResult> {
        // 将字符串 ID 转换为 NodeId
        let target = NodeId::from_string(target_id)?;
        
        match self.inner.find_node(&target).await {
            Ok(nodes) => {
                let node_infos: Vec<NodeInfo> = nodes
                    .into_iter()
                    .map(|n| NodeInfo {
                        node_id: n.node_id.to_string(),
                        address: n.address.to_string(),
                        distance: self.calculate_distance(&n.node_id, &target),
                    })
                    .collect();
                Ok(DhtResult::FindNodeSuccess { nodes: node_infos })
            }
            Err(e) => Ok(DhtResult::Error {
                message: e.to_string(),
            }),
        }
    }
}
```

---

## 6. 异步 Rust 实现模式

### 6.1 使用 Tokio 的并发模式

```rust
/// 查询管理器 - 管理进行中的查询
pub struct QueryManager {
    /// 活跃的查询
    active_queries: DashMap<u64, QueryState>,
    /// 查询 ID 生成器
    next_id: AtomicU64,
}

impl QueryManager {
    /// 创建新的查询
    pub fn create_query(&self, target: NodeId, find_value: bool) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let state = QueryState {
            target,
            find_value,
            start_time: Instant::now(),
            candidates: RwLock::new(BinaryHeap::new()),
            queried: DashSet::new(),
        };
        self.active_queries.insert(id, state);
        id
    }
    
    /// 完成查询
    pub fn complete_query(&self, id: u64) -> Option<QueryState> {
        self.active_queries.remove(&id).map(|(_, v)| v)
    }
}

/// 使用 tokio::spawn 进行并行查询
async fn parallel_queries(
    &self,
    candidates: Vec<KBucketEntry>,
    query_fn: impl Fn(KBucketEntry) -> BoxFuture<'static, Result<QueryResponse>>,
) -> Vec<Result<QueryResponse>> {
    let futures: Vec<_> = candidates
        .into_iter()
        .map(|node| {
            let f = query_fn(node);
            tokio::spawn(async move {
                tokio::time::timeout(Duration::from_secs(10), f).await
            })
        })
        .collect();
    
    let results = futures::future::join_all(futures).await;
    
    results
        .into_iter()
        .map(|r| match r {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(_)) => Err(anyhow!("Query timeout")),
            Err(e) => Err(anyhow!("Join error: {}", e)),
        })
        .collect()
}
```

### 6.2 使用 Channel 进行事件通知

```rust
/// DHT 事件
#[derive(Debug, Clone)]
pub enum DhtEvent {
    /// 发现新节点
    NodeDiscovered { node_id: NodeId, address: SocketAddr },
    /// 节点断开
    NodeDisconnected { node_id: NodeId },
    /// 值存储
    ValueStored { key: Vec<u8>, node_count: usize },
    /// 值检索
    ValueRetrieved { key: Vec<u8>, from_node: Option<NodeId> },
    /// 路由表更新
    RoutingTableUpdated { bucket_index: usize, node_count: usize },
}

/// 事件处理器
pub struct DhtEventHandler {
    receiver: mpsc::Receiver<DhtEvent>,
}

impl DhtEventHandler {
    pub async fn run(mut self) {
        while let Some(event) = self.receiver.recv().await {
            match event {
                DhtEvent::NodeDiscovered { node_id, address } => {
                    tracing::info!("Discovered node {} at {}", node_id, address);
                }
                DhtEvent::ValueStored { key, node_count } => {
                    tracing::info!("Stored value with key {:?} to {} nodes", key, node_count);
                }
                // ... 处理其他事件
            }
        }
    }
}
```

---

## 7. 测试策略

### 7.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_xor_distance() {
        let a = NodeId::from_bytes(&[0u8; 20]);
        let b = NodeId::from_bytes(&[1u8; 20]);
        
        let dist_ab = XorDistance::between(&a, &b);
        let dist_ba = XorDistance::between(&b, &a);
        
        // 验证对称性
        assert_eq!(dist_ab, dist_ba);
        
        // 验证到自身的距离为 0
        let dist_aa = XorDistance::between(&a, &a);
        assert!(dist_aa.bucket_index().is_none());
    }
    
    #[test]
    fn test_k_bucket_insertion() {
        let mut bucket = KBucket::new();
        
        // 添加节点直到满
        for i in 0..constants::K + 5 {
            let entry = create_test_entry(i);
            let evicted = bucket.insert(entry);
            
            if i < constants::K {
                assert!(evicted.is_none());
            } else {
                // Bucket 满后应该返回最旧的节点
                assert!(evicted.is_some());
            }
        }
        
        assert_eq!(bucket.len(), constants::K);
    }
    
    #[test]
    fn test_routing_table_closest_nodes() {
        let local_id = NodeId::random();
        let table = RoutingTable::new(local_id);
        
        // 添加一些节点
        for i in 0..100 {
            let entry = create_test_entry(i);
            table.update_node(entry);
        }
        
        let target = NodeId::random();
        let closest = table.closest_nodes(&target, 10);
        
        assert_eq!(closest.len(), 10);
        
        // 验证节点按距离排序
        for i in 1..closest.len() {
            let dist_prev = XorDistance::between(&closest[i-1].node_id, &target);
            let dist_curr = XorDistance::between(&closest[i].node_id, &target);
            assert!(dist_prev <= dist_curr);
        }
    }
}
```

### 7.2 集成测试

```rust
/// 测试两个节点的 DHT 通信
#[tokio::test]
async fn test_two_node_dht() {
    let (node1, node2) = create_test_pair().await;
    
    // Node1 存储值
    let key = b"test-key";
    let value = b"test-value".to_vec();
    
    node1.put(key, value.clone(), StoreOptions::default()).await.unwrap();
    
    // Node2 查找值
    let result = node2.get(key).await.unwrap();
    
    assert_eq!(result, Some(value));
}

/// 测试多节点网络中的值传播
#[tokio::test]
async fn test_value_propagation() {
    // 创建 10 个节点的网络
    let nodes = create_test_network(10).await;
    
    // 第一个节点存储值
    let key = b"shared-key";
    let value = b"shared-value".to_vec();
    
    nodes[0].put(key, value.clone(), StoreOptions::default()).await.unwrap();
    
    // 等待传播
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 随机选择一个节点查找
    let random_node = &nodes[rand::random::<usize>() % nodes.len()];
    let result = random_node.get(key).await.unwrap();
    
    assert_eq!(result, Some(value));
}

/// 测试节点加入和离开
#[tokio::test]
async fn test_node_churn() {
    let mut network = TestNetwork::new(5).await;
    
    // 存储值
    let key = b"churn-test";
    let value = b"value".to_vec();
    network.nodes[0].put(key, value.clone(), StoreOptions::default()).await.unwrap();
    
    // 移除两个节点
    network.remove_node(1).await;
    network.remove_node(2).await;
    
    // 验证值仍然可以访问
    let result = network.nodes[3].get(key).await.unwrap();
    assert_eq!(result, Some(value));
}
```

### 7.3 模拟测试

```rust
/// 使用模拟时钟测试超时逻辑
#[tokio::test]
async fn test_query_timeout_with_mock_clock() {
    let _guard = mock_instant::MockClock::advance_system_time(Duration::from_secs(1));
    
    let query_manager = QueryManager::new();
    let query_id = query_manager.create_query(NodeId::random(), false);
    
    // 模拟时间流逝
    mock_instant::MockClock::advance(Duration::from_secs(20));
    
    // 验证查询被标记为超时
    let state = query_manager.get_state(query_id).await;
    assert!(state.is_expired());
}

/// 测试网络分区恢复
#[tokio::test]
async fn test_network_partition_recovery() {
    let network = TestNetwork::new(10).await;
    
    // 创建网络分区
    network.partition(&[0, 1, 2, 3, 4], &[5, 6, 7, 8, 9]).await;
    
    // 在分区 A 存储值
    let key = b"partition-test";
    let value = b"value".to_vec();
    network.nodes[0].put(key, value.clone(), StoreOptions::default()).await.unwrap();
    
    // 验证分区 B 无法访问
    let result = network.nodes[5].get(key).await.unwrap();
    assert!(result.is_none());
    
    // 恢复网络连接
    network.heal_partition().await;
    
    // 等待同步
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // 验证值现在可以访问
    let result = network.nodes[5].get(key).await.unwrap();
    assert_eq!(result, Some(value));
}
```

---

## 8. 实现路线图

### Phase 1: 基础数据结构 (1 周)
1. 实现 `NodeId` 和 `XorDistance`
2. 实现 `KBucket` 和 `RoutingTable`
3. 实现基本的存储层

### Phase 2: 核心协议 (1 周)
1. 实现 PING/PONG
2. 实现 FIND_NODE
3. 实现消息编解码

### Phase 3: 查询算法 (1 周)
1. 实现迭代查找
2. 实现并行查询管理
3. 实现超时和重试逻辑

### Phase 4: 存储功能 (1 周)
1. 实现 STORE
2. 实现 FIND_VALUE
3. 实现数据复制

### Phase 5: 维护与优化 (1 周)
1. 实现 bucket 刷新
2. 实现节点存活检测
3. 性能优化

### Phase 6: CIS 集成 (1 周)
1. 集成到 P2PNetwork
2. 实现 DhtOps 适配器
3. 端到端测试

---

## 9. 参考资源

1. **Kademlia 原始论文**: Maymounkov & Mazières, "Kademlia: A Peer-to-peer Information System Based on the XOR Metric"
2. **libp2p Kademlia**: https://docs.rs/libp2p-kad
3. **S/Kademlia 扩展**: 安全增强的 Kademlia 变体
4. **CIS 现有代码**: `cis-core/src/p2p/dht.rs`, `cis-core/src/p2p/dht_ops.rs`

---

*文档版本: 1.0*  
*更新日期: 2026-02-10*
