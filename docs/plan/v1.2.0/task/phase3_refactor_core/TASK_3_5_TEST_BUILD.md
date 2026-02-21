# TASK 3.5: 测试编译

> **Phase**: 3 - cis-core 重构
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **负责人**: TBD
> **周期**: Week 7

---

## 任务概述

全面测试 cis-common workspace 和 cis-core 的编译，确保重构后一切正常。

## 工作内容

### 1. 测试 cis-common workspace

```bash
cd cis-common

# 基础编译
cargo check

# 完整编译
cargo build --release

# 运行测试
cargo test

# 检查所有 features
cargo check --all-features
```

### 2. 测试 cis-core

```bash
cd cis-core

# 基础编译
cargo check

# 完整编译
cargo build --release

# 运行测试
cargo test

# 检查 backward compatibility
cargo check --features backward-compat
```

### 3. 测试完整 workspace

```bash
# 在根目录测试
cd /Users/jiangxiaolong/work/project/CIS

# 完整构建
cargo build --workspace --release

# 运行所有测试
cargo test --workspace

# 检查文档
cargo doc --workspace --no-deps
```

### 4. 验证向后兼容

```bash
# 测试现有代码是否仍能编译（使用重导出）
cat > /tmp/test_backward_compat.rs << 'EOF'
// 测试向后兼容 - 应能通过重导出使用旧路径
use cis_core::types::TaskLevel;  // 重导出路径
use cis_core::Memory;            // trait 重导出

fn main() {
    let level = TaskLevel::Mechanical { retry: 3 };
    println!("{:?}", level);
}
EOF

# 编译测试（应在 cis-core 中提供重导出）
```

### 5. 性能回归测试

```bash
# 运行基准测试
cargo bench -p cis-core

# 对比重构前后性能（如果有历史数据）
```

### 6. 检查警告

```bash
# 检查编译警告
cargo check --workspace 2>&1 | grep -i warning | head -50

# 检查未使用代码
cargo clippy --workspace -- -W unused
```

## 验收标准

- [ ] cis-common workspace 编译通过
- [ ] cis-core 编译通过
- [ ] 完整 workspace 编译通过
- [ ] 所有测试通过
- [ ] 向后兼容层工作正常
- [ ] 无关键编译警告

## 依赖

- Task 3.4 (更新导入语句)

## 阻塞

- Task 4.x (ZeroClaw 集成)
- Task 5.x (测试)

---
