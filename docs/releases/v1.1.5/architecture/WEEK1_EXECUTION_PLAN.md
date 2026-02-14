# CIS v1.1.5 Week 1 执行计划

> v1.1.5 第一周具体执行任务
> 日期: 2026-02-10 ~ 2026-02-17

---

## 本周目标

1. **P0-1: Mock 降级移除** - 完成 100%
2. **P0-2: Kademlia DHT 设计** - 完成设计文档
3. **技术预研** - SSH Key 库调研完成

---

## Day 1 (2026-02-10) - Mock 降级移除

### 任务 1.1: 识别所有 Mock 降级代码
```bash
# 查找所有 Mock 降级
grep -rn "MockEmbeddingService\|MockNetworkService\|MockStorageService" \
  cis-core/src --include="*.rs" | grep -v test | grep -v "// "
```

**目标文件**:
- `cis-core/src/container.rs:108` - MockEmbeddingService 降级
- `cis-core/src/container.rs:127` - MockEmbeddingService 降级
- `cis-core/src/container.rs:196` - MockNetworkService 降级

### 任务 1.2: 修改 Cargo.toml
```toml
# 修改前
[features]
default = []
vector = ["dep:sqlite-vec"]
p2p = ["dep:quinn"]

# 修改后
[features]
default = ["vector", "p2p"]  # 默认启用所有必要 features
vector = ["dep:sqlite-vec"]
p2p = ["dep:quinn"]

# 生产环境编译时检查
[profile.release]
features = ["vector", "p2p"]  # 强制要求
```

### 任务 1.3: 修改 ServiceContainer
```rust
// 移除所有 cfg 条件下的 Mock 降级代码
// 替换为编译错误提示

#[cfg(not(feature = "vector"))]
compile_error!(
    "The 'vector' feature must be enabled for production builds. \
     Use: cargo build --features vector"
);

#[cfg(not(feature = "p2p"))]
compile_error!(
    "The 'p2p' feature must be enabled for production builds. \
     Use: cargo build --features p2p"
);
```

**验收标准**:
- [ ] `cargo build --release` 成功（使用默认 features）
- [ ] `cargo build --no-default-features` 编译失败并显示清晰错误
- [ ] 所有 Mock 降级代码已移除

---

## Day 2 (2026-02-11) - Mock 降级测试

### 任务 2.1: 更新测试代码
- 确保测试使用 `TestContainerBuilder` 而非依赖 Mock 降级
- 更新 `cis-core/tests/container_production_test.rs`

### 任务 2.2: 编写编译错误测试
```rust
// tests/feature_check_test.rs
#[test]
#[cfg(not(feature = "vector"))]
fn test_vector_feature_required() {
    // 这应该编译失败
    panic!("vector feature must be enabled");
}
```

### 任务 2.3: 更新 CI/CD
```yaml
# .github/workflows/ci.yml
- name: Build
  run: cargo build --release --all-features
  
- name: Check no-default-features fails
  run: |
    if cargo build --no-default-features 2>&1; then
      echo "ERROR: Build should fail without features"
      exit 1
    fi
```

**验收标准**:
- [ ] 所有测试通过
- [ ] CI 配置更新
- [ ] 文档更新（BUILD.md）

---

## Day 3 (2026-02-12) - Kademlia 设计

### 任务 3.1: 研究 Kademlia 协议
- 阅读 Kademlia 论文
- 研究 libp2p Kademlia 实现
- 确定 CIS 的适配点

### 任务 3.2: 设计 Kademlia 模块结构
```
cis-core/src/p2p/kademlia/
├── mod.rs           # 模块导出
├── routing_table.rs # K-buckets 实现
├── node_id.rs       # 160-bit 节点 ID
├── distance.rs      # XOR 距离计算
├── lookup.rs        # 迭代查找
├── protocol.rs      # 协议消息定义
└── tests.rs         # 单元测试
```

### 任务 3.3: 编写设计文档
**文档**: `docs/design/kademlia_implementation.md`

内容包括:
- Kademlia 概述
- CIS 适配设计
- 数据结构定义
- 算法流程
- 接口设计
- 测试策略

**验收标准**:
- [ ] 设计文档完成
- [ ] 接口定义完成
- [ ] 团队评审通过

---

## Day 4 (2026-02-13) - Kademlia 接口实现

### 任务 4.1: 实现 NodeId
```rust
/// 160-bit Kademlia 节点 ID
pub struct NodeId([u8; 20]);

impl NodeId {
    pub fn from_bytes(bytes: [u8; 20]) -> Self;
    pub fn from_public_key(key: &PublicKey) -> Self;
    pub fn xor_distance(&self, other: &NodeId) -> Distance;
    pub fn bit(&self, index: usize) -> bool;
}
```

