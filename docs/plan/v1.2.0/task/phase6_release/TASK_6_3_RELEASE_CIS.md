# TASK 6.3: 发布 CIS v1.2.0

> **Phase**: 6 - 发布准备
> **状态**: 🔄 进行中 (占位符已创建)
> **负责人**: TBD
> **周期**: Week 12

---

## 任务概述

正式发布 CIS v1.2.0，包括打标签、发布到 crates.io 和创建 GitHub Release。

## 工作内容

### 1. 最终版本检查

```bash
# 确保所有测试通过
cargo test --workspace --all-features

# 确保文档构建
cargo doc --workspace --no-deps

# 确保 clippy 无警告
cargo clippy --workspace --all-features -- -D warnings

# 检查格式
cargo fmt --all -- --check
```

### 2. 更新版本号

**文件**: 所有 `Cargo.toml`

```bash
# cis-common/cis-types/Cargo.toml
version = "1.2.0"

# cis-common/cis-traits/Cargo.toml
version = "1.2.0"

# cis-common/cis-storage/Cargo.toml
version = "1.2.0"

# cis-common/cis-memory/Cargo.toml
version = "1.2.0"

# cis-common/cis-scheduler/Cargo.toml
version = "1.2.0"

# cis-common/cis-vector/Cargo.toml
version = "1.2.0"

# cis-common/cis-p2p/Cargo.toml
version = "1.2.0"

# cis-core/Cargo.toml
version = "1.2.0"
```

### 3. 创建 Git Tag

```bash
# 提交所有变更
git add -A
git commit -m "Prepare for v1.2.0 release"

# 创建签名标签
git tag -s v1.2.0 -m "CIS v1.2.0 - Modular Architecture Release

Key features:
- 7 independent cis-common crates
- Builder pattern for Runtime
- ZeroClaw compatibility (optional)
- Multi-agent architecture (P3)
- Four-level decision mechanism
- DAG orchestration
- P2P cross-device agent calls

Breaking changes:
- Module reorganization
- Runtime initialization changed

See CHANGELOG.md for details."

# 推送标签
git push origin main
git push origin v1.2.0
```

### 4. 发布到 crates.io

```bash
# 登录 crates.io
cargo login

# 按依赖顺序发布
# 1. cis-types (零依赖)
cd cis-common/cis-types
cargo publish
sleep 30

# 2. cis-traits
cd ../cis-traits
cargo publish
sleep 30

# 3. cis-storage
cd ../cis-storage
cargo publish
sleep 30

# 4. cis-memory
cd ../cis-memory
cargo publish
sleep 30

# 5. cis-scheduler
cd ../cis-scheduler
cargo publish
sleep 30

# 6. cis-vector
cd ../cis-vector
cargo publish
sleep 30

# 7. cis-p2p
cd ../cis-p2p
cargo publish
sleep 30

# 8. cis-core (主 crate)
cd ../../cis-core
cargo publish
```

### 5. 创建 GitHub Release

```bash
# 使用 GitHub CLI
gh release create v1.2.0 \
  --title "CIS v1.2.0 - Modular Architecture" \
  --notes-file CHANGELOG.md \
  --verify-tag \
  --discussion-category "Releases"

# 或手动在 GitHub Web 界面创建
```

**Release 内容模板**:

```markdown
## CIS v1.2.0 - Modular Architecture Release 🚀

### ✨ Highlights

- **Modular Architecture**: 7 independent crates with clear dependencies
- **Builder Pattern**: `Runtime::builder()` for flexible initialization  
- **ZeroClaw Compatibility**: Optional feature flag for ZeroClaw integration
- **Multi-Agent Architecture**: Receptionist + Worker Agents + DAG orchestration
- **Four-Level Decisions**: Mechanical → Recommended → Confirmed → Arbitrated
- **P2P Cross-Device**: Remote agent calls across devices

### 📦 New Crates

| Crate | Description | Version |
|-------|-------------|---------|
| cis-types | Core types | 1.2.0 |
| cis-traits | Trait definitions | 1.2.0 |
| cis-storage | Storage backends | 1.2.0 |
| cis-memory | Memory system | 1.2.0 |
| cis-scheduler | Task scheduler | 1.2.0 |
| cis-vector | Vector search | 1.2.0 |
| cis-p2p | P2P networking | 1.2.0 |

### ⚠️ Breaking Changes

See [MIGRATION.md](docs/migration-guide.md)

### 📖 Documentation

- [Integration Guide](docs/zeroclaw-integration.md)
- [Architecture Overview](docs/architecture/README.md)
- [API Documentation](https://docs.rs/cis-core/1.2.0)

### 🙏 Contributors

Thanks to all contributors!
```

### 6. 发布后的验证

```bash
# 验证 crates.io 发布
cargo search cis-core
cargo search cis-types

# 验证文档
curl -s https://docs.rs/cis-core/1.2.0 | grep -i "modular" || echo "Doc check needed"

# 验证标签
git ls-remote --tags origin | grep v1.2.0
```

### 7. 社区公告

- [ ] 发布到 GitHub Discussions
- [ ] 发送邮件到开发者列表
- [ ] 更新项目网站
- [ ] 社交媒体公告（如有）

## 验收标准

- [ ] v1.2.0 tag 创建完成
- [ ] 所有 crates 发布到 crates.io
- [ ] GitHub Release 创建完成
- [ ] 文档网站更新
- [ ] 社区公告发布

## 依赖

- Task 6.1 (文档更新)
- Task 6.2 (发布准备)
- Task 5.3 (性能测试)

## 阻塞

- Phase 7 (可选多 Agent)

---
