# TASK 6.2: v1.2.0 发布

> **Phase**: 6 - 发布准备
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: 387f2d1
> **负责人**: TBD
> **周期**: Week 12

---

## 任务概述

执行 v1.2.0 正式发布，包括版本号更新、CHANGELOG、Git tag 和 crates.io 发布。

## 工作内容

### 1. 更新版本号

```bash
# 更新所有 Cargo.toml
crates/cis-common/Cargo.toml    # 0.1.0
crates/cis-types/Cargo.toml     # 0.1.0
crates/cis-traits/Cargo.toml    # 0.1.0
crates/cis-storage/Cargo.toml   # 0.1.0
crates/cis-memory/Cargo.toml    # 0.1.0
crates/cis-scheduler/Cargo.toml # 0.1.0
crates/cis-vector/Cargo.toml    # 0.1.0
crates/cis-p2p/Cargo.toml       # 0.1.0
cis-core/Cargo.toml             # 0.2.0 (主版本更新)
```

### 2. 编写 CHANGELOG

```markdown
# CHANGELOG.md

## [1.2.0] - 2026-XX-XX

### ✨ New Features

- **Modular Architecture**: 7 independent crates with clear dependencies
- **Builder Pattern**: `Runtime::builder()` for flexible initialization
- **ZeroClaw Compatibility**: Optional feature flag for ZeroClaw integration
- **Type Safety**: Newtype pattern for PeerId, TaskId, etc.

### 🔧 Improvements

- **Reduced Core Size**: cis-core is now a thin orchestration layer
- **Better Testability**: Mock implementations for all traits
- **Feature Flags**: Fine-grained control over dependencies

### 📦 New Crates

- `cis-common`: Shared types and utilities
- `cis-types`: Public type definitions
- `cis-traits`: Core trait definitions
- `cis-storage`: Storage backends (RocksDB, Sled, Memory)
- `cis-memory`: Memory management with ZeroClaw adapter
- `cis-scheduler`: Task scheduling and execution
- `cis-vector`: Vector storage and HNSW indexing
- `cis-p2p`: P2P networking for cross-device communication

### ⚠️ Breaking Changes

- Module reorganization: types moved to `cis-types`
- Runtime initialization changed to builder pattern
- Some internal APIs moved to separate crates

### 🔄 Migration Guide

See [MIGRATION.md](./MIGRATION.md)

## [1.1.x] - Previous releases
...
```

### 3. 创建 Git Tag

```bash
# 确保所有测试通过
cargo test --all-features

# 创建签名 tag
git tag -s v1.2.0 -m "CIS v1.2.0 - Modular Architecture Release"

# 推送 tag
git push origin v1.2.0
```

### 4. 发布到 crates.io

```bash
# 按照依赖顺序发布
# 1. cis-common (无依赖)
cd crates/cis-common && cargo publish && cd ../..
sleep 30

# 2. cis-types (依赖 cis-common)
cd crates/cis-types && cargo publish && cd ../..
sleep 30

# 3. cis-traits (依赖 cis-common)
cd crates/cis-traits && cargo publish && cd ../..
sleep 30

# 4. cis-storage (依赖 cis-common, cis-traits)
cd crates/cis-storage && cargo publish && cd ../..
sleep 30

# 5. cis-memory (依赖 cis-common, cis-traits, cis-storage)
cd crates/cis-memory && cargo publish && cd ../..
sleep 30

# 6. cis-scheduler (依赖 cis-common, cis-traits)
cd crates/cis-scheduler && cargo publish && cd ../..
sleep 30

# 7. cis-vector (依赖 cis-common, cis-traits)
cd crates/cis-vector && cargo publish && cd ../..
sleep 30

# 8. cis-p2p (依赖 cis-common, cis-traits)
cd crates/cis-p2p && cargo publish && cd ../..
sleep 30

# 9. cis-core (依赖所有其他 crates)
cd cis-core && cargo publish && cd ..
```

### 5. 创建 GitHub Release

```bash
# 使用 GitHub CLI 创建 release
gh release create v1.2.0 \
  --title "CIS v1.2.0 - Modular Architecture" \
  --notes-file CHANGELOG.md \
  --verify-tag
```

## 验收标准

- [ ] 所有 crates 发布到 crates.io
- [ ] GitHub Release 创建
- [ ] CHANGELOG 完整
- [ ] 文档网站更新
- [ ] 社区公告发布

## 依赖

- Task 6.1 (文档更新)
- Task 5.2 (CI 配置)

## 阻塞

- Phase 7 (多 Agent - P3 可选)

---
