# TASK 5.2: CI/CD 配置

> **Phase**: 5 - 测试与验证
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: a2b8ae2
> **负责人**: TBD
> **周期**: Week 9-10

---

## 任务概述

配置完整的 CI/CD 流水线，包括测试、代码覆盖率检查和发布流程。

## 工作内容

### 1. GitHub Actions 配置

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta]
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          toolchain: ${{ matrix.rust }}
      
      - name: Cache cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Check formatting
        run: cargo fmt --all -- --check
      
      - name: Run clippy
        run: cargo clippy --all-features -- -D warnings
      
      - name: Build
        run: cargo build --all-features
      
      - name: Run tests
        run: cargo test --all-features
      
      - name: Run benchmarks (smoke test)
        run: cargo bench -- --test

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
      
      - name: Generate coverage
        run: cargo tarpaulin --all-features --out Xml
      
      - name: Upload to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
          fail_ci_if_error: true
```

### 2. 代码质量检查

```yaml
# .github/workflows/quality.yml
name: Code Quality

on: [pull_request]

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Security audit
        uses: actions-rust-lang/audit@v1
      
      - name: Check dependencies
        run: cargo deny check
```

### 3. 发布流程

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
      
      - name: Publish crates (ordered)
        run: |
          # 按照依赖顺序发布
          cargo publish -p cis-common --token ${{ secrets.CRATES_IO_TOKEN }}
          sleep 30
          cargo publish -p cis-types --token ${{ secrets.CRATES_IO_TOKEN }}
          sleep 30
          cargo publish -p cis-traits --token ${{ secrets.CRATES_IO_TOKEN }}
          sleep 30
          cargo publish -p cis-storage --token ${{ secrets.CRATES_IO_TOKEN }}
          # ... 继续发布其他 crates
```

### 4. 配置 cargo-deny

```toml
# deny.toml
[advisories]
version = 2
db-urls = ["https://github.com/rustsec/advisory-db"]
yanked = "deny"

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
]

[bans]
multiple-versions = "warn"
wildcards = "allow"  # 开发时允许 path 依赖
```

## 验收标准

- [ ] CI 流水线通过所有检查
- [ ] 代码覆盖率报告生成
- [ ] Security audit 通过
- [ ] 发布流程自动化
- [ ] 文档自动部署

## 依赖

- Task 5.1 (测试框架)

## 阻塞

- Task 6.1 (文档更新)

---
