# WASM 沙箱 v2 快速开始指南

## 5 分钟上手

### 1. 启用 v2 功能

```bash
# 在 Cargo.toml 中确保启用 wasm-v2
cargo build --package cis-core --features wasm-v2
```

### 2. 基本使用

```rust
use cis_core::wasm::v2::SecureSandbox;

// 创建沙箱
let sandbox = SecureSandbox::with_default_config()?;

// 验证系统调用
assert!(sandbox.validate_syscall(libc::SYS_READ).is_ok());
```

### 3. 自定义配置

```rust
use cis_core::wasm::v2::SecureSandboxConfig;
use std::path::PathBuf;

let config = SecureSandboxConfig {
    max_memory: 256 * 1024 * 1024,  // 256 MB
    max_execution_time_ms: 10000,      // 10 秒
    max_fuel: 1_000_000_000,          // 10 亿燃料
    enable_fuel: true,
    allowed_directories: vec![
        PathBuf::from("/tmp/skills"),
    ],
    allow_network: false,
    max_open_files: 5,
};

let sandbox = SecureSandbox::new(config)?;
```

### 4. 资源监控

```rust
let monitor = sandbox.monitor();

// 使用资源
monitor.check_memory(1024)?;
monitor.record_file_open("/tmp/test".into())?;

// 生成报告
let report = monitor.generate_report();
println!("{}", report.to_string());
```

## 常见用例

### 场景 1: 生产环境（严格限制）

```rust
let config = SecureSandboxConfig {
    max_memory: 128 * 1024 * 1024,  // 128 MB
    max_execution_time_ms: 5000,       // 5 秒
    max_fuel: 5_000_000_000,          // 50 亿燃料
    enable_fuel: true,
    allowed_directories: vec![],
    allow_network: false,
    max_open_files: 3,
};
```

### 场景 2: 开发环境（宽松限制）

```rust
let config = SecureSandboxConfig {
    max_memory: 1024 * 1024 * 1024, // 1 GB
    max_execution_time_ms: 300000,    // 5 分钟
    max_fuel: 100_000_000_000,       // 1000 亿燃料
    enable_fuel: true,
    allowed_directories: vec![
        PathBuf::from("/tmp/dev"),
    ],
    allow_network: true,  // 开发时允许网络
    max_open_files: 20,
};
```

### 场景 3: 测试环境（平衡）

```rust
let config = SecureSandboxConfig {
    max_memory: 512 * 1024 * 1024,  // 512 MB
    max_execution_time_ms: 30000,      // 30 秒
    max_fuel: 10_000_000_000,         // 100 亿燃料
    enable_fuel: true,
    allowed_directories: vec![],
    allow_network: false,
    max_open_files: 10,
};
```

## 安全检查清单

在部署前确保：

- [ ] 系统调用白名单已配置
- [ ] 内存限制已设置
- [ ] 执行超时已启用
- [ ] 燃料限制已启用
- [ ] 网络访问已正确配置（生产环境应为 false）
- [ ] 文件句柄限制已设置
- [ ] 允许的目录已配置
- [ ] 资源监控已启用
- [ ] 违规日志已配置
- [ ] 测试已通过

## 调试技巧

### 查看详细日志

```rust
use tracing::{info, warn};

// 记录所有系统调用
info!("Validating syscall: {}", syscall_number);

// 记录违规
if monitor.has_violations() {
    for violation in monitor.get_violations() {
        warn!("Violation: {:?}", violation);
    }
}
```

### 检查资源使用

```rust
let usage = monitor.get_usage();
println!("Memory: {} / {} bytes",
    usage.memory_used,
    monitor.get_limits().max_memory_bytes
);

let utilization = monitor.get_utilization();
println!("Memory utilization: {:.1}%",
    utilization.memory_utilization * 100.0
);
```

### 生成报告

```rust
let report = monitor.generate_report();

// 打印人类可读的报告
println!("{}", report.to_string());

// 检查是否成功
if !report.is_success() {
    eprintln!("Execution had {} violations", report.total_violations);
}
```

## 故障排查

### 编译错误

```bash
# 确保启用了正确的 feature
cargo build --features wasm-v2

# 清理并重新构建
cargo clean
cargo build --features wasm-v2
```

### 运行时错误

```rust
// 添加错误处理
match sandbox.validate_syscall(syscall) {
    Ok(_) => {},
    Err(e) => {
        eprintln!("Syscall validation failed: {}", e);
        // 记录违规
    }
}
```

### 性能问题

```rust
// 启用燃料监控
let limiter = FuelLimiter::new(fuel_config)?;
let stats = limiter.get_stats(&store)?;

println!("Fuel consumed: {}", stats.consumed_fuel);
println!("Consumption rate: {:.2} fuel/sec", stats.consumption_rate);
```

## 测试

### 运行所有测试

```bash
cargo test --package cis-core --features wasm-v2
```

### 运行安全测试

```bash
cargo test --package cis-core --features wasm-v2 --test security_tests
```

### 运行集成测试

```bash
cargo test --package cis-core --features wasm-v2 --test integration_tests
```

### 运行特定测试

```bash
# 系统调用测试
cargo test --package cis-core --features wasm-v2 test_syscall_whitelist

# 内存限制测试
cargo test --package cis-core --features wasm-v2 test_memory_limit

# 燃料测试
cargo test --package cis-core --features wasm-v2 test_fuel_config_validation
```

## 性能优化建议

1. **批量操作**: 合并小内存分配
2. **重用实例**: 避免频繁创建沙箱
3. **调整限制**: 根据实际需求配置
4. **监控频率**: 避免过于频繁的查询

## 下一步

- 阅读完整文档: `cis-core/src/wasm/v2/README.md`
- 查看示例代码: `cis-core/wasm/tests/integration_tests.rs`
- 了解系统调用白名单: `cis-core/src/wasm/design/syscall_whitelist.yaml`

## 获取帮助

- 查看文档: `/docs/`
- 运行示例: `cargo test --package cis-core --features wasm-v2 --test integration_tests`
- 查看源码: `cis-core/src/wasm/v2/`
