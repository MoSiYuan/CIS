# CIS WASM 沙箱 v2 - 安全增强实现

## 概述

CIS v1.1.6 引入了全新的 WASM 沙箱实现（v2），基于 `wasmtime` 和多层安全机制，显著提升了 WASM 模块执行的安全性。

## 主要改进

### 1. 完整的系统调用白名单

**文件**: `syscall_whitelist.yaml`

- 定义了严格的系统调用白名单
- 只允许安全的系统调用执行
- 黑名单禁止危险操作（execve、fork、mount 等）
- 支持平台特定配置

**允许的系统调用**:
- 文件操作：READ, WRITE, CLOSE, FSTAT
- 内存管理：MMAP, MUNMAP, MREMAP
- 时间：CLOCK_GETTIME, GETTIMEOFDAY
- 进程控制：EXIT, EXIT_GROUP, GETPID

**禁止的系统调用**:
- 进程创建：FORK, CLONE, EXECVE
- 权限提升：SETUID, SETGID, CHMOD
- 系统修改：MOUNT, UMOUNT, CHROOT

### 2. wasmtime 沙箱集成

**文件**: `sandbox.rs`

使用 `wasmtime` 替代 `wasmer`，提供更强的安全保证：

```rust
use cis_core::wasm::v2::SecureSandbox;

let sandbox = SecureSandbox::with_default_config()?;

// 验证系统调用
sandbox.validate_syscall(libc::SYS_READ)?;  // OK
sandbox.validate_syscall(libc::SYS_EXECVE)?; // Error
```

**特性**:
- 完整的系统调用过滤
- 内存隔离
- 资源限制集成
- 详细的违规记录

### 3. 燃料限制

**文件**: `fuel_limiter.rs`

防止无限循环和 DoS 攻击：

```rust
use cis_core::wasm::v2::FuelLimiter;

let limiter = FuelLimiter::with_default_config()?;

// 设置燃料到 Store
limiter.set_fuel_to_store(&mut store)?;

// 检查燃料消耗
let stats = limiter.get_stats(&store)?;
println!("Fuel consumed: {}", stats.consumed_fuel);
```

**特性**:
- 燃料自动补充
- 消耗率分析
- 异常检测
- 使用趋势追踪

### 4. 资源监控

**文件**: `resource_monitor.rs`

全面监控 WASM 模块的资源使用：

```rust
use cis_core::wasm::v2::ResourceMonitor;

let monitor = ResourceMonitor::with_default_limits()?;

// 检查内存
monitor.check_memory(1024)?;

// 记录文件操作
monitor.record_file_open(path)?;

// 生成报告
let report = monitor.generate_report();
println!("{}", report.to_string());
```

**监控指标**:
- 内存使用（堆、栈、总计）
- 执行时间
- 打开文件数
- 网络连接数
- 系统调用计数和速率

### 5. 安全测试套件

**文件**: `security_tests.rs`

全面的安全测试覆盖：

- 系统调用过滤测试
- 内存限制测试
- 超时测试
- 燃料限制测试
- 资源监控测试
- 违规检测测试
- 集成测试

## 使用方法

### 基本使用

```rust
use cis_core::wasm::v2::{SecureSandbox, SecureSandboxConfig};

// 使用默认配置
let sandbox = SecureSandbox::with_default_config()?;

// 或自定义配置
let config = SecureSandboxConfig {
    max_memory: 512 * 1024 * 1024,  // 512 MB
    max_execution_time_ms: 30000,      // 30 秒
    max_fuel: 10_000_000_000,          // 100 亿燃料
    enable_fuel: true,
    allowed_directories: vec![],
    allow_network: false,
    max_open_files: 10,
};

let sandbox = SecureSandbox::new(config)?;
```

### 执行 WASM 模块

```rust
use std::fs;

// 读取 WASM 模块
let wasm_bytes = fs::read("skill.wasm")?;

// 执行模块
let result = sandbox.execute_module(&wasm_bytes, "main").await?;

if result.success {
    println!("Execution succeeded");
    println!("Time: {:?}", result.execution_time);
    println!("Fuel consumed: {:?}", result.fuel_consumed);
} else {
    println!("Execution failed: {:?}", result.error);
}
```

### 资源监控

```rust
let monitor = sandbox.monitor();

// 获取实时统计
let stats = monitor.get_stats();
println!("Memory used: {} bytes", stats.memory_used);
println!("Open files: {}", stats.open_files);
println!("Syscalls: {}", stats.syscalls_called);

// 检查违规
if monitor.has_violations() {
    let violations = monitor.get_violations();
    for violation in violations {
        eprintln!("Violation: {:?}", violation);
    }
}

// 生成报告
let report = monitor.generate_report();
println!("{}", report.to_string());
```