### 任务 4.2: 实现 Distance
```rust
/// XOR 距离
pub struct Distance([u8; 20]);

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        // 大端序比较
        self.0.cmp(&other.0)
    }
}
```

### 任务 4.3: 实现 KBucket
```rust
/// K-bucket (K=20)
pub struct KBucket {
    nodes: Vec<NodeInfo>,
    replacement_cache: Vec<NodeInfo>,
}

impl KBucket {
    pub fn insert(&mut self, node: NodeInfo) -> InsertResult;
    pub fn remove(&mut self, id: &NodeId);
    pub fn find_closest(&self, target: &NodeId, count: usize) -> Vec<NodeInfo>;
}
```

**验收标准**:
- [ ] NodeId 实现完成
- [ ] Distance 计算正确
- [ ] KBucket 基础操作完成
- [ ] 单元测试通过

---

## Day 5 (2026-02-14) - Kademlia 路由表

### 任务 5.1: 实现 RoutingTable
```rust
/// Kademlia 路由表
pub struct RoutingTable {
    local_id: NodeId,
    buckets: [KBucket; 160], // 每个 bit 一个 bucket
}

impl RoutingTable {
    pub fn new(local_id: NodeId) -> Self;
    pub fn update(&mut self, node: NodeInfo) -> UpdateResult;
    pub fn find_node(&self, target: &NodeId) -> Vec<NodeInfo>;
    pub fn find_closest(&self, target: &NodeId, count: usize) -> Vec<NodeInfo>;
}
```

### 任务 5.2: 实现刷新机制
```rust
impl RoutingTable {
    /// 定期刷新 bucket
    pub async fn refresh_bucket(&mut self, bucket_index: usize);
    
    /// 检查节点活性
    pub async fn check_node_health(&mut self, id: &NodeId) -> bool;
}
```

### 任务 5.3: 编写测试
```rust
#[test]
fn test_routing_table_update() {
    let local = NodeId::random();
    let mut table = RoutingTable::new(local);
    
    // 插入节点
    let node = NodeInfo::random();
    assert!(table.update(node).is_success());
    
    // 查找节点
    let closest = table.find_closest(&node.id, 1);
    assert_eq!(closest[0].id, node.id);
}
```

**验收标准**:
- [ ] RoutingTable 实现完成
- [ ] 刷新机制实现
- [ ] 单元测试覆盖率 > 80%

---

## Weekend (2026-02-15 ~ 16) - SSH Key 调研

### 任务 6.1: 调研 ssh-key 库
- 评估 `ssh-key` crate
- 评估 `openssh-keys` crate
- 确定选择

### 任务 6.2: 调研 ssh-agent 库
- 评估 `ssh-agent-client` crate
- 测试与本地 ssh-agent 通信

### 任务 6.3: 编写 SSH Key 方案文档
**文档**: `docs/design/ssh_key_protection.md`

内容包括:
- 密钥派生流程
- 加解密流程
- ssh-agent 集成
- 硬件密钥支持
- 错误处理

**验收标准**:
- [ ] 库选择确定
- [ ] 方案文档完成
- [ ] 原型代码验证

---

## Week 1 验收标准

### P0-1: Mock 降级移除 ✅
- [ ] 所有 Mock 降级代码已移除
- [ ] 编译时强制检查 features
- [ ] CI 更新
- [ ] 文档更新

### P0-2: Kademlia 设计 ✅
- [ ] 设计文档完成
- [ ] NodeId 实现
- [ ] Distance 实现
- [ ] KBucket 基础实现
- [ ] RoutingTable 基础实现

### 技术预研 ✅
- [ ] SSH Key 库调研完成
- [ ] 方案文档完成

---

## 风险与缓解

| 风险 | 概率 | 缓解措施 |
|------|------|---------|
| Kademlia 实现复杂 | 中 | 先实现基础功能，高级特性延后 |
| SSH Key 库不成熟 | 低 | 准备备选方案（原密码方案） |
| Week 1 工作量过大 | 低 | Mock 降级优先，Kademlia 可延期 1-2 天 |

---

## 每日站会模板

```
昨日完成:
- 任务 X: 完成度 Y%

今日计划:
- 任务 Z: 目标完成

阻塞:
- 需要 XXX 协助
```

---

*计划创建: 2026-02-10*  
*执行周期: 2026-02-10 ~ 2026-02-16*
