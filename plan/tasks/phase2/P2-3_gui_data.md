# P2-3: GUI 数据连接

**优先级**: P1 (重要)  
**阶段**: Phase 2 - 核心功能完善  
**负责人**: Agent-J (Frontend)  
**预估**: 3 天  
**依赖**: P1-1 (内存安全修复)  

---

## 当前状态

GUI 使用演示数据，无法生产使用。

---

## 架构决策

**方案选择**: 共享 SQLite (推荐)
- ✅ 最简单，无需额外协议
- ✅ 利用 SQLite WAL 模式并发支持
- ✅ 直接读取 CIS 数据库

**备选方案**:
- Unix Socket: 轻量，但需要额外实现
- HTTP API: 灵活，但增加复杂性

---

## 原子任务

### [ ] P2-3.1: 实现 GUI ↔ Core 通信

**方案**: 共享 SQLite + 轮询

**文件**: `cis-gui/src/store/mod.rs` (新建)

**实现**:
```rust
use cis_core::storage::StoragePaths;

pub struct GuiDataStore {
    node_db: Connection,
    memory_db: Connection,
    matrix_social_db: Connection,
}

impl GuiDataStore {
    pub fn new() -> Result<Self> {
        let node_db = Connection::open(StoragePaths::node_db())?;
        let memory_db = Connection::open(StoragePaths::memory_db())?;
        let matrix_social_db = Connection::open(StoragePaths::matrix_social_db())?;
        
        Ok(Self { node_db, memory_db, matrix_social_db })
    }
}
```

---

### [ ] P2-3.2: 实现 NodeStore

**文件**: `cis-gui/src/store/node_store.rs`

**功能**:
- 从 `network.nodes` 表读取节点列表
- 实时状态更新 (轮询 1s)
- 节点详情查询

**代码框架**:
```rust
pub struct NodeStore {
    conn: Connection,
}

impl NodeStore {
    pub fn get_all_nodes(&self) -> Result<Vec<NodeInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, status, did, address FROM nodes ORDER BY last_seen DESC"
        )?;
        // ... 映射为 NodeInfo
    }
    
    pub fn watch_nodes(&self) -> Receiver<Vec<NodeInfo>> {
        // 启动轮询线程，每秒查询一次
        // 数据变化时发送更新
    }
}
```

---

### [ ] P2-3.3: 实现 MemoryStore

**文件**: `cis-gui/src/store/memory_store.rs`

**功能**:
- 从 `memory.db` 读取记忆/对话
- 向量搜索接口
- 时间线浏览

---

### [ ] P2-3.4: 实现 MatrixStore

**文件**: `cis-gui/src/store/matrix_store.rs`

**功能**:
- 从 `matrix-social.db` 读取用户/设备
- Room 列表和消息
- 实时消息推送 (WebSocket)

---

### [ ] P2-3.5: 替换 app.rs 演示数据

**文件**: `cis-gui/src/app.rs`

**修改**:
```rust
// 之前
let nodes = vec![
    ManagedNode { name: "Munin".to_string(), ... }, // 硬编码
];

// 之后
let store = GuiDataStore::new().unwrap();
let nodes = store.node_store().get_all_nodes().unwrap();
```

---

### [ ] P2-3.6: 替换 GLM Panel 演示数据

**文件**: `cis-gui/src/glm_panel.rs`

**修改**:
```rust
// 从数据库读取 pending DAGs
pub fn refresh_dags(&mut self) {
    self.pending_dags = self.store.dag_store().get_pending().unwrap();
}
```

---

## 实时更新机制

### 方案: 轮询 + 乐观更新

```rust
// 在 egui 的 update 循环中
fn update(&mut self, ctx: &egui::Context) {
    // 每 60 帧 (约 1s) 刷新一次
    if self.frame_count % 60 == 0 {
        self.refresh_data();
    }
}
```

### 后续优化
- 使用 SQLite `notify` 机制 (需扩展)
- WebSocket 推送 (长期)

---

## 验收标准

```bash
# 1. 启动 CIS
cis node start

# 2. 启动 GUI
cis gui

# 3. 验证真实数据
# - 节点列表显示真实节点 (非演示数据)
# - 添加节点后 GUI 自动更新
# - 记忆浏览器显示真实记忆
```

---

## 输出物清单

- [ ] `cis-gui/src/store/mod.rs` - Store 模块
- [ ] `cis-gui/src/store/node_store.rs` - 节点存储
- [ ] `cis-gui/src/store/memory_store.rs` - 记忆存储
- [ ] `cis-gui/src/store/matrix_store.rs` - Matrix 存储
- [ ] `cis-gui/src/app.rs` 更新 - 使用真实数据
- [ ] `cis-gui/src/glm_panel.rs` 更新 - 使用真实数据
- [ ] `reports/P2-3-completion.md`

---

## 并行提示

**依赖上游**: P1-1 (内存安全修复)
**可并行**: P2-1 (WASM)、P2-2 (四级决策)、P2-4 (P2P)
**冲突注意**: 修改 `app.rs` 需与 GUI 其他修改协调