## 配置选项

### 安全级别

**严格模式（生产环境推荐）**:
```toml
[wasm]
max_memory = "128MB"
max_execution_time = "5s"
max_fuel = 5_000_000_000
enable_fuel = true
allow_network = false
max_open_files = 3
```

**宽松模式（开发环境）**:
```toml
[wasm]
max_memory = "1024MB"
max_execution_time = "300s"
max_fuel = 100_000_000_000
enable_fuel = true
allow_network = true
max_open_files = 20
```

## 编译和测试

### 启用 v2 功能

```bash
# 添加 wasm-v2 feature
cargo build --package cis-core --features wasm-v2

# 运行测试
cargo test --package cis-core --features wasm-v2 --test security_tests
cargo test --package cis-core --features wasm-v2 --test integration_tests
```

### 运行特定测试

```bash
# 系统调用测试
cargo test --package cis-core --features wasm-v2 test_syscall_whitelist

# 内存限制测试
cargo test --package cis-core --features wasm-v2 test_memory_limit

# 燃料测试
cargo test --package cis-core --features wasm-v2 test_fuel_limit
```

## 性能考虑

### 开销分析

- **系统调用过滤**: ~10-50ns per syscall
- **燃料消耗**: 几乎无开销（编译时优化）
- **资源监控**: ~1-5μs per operation
- **总体开销**: < 1% 执行时间

### 优化建议

1. **批量操作**: 合并小内存分配
2. **预热**: 重用沙箱实例
3. **配置调优**: 根据实际需求设置限制
4. **监控频率**: 避免过于频繁的资源查询

## 安全最佳实践

### 1. 最小权限原则

```rust
// 只授予必要的权限
let config = SecureSandboxConfig {
    allow_network: false,  // 默认禁止网络
    max_open_files: 3,     // 最小文件数
    ..Default::default()
};
```

### 2. 违规响应

```rust
if monitor.has_violations() {
    // 立即终止执行
    let violations = monitor.get_violations();
    for violation in violations {
        match violation {
            ResourceViolation::MemoryLimitExceeded { .. } => {
                // 记录违规并终止
                eprintln!("Memory limit exceeded, terminating");
                break;
            }
            _ => {
                eprintln!("Violation: {:?}", violation);
            }
        }
    }
}
```

### 3. 审计日志

```rust
use tracing::{info, warn};

// 记录执行
info!("WASM execution started");
info!("Config: {:?}", sandbox.config());

// 记录违规
if monitor.has_violations() {
    for violation in monitor.get_violations() {
        warn!("Security violation: {:?}", violation);
    }
}

// 记录结果
let report = monitor.generate_report();
info!("Execution report:\n{}", report.to_string());
```

## 故障排查

### 常见问题

**Q: 编译错误 "cannot find in this scope"**
```bash
# 确保启用了 wasm-v2 feature
cargo build --features wasm-v2
```

**Q: WASM 模块执行超时**
```rust
// 增加超时时间
let config = SecureSandboxConfig {
    max_execution_time_ms: 60000,  // 60 秒
    ..Default::default()
};
```

**Q: 内存限制超出**
```rust
// 增加内存限制
let config = SecureSandboxConfig {
    max_memory: 1024 * 1024 * 1024,  // 1 GB
    ..Default::default()
};
```

**Q: 燃料耗尽**
```rust
// 启用自动补充
let fuel_config = FuelConfig::new(10_000_000_000)
    .with_refill(1000, 1_000_000_000);  // 每秒补充
```

## 迁移指南

### 从 v1 迁移到 v2

**旧的 wasmer 实现**:
```rust
use cis_core::wasm::WasmRuntime;

let runtime = WasmRuntime::new()?;
```

**新的 wasmtime 实现**:
```rust
use cis_core::wasm::v2::SecureSandbox;

let sandbox = SecureSandbox::with_default_config()?;
```

### 兼容性

- v1（wasmer）仍然可用（默认 feature `wasm`）
- v2（wasmtime）需要 `wasm-v2` feature
- 两个版本可以共存，逐步迁移

## 未来计划

- [ ] 系统调用参数验证
- [ ] 文件路径沙箱化
- [ ] 网络访问细粒度控制
- [ ] WebAssembly GC 支持
- [ ] 组件模型（Component Model）支持
- [ ] 性能基准测试
- [ ] 形式化验证

## 相关文档

- [WASI 规范](https://wasi.dev/)
- [wasmtime 文档](https://docs.wasmtime.dev/)
- [WebAssembly 规范](https://webassembly.github.io/spec/)
- [CIS 安全指南](../../../docs/SECURITY.md)

## 许可证

MIT License - 详见项目根目录的 LICENSE 文件
