# T4.1: DHT 真实操作

**任务编号**: T4.1  
**任务名称**: Real DHT Operations  
**优先级**: P2  
**预估时间**: 6h  
**依赖**: T2.1 (P2P Network)  
**分配状态**: 待分配

---

## 任务概述

实现 DHT put/get/find_node 的真实操作。

---

## 输入

### 待修改文件
- `cis-core/src/p2p/dht.rs`
- `cis-node/src/commands/p2p.rs:760-840` (模拟实现)

### 当前问题
```rust
// 模拟存储
println!("  💾 Storing key '{}' in DHT...", key);
// 模拟获取
```

---

## 输出要求

```rust
impl P2PNetwork {
    pub async fn dht_put(&self, key: &str, value: &str) -> Result<()>;
    pub async fn dht_get(&self, key: &str) -> Result<Option<String>>;
    pub async fn dht_find_node(&self, node_id: &str) -> Result<Vec<NodeInfo>>;
}
```

---

## 验收标准

- [ ] put 后 get 能获取相同值
- [ ] 跨节点数据可检索
- [ ] 路由表维护正确

---

## 阻塞关系

**依赖**:
- T2.1: P2PNetwork
