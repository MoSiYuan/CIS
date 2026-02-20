# TASK 4.2: ZeroClaw 端到端验证

> **Phase**: 4 - ZeroClaw 兼容
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 8-9

---

## 任务概述

与 ZeroClaw 项目进行端到端集成测试，验证 CIS v1.2.0 可作为其 backend 正常工作。

## 工作内容

### 1. 准备测试环境

```bash
# 克隆 ZeroClaw 仓库
git clone https://github.com/cis-projects/zeroclaw.git /tmp/zeroclaw-test

# 创建 CIS v1.2.0 测试分支
cd /tmp/zeroclaw-test
git checkout -b test-cis-v1.2.0-backend
```

### 2. 修改 ZeroClaw 使用 CIS backend

```rust
// zeroclaw/src/main.rs (修改后)
#[cfg(feature = "cis-backend")]
use cis_core::zeroclaw::{ZeroclawMemoryAdapter, ZeroclawDelegateAdapter};

#[cfg(feature = "cis-backend")]
async fn setup_cis_backend() -> (impl Memory, impl DelegateTool) {
    let runtime = cis_core::Runtime::builder()
        .with_storage(cis_storage::RocksDbStorage::new("./data"))
        .with_memory(cis_memory::CISMemory::new(...))
        .build()
        .expect("Failed to build CIS runtime");
    
    let memory = ZeroclawMemoryAdapter::new(runtime.memory().clone());
    let delegate = ZeroclawDelegateAdapter::new(Arc::new(runtime));
    
    (memory, delegate)
}
```

### 3. 执行功能测试

| 测试项 | 期望结果 |
|--------|----------|
| 记忆存储 | 通过 CIS Memory 存储对话历史 |
| 记忆召回 | 从 CIS Memory 检索相关记忆 |
| 任务委托 | DelegateTool 调用 CIS 调度器 |
| Session 隔离 | 不同 Session 记忆互不可见 |
| 幻觉过滤 | CIS 的四层过滤机制生效 |

### 4. 性能对比测试

```rust
#[test]
fn benchmark_memory_operations() {
    // 对比 ZeroClaw 原生 vs CIS backend
    let native_times = benchmark_native();
    let cis_times = benchmark_cis_backend();
    
    // CIS backend 不应比原生慢超过 20%
    assert!(cis_times.avg() < native_times.avg() * 1.2);
}
```

### 5. 回归测试

- [ ] ZeroClaw 所有现有测试通过
- [ ] CIS backend 不破坏原有功能
- [ ] Feature flag 可正常切换

## 验收标准

- [ ] ZeroClaw 可使用 CIS 作为 backend 运行
- [ ] 所有功能测试通过
- [ ] 性能损失 < 20%
- [ ] ZeroClaw 回归测试通过
- [ ] 集成测试文档完整

## 依赖

- Task 4.1 (适配层实现)

## 阻塞

- Task 5.1 (测试框架搭建)

---
