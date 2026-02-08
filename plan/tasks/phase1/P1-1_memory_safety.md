# P1-1: 内存安全修复

**优先级**: P0 (阻塞)  
**阶段**: Phase 1 - 稳定性加固  
**负责人**: Agent-A (Rust-Core)  
**预估**: 3 天  
**依赖**: 无  

---

## 问题描述

```
SIGBUS - access to undefined memory
文件: memory/service.rs, storage/db.rs
根因: 异步锁修改导致状态竞争，数据库连接未正确关闭
```

---

## 原子任务

### [ ] P1-1.1: 修复 `memory::service::tests::test_memory_service_delete`

**分析**: 异步删除操作的竞态条件

**实现步骤**:
1. 定位 `memory/service.rs` 第 ~450 行的删除逻辑
2. 添加 `Arc<Mutex<>>` 保护删除流程
3. 确保删除完成后才释放锁

**验收标准**:
```bash
cargo test -p cis-core --lib memory::service::tests::test_memory_service_delete
# 连续运行 100 次通过
for i in {1..100}; do cargo test --lib test_memory_service_delete || break; done
```

**输出物**:
- `fix/memory-service-delete.patch`

---

### [ ] P1-1.2: 修复 `storage::db::tests::test_core_db`

**分析**: 数据库连接池在测试间未隔离

**实现步骤**:
1. 使用 `tempfile::TempDir` 创建隔离测试数据库
2. 确保每个测试使用独立的数据库文件
3. 测试完成后自动清理

**代码参考**:
```rust
use tempfile::TempDir;

#[test]
fn test_core_db() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = CoreDb::open(&db_path).unwrap();
    // ... 测试逻辑
} // RAII 自动清理
```

**验收标准**:
```bash
cargo test -p cis-core --lib storage::db::tests::test_core_db
# 并行运行无冲突
cargo test --lib test_core_db -- --test-threads=8
```

**输出物**:
- `fix/storage-db-test.patch`

---

### [ ] P1-1.3: 修复 WASM 运行时内存问题 (如有)

**分析**: `wasmtime`/`wasmer` 运行时未正确初始化

**实现步骤**:
1. 检查 `scheduler/skill_executor.rs` 中的 WASM 初始化
2. 添加 `lazy_static` 全局运行时
3. 确保运行时生命周期管理正确

**验收标准**:
```bash
cargo test -p cis-core --lib wasm
# 或如果测试不存在，验证编译通过
cargo check -p cis-core
```

**输出物**:
- `fix/wasm-runtime-init.patch`

---

## 合并与验证

### 最终验收
```bash
# 1. 应用所有补丁
git apply fix/memory-service-delete.patch
git apply fix/storage-db-test.patch

# 2. 运行测试
cargo test -p cis-core --lib

# 3. 检查内存错误 (如可用)
# RUSTFLAGS="-Z sanitizer=address" cargo test --lib
```

### 输出物清单
- [ ] `fix/memory-service-delete.patch`
- [ ] `fix/storage-db-test.patch`
- [ ] `fix/wasm-runtime-init.patch` (如需要)
- [ ] `reports/P1-1-completion.md` (完成报告)

---

## 并行提示

**可并行**: P1-1 与 P1-2 (WebSocket测试)、P1-3 (项目注册表) 同时执行
**不可并行**: 同文件修改需串行
**依赖下游**: P1-4 (E2E测试) 需等待 P1-1 完成
