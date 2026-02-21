# TASK 3.4: 更新依赖模块导入语句

> **Phase**: 3 - cis-core 重构
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **负责人**: TBD
> **周期**: Week 6-7

---

## 任务概述

更新 cis-core 内部模块的导入语句，从 `crate::` 改为 `cis_*` crate 引用。

## 工作内容

### 1. 批量替换导入语句

```bash
# 创建替换脚本
#!/bin/bash
# update_imports.sh

# types 模块
find cis-core/src -name "*.rs" -exec sed -i 's/use crate::types::/use cis_types::/g' {} \;
find cis-core/src -name "*.rs" -exec sed -i 's/crate::types::/cis_types::/g' {} \;

# traits 模块
find cis-core/src -name "*.rs" -exec sed -i 's/use crate::traits::/use cis_traits::/g' {} \;
find cis-core/src -name "*.rs" -exec sed -i 's/crate::traits::/cis_traits::/g' {} \;

# storage 模块
find cis-core/src -name "*.rs" -exec sed -i 's/use crate::storage::/use cis_storage::/g' {} \;
find cis-core/src -name "*.rs" -exec sed -i 's/crate::storage::/cis_storage::/g' {} \;

# memory 模块
find cis-core/src -name "*.rs" -exec sed -i 's/use crate::memory::/use cis_memory::/g' {} \;
find cis-core/src -name "*.rs" -exec sed -i 's/crate::memory::/cis_memory::/g' {} \;

# scheduler 模块
find cis-core/src -name "*.rs" -exec sed -i 's/use crate::scheduler::/use cis_scheduler::/g' {} \;
find cis-core/src -name "*.rs" -exec sed -i 's/crate::scheduler::/cis_scheduler::/g' {} \;

# vector 模块
find cis-core/src -name "*.rs" -exec sed -i 's/use crate::vector::/use cis_vector::/g' {} \;
find cis-core/src -name "*.rs" -exec sed -i 's/crate::vector::/cis_vector::/g' {} \;

# p2p 模块
find cis-core/src -name "*.rs" -exec sed -i 's/use crate::p2p::/use cis_p2p::/g' {} \;
find cis-core/src -name "*.rs" -exec sed -i 's/crate::p2p::/cis_p2p::/g' {} \;
```

### 2. 手动检查复杂引用

```bash
# 查找可能遗漏的引用
grep -r "crate::types" cis-core/src/ || echo "OK: No crate::types references"
grep -r "crate::storage" cis-core/src/ || echo "OK: No crate::storage references"
grep -r "crate::memory" cis-core/src/ || echo "OK: No crate::memory references"
grep -r "crate::scheduler" cis-core/src/ || echo "OK: No crate::scheduler references"
```

### 3. 更新测试文件

```bash
# 更新测试中的导入
find cis-core/tests -name "*.rs" -exec sed -i 's/use cis_core::types/use cis_types/g' {} \;
find cis-core/tests -name "*.rs" -exec sed -i 's/use cis_core::storage/use cis_storage/g' {} \;
find cis-core/tests -name "*.rs" -exec sed -i 's/use cis_core::memory/use cis_memory/g' {} \;
```

### 4. 更新示例代码

```bash
# 更新 examples 目录
find examples -name "*.rs" -exec sed -i 's/use cis_core::types/use cis_types/g' {} \;
```

### 5. 修复编译错误

```bash
# 逐步编译并修复错误
cargo check -p cis-core 2>&1 | head -100

# 常见错误处理:
# 1. 类型不匹配 → 检查类型映射
# 2. 方法不存在 → 检查 trait 实现
# 3. 路径错误 → 修正 use 语句
```

## 验收标准

- [ ] 所有 `crate::types` 引用已替换
- [ ] 所有 `crate::storage` 引用已替换
- [ ] 所有 `crate::memory` 引用已替换
- [ ] 所有 `crate::scheduler` 引用已替换
- [ ] 所有测试文件已更新
- [ ] 编译无错误

## 依赖

- Task 3.3 (移除模块)

## 阻塞

- Task 3.5 (测试编译)

---
