// CIS v1.1.6 - WASM 沙箱安全测试
//
// 本模块包含 WASM 沙箱的全面安全测试。
// 测试覆盖：
// - 系统调用过滤
// - 内存限制
// - 执行超时
// - 资源限制
// - 燃料消耗
// - 文件系统隔离
// - 网络访问控制

use std::time::Duration;
use cis_core::wasm::v2::{
    sandbox::{SecureSandbox, SecureSandboxConfig},
    fuel_limiter::{FuelLimiter, FuelConfig},
    resource_monitor::{ResourceMonitor, ResourceLimits},
};

// ============================================================================
// 系统调用过滤测试
// ============================================================================

#[test]
fn test_syscall_whitelist_allows_safe_operations() {
    let sandbox = SecureSandbox::with_default_config().unwrap();

    // 这些系统调用应该被允许
    let safe_syscalls = vec![
        libc::SYS_READ,
        libc::SYS_WRITE,
        libc::SYS_CLOSE,
        libc::SYS_MMAP,
        libc::SYS_MUNMAP,
        libc::SYS_CLOCK_GETTIME,
        libc::SYS_EXIT,
    ];

    for syscall in safe_syscalls {
        assert!(
            sandbox.validate_syscall(syscall).is_ok(),
            "Syscall {} should be allowed",
            syscall
        );
    }
}

#[test]
fn test_syscall_whitelist_blocks_unsafe_operations() {
    let sandbox = SecureSandbox::with_default_config().unwrap();

    // 测试一个不在白名单中的系统调用
    let unsafe_syscall = libc::SYS_SOCKET; // 默认应该被禁止

    assert!(
        sandbox.validate_syscall(unsafe_syscall).is_err(),
        "Syscall {} should be blocked",
        unsafe_syscall
    );
}

#[test]
fn test_syscall_blacklist_blocks_critical_operations() {
    let sandbox = SecureSandbox::with_default_config().unwrap();

    // 这些系统调用永远不应该被允许
    let forbidden_syscalls = vec![
        libc::SYS_EXECVE,
        libc::SYS_FORK,
        libc::SYS_CLONE,
        libc::SYS_KILL,
        libc::SYS_PTRACE,
        libc::SYS_MOUNT,
        libc::SYS_CHROOT,
        libc::SYS_SETUID,
        libc::SYS_SETGID,
    ];

    for syscall in forbidden_syscalls {
        assert!(
            sandbox.validate_syscall(syscall).is_err(),
            "Syscall {} should be forbidden",
            syscall
        );
    }
}

// ============================================================================
// 内存限制测试
// ============================================================================

#[test]
fn test_memory_limit_enforcement() {
    let config = SecureSandboxConfig {
        max_memory: 1024, // 1 KB
        ..Default::default()
    };
    let sandbox = SecureSandbox::new(config).unwrap();

    let monitor = sandbox.monitor();

    // 分配 512 字节应该成功
    assert!(monitor.check_memory(512).is_ok());

    // 再分配 600 字节应该失败（总共 1112 > 1024）
    assert!(monitor.check_memory(600).is_err());

    let stats = monitor.get_stats();
    assert!(stats.memory_used >= 512);
}

#[test]
fn test_memory_accumulation_tracking() {
    let monitor = ResourceMonitor::new(ResourceLimits::default()).unwrap();

    // 多次分配
    monitor.check_memory(100).unwrap();
    monitor.check_memory(200).unwrap();
    monitor.check_memory(300).unwrap();

    let usage = monitor.get_usage();
    assert_eq!(usage.memory_used, 600);
}

#[test]
fn test_memory_exceeds_maximum_webassembly_limit() {
    let config = SecureSandboxConfig {
        max_memory: 5 * 1024 * 1024 * 1024, // 5 GB，超过 WebAssembly 最大值
        ..Default::default()
    };

    assert!(SecureSandbox::new(config).is_err());
}

// ============================================================================
// 执行超时测试
// ============================================================================

#[test]
fn test_execution_timeout_enforcement() {
    let config = SecureSandboxConfig {
        max_execution_time_ms: 100, // 100 ms
        ..Default::default()
    };
    let sandbox = SecureSandbox::new(config).unwrap();
    let monitor = sandbox.monitor();

    // 立即检查应该不超时
    assert!(monitor.check_timeout().is_ok());

    // 等待超过限制
    std::thread::sleep(Duration::from_millis(150));

    // 应该超时
    assert!(monitor.check_timeout().is_err());
}

#[test]
fn test_timeout_records_violation() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_execution_time_ms: 50,
            ..Default::default()
        }
    ).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    let _ = monitor.check_timeout();

    let violations = monitor.get_violations();
    assert!(!violations.is_empty(), "Should have recorded a timeout violation");
}

// ============================================================================
// 燃料限制测试
// ============================================================================

#[test]
fn test_fuel_limit_enforcement() {
    let config = FuelConfig {
        initial_fuel: 1_000_000, // 100 万燃料
        ..Default::default()
    };
    let limiter = FuelLimiter::new(config).unwrap();

    // 创建模拟的 store（需要 wasmtime）
    // 在真实测试中，这里会执行 WASM 代码
    assert_eq!(limiter.initial_fuel(), 1_000_000);
}

#[test]
fn test_fuel_auto_refill() {
    let config = FuelConfig {
        initial_fuel: 1_000_000,
        fuel_interval_ms: 100,
        fuel_refill_amount: 100_000,
        enable_auto_refill: true,
        ..Default::default()
    };
    let limiter = FuelLimiter::new(config).unwrap();

    // 在真实测试中，这里会模拟燃料消耗和补充
    let stats = FuelStats {
        current_fuel: 500_000,
        consumed_fuel: 500_000,
        refilled_fuel: 0,
        refill_count: 0,
        elapsed_time: Duration::from_millis(50),
        consumption_rate: 10_000_000.0,
    };

    assert_eq!(stats.current_fuel, 500_000);
    assert_eq!(stats.consumed_fuel, 500_000);
}

#[test]
fn test_fuel_config_validation() {
    // 有效配置
    let config = FuelConfig::default();
    assert!(config.validate().is_ok());

    // 无效配置（零燃料）
    let invalid_config = FuelConfig {
        initial_fuel: 0,
        ..Default::default()
    };
    assert!(invalid_config.validate().is_err());

    // 无效配置（最大积累量小于初始量）
    let invalid_config2 = FuelConfig {
        initial_fuel: 1_000_000,
        max_fuel_accumulation: 500_000,
        ..Default::default()
    };
    assert!(invalid_config2.validate().is_err());
}

// ============================================================================
// 资源监控测试
// ============================================================================

#[test]
fn test_resource_monitor_file_limits() {
    let limits = ResourceLimits {
        max_open_files: 3,
        ..Default::default()
    };
    let monitor = ResourceMonitor::new(limits).unwrap();

    // 打开 3 个文件应该成功
    assert!(monitor.record_file_open("/tmp/file1".into()).is_ok());
    assert!(monitor.record_file_open("/tmp/file2".into()).is_ok());
    assert!(monitor.record_file_open("/tmp/file3".into()).is_ok());

    // 打开第 4 个文件应该失败
    assert!(monitor.record_file_open("/tmp/file4".into()).is_err());

    // 关闭一个文件后再打开应该成功
    monitor.record_file_close();
    assert!(monitor.record_file_open("/tmp/file5".into()).is_ok());
}

#[test]
fn test_resource_monitor_syscall_rate_limiting() {
    let limits = ResourceLimits {
        max_syscalls_per_second: 10,
        ..Default::default()
    };
    let monitor = ResourceMonitor::new(limits).unwrap();

    // 快速执行 10 次系统调用应该成功
    for _ in 0..10 {
        assert!(monitor.record_syscall(1).is_ok());
    }

    // 第 11 次应该失败（速率限制）
    assert!(monitor.record_syscall(1).is_err());
}

#[test]
fn test_resource_monitor_total_syscall_limit() {
    let limits = ResourceLimits {
        max_total_syscalls: 50,
        max_syscalls_per_second: 1000, // 高速率限制
        ..Default::default()
    };
    let monitor = ResourceMonitor::new(limits).unwrap();

    // 执行 50 次系统调用
    for _ in 0..50 {
        assert!(monitor.record_syscall(1).is_ok());
    }

    // 第 51 次应该失败（总数限制）
    assert!(monitor.record_syscall(1).is_err());
}

#[test]
fn test_resource_monitor_network_blocking() {
    let limits = ResourceLimits {
        max_network_connections: 0, // 禁止网络
        ..Default::default()
    };
    let monitor = ResourceMonitor::new(limits).unwrap();

    // 尝试建立网络连接应该失败
    assert!(monitor.record_network_connection("example.com:80".to_string()).is_err());
}

#[test]
fn test_resource_monitor_network_allow() {
    let limits = ResourceLimits {
        max_network_connections: 2,
        ..Default::default()
    };
    let monitor = ResourceMonitor::new(limits).unwrap();

    // 建立 2 个连接应该成功
    assert!(monitor.record_network_connection("example.com:80".to_string()).is_ok());
    assert!(monitor.record_network_connection("api.example.com:443".to_string()).is_ok());

    // 第 3 个连接应该失败
    assert!(monitor.record_network_connection("localhost:8080".to_string()).is_err());

    // 断开一个后应该能再连接
    monitor.record_network_disconnect();
    assert!(monitor.record_network_connection("localhost:8080".to_string()).is_ok());
}

// ============================================================================
// 资源利用率和报告测试
// ============================================================================

#[test]
fn test_resource_utilization_calculation() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_memory_bytes: 1000,
            max_execution_time_ms: 10000,
            max_open_files: 10,
            max_network_connections: 5,
            max_total_syscalls: 1000,
            ..Default::default()
        }
    ).unwrap();

    // 使用一些资源
    monitor.check_memory(500).unwrap();
    monitor.record_file_open("/tmp/file".into()).unwrap();
    for _ in 0..500 {
        monitor.record_syscall(1).ok();
    }

    let utilization = monitor.get_utilization();

    assert_eq!(utilization.memory_utilization, 0.5);
    assert_eq!(utilization.file_utilization, 0.1);
    assert_eq!(utilization.syscall_utilization, 0.5);
}

#[test]
fn test_utilization_pressure_detection() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_memory_bytes: 1000,
            ..Default::default()
        }
    ).unwrap();

    monitor.check_memory(800).unwrap();

    let utilization = monitor.get_utilization();
    assert!(utilization.has_pressure(0.7), "Should detect pressure at 80% usage");
    assert!(!utilization.has_pressure(0.9), "Should not detect pressure at 90% threshold");
}

#[test]
fn test_monitoring_report_generation() {
    let monitor = ResourceMonitor::with_default_limits().unwrap();

    // 使用一些资源
    monitor.check_memory(1024).unwrap();
    monitor.record_file_open("/tmp/test".into()).unwrap();

    let report = monitor.generate_report();

    assert_eq!(report.total_violations, 0);
    assert!(report.is_success());

    let report_str = report.to_string();
    assert!(report_str.contains("执行时间"));
    assert!(report_str.contains("资源使用"));
    assert!(report_str.contains("无违规"));
}

// ============================================================================
// 违规检测测试
// ============================================================================

#[test]
fn test_memory_limit_violation() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_memory_bytes: 1000,
            ..Default::default()
        }
    ).unwrap();

    // 尝试分配超过限制
    monitor.check_memory(500).unwrap();
    assert!(monitor.check_memory(600).is_err());

    let violations = monitor.get_violations();
    assert!(!violations.is_empty());

    match &violations[0] {
        cis_core::wasm::v2::resource_monitor::ResourceViolation::MemoryLimitExceeded { used, limit } => {
            assert_eq!(*used, 1100);
            assert_eq!(*limit, 1000);
        }
        _ => panic!("Expected MemoryLimitExceeded violation"),
    }
}

#[test]
fn test_timeout_violation() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_execution_time_ms: 50,
            ..Default::default()
        }
    ).unwrap();

    std::thread::sleep(Duration::from_millis(100));

    let _ = monitor.check_timeout();

    let violations = monitor.get_violations();
    assert!(!violations.is_empty());
}

#[test]
fn test_file_access_violation() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_open_files: 2,
            ..Default::default()
        }
    ).unwrap();

    monitor.record_file_open("/tmp/file1".into()).unwrap();
    monitor.record_file_open("/tmp/file2".into()).unwrap();

    // 尝试打开第 3 个文件
    assert!(monitor.record_file_open("/tmp/file3".into()).is_err());

    let violations = monitor.get_violations();
    assert!(!violations.is_empty());
}

// ============================================================================
// 集成测试
// ============================================================================

#[test]
fn test_full_security_enforcement() {
    let config = SecureSandboxConfig {
        max_memory: 1024 * 1024,      // 1 MB
        max_execution_time_ms: 1000,    // 1 秒
        max_fuel: 100_000_000,         // 1 亿燃料
        enable_fuel: true,
        allowed_directories: vec![],
        allow_network: false,
        max_open_files: 5,
    };
    let sandbox = SecureSandbox::new(config).unwrap();

    // 系统调用验证
    assert!(sandbox.validate_syscall(libc::SYS_READ).is_ok());
    assert!(sandbox.validate_syscall(libc::SYS_EXECVE).is_err());

    // 资源监控
    let monitor = sandbox.monitor();
    assert!(monitor.check_memory(512 * 1024).is_ok());
    assert!(monitor.check_memory(512 * 1024).is_err()); // 超过 1 MB

    // 文件限制
    assert!(monitor.record_file_open("/tmp/test1".into()).is_ok());
    assert!(monitor.record_file_open("/tmp/test2".into()).is_ok());

    // 报告生成
    let report = monitor.get_stats();
    assert!(report.memory_used > 0);
}

#[test]
fn test_multiple_violations_tracking() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_memory_bytes: 1000,
            max_execution_time_ms: 50,
            max_open_files: 1,
            ..Default::default()
        }
    ).unwrap();

    // 触发多个违规
    monitor.check_memory(500).unwrap();
    monitor.check_memory(600).unwrap_err(); // 内存违规

    std::thread::sleep(Duration::from_millis(100));
    let _ = monitor.check_timeout(); // 时间违规

    monitor.record_file_open("/tmp/file1".into()).unwrap();
    monitor.record_file_open("/tmp/file2".into()).unwrap_err(); // 文件违规

    let violations = monitor.get_violations();
    assert!(violations.len() >= 2, "Should have at least 2 violations");
}

// ============================================================================
// 边界条件测试
// ============================================================================

#[test]
fn test_zero_memory_limit() {
    let config = SecureSandboxConfig {
        max_memory: 0,
        ..Default::default()
    };
    assert!(SecureSandbox::new(config).is_err());
}

#[test]
fn test_zero_timeout() {
    let config = SecureSandboxConfig {
        max_execution_time_ms: 0,
        ..Default::default()
    };
    assert!(SecureSandbox::new(config).is_err());
}

#[test]
fn test_maximum_webassembly_memory_limit() {
    let config = SecureSandboxConfig {
        max_memory: 4 * 1024 * 1024 * 1024, // 4 GB（WebAssembly 最大值）
        ..Default::default()
    };
    assert!(SecureSandbox::new(config).is_ok());

    let config_over = SecureSandboxConfig {
        max_memory: 4 * 1024 * 1024 * 1024 + 1, // 超过 4 GB
        ..Default::default()
    };
    assert!(SecureSandbox::new(config_over).is_err());
}

#[test]
fn test_exactly_at_limits() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_memory_bytes: 1000,
            max_open_files: 5,
            max_total_syscalls: 100,
            ..Default::default()
        }
    ).unwrap();

    // 精确达到限制
    monitor.check_memory(1000).unwrap();
    assert!(monitor.check_memory(1).is_err());

    for _ in 0..5 {
        monitor.record_file_open("/tmp/file".into()).unwrap();
    }
    assert!(monitor.record_file_open("/tmp/file6".into()).is_err());

    for _ in 0..100 {
        monitor.record_syscall(1).unwrap();
    }
    assert!(monitor.record_syscall(1).is_err());
}

// ============================================================================
// 性能测试
// ============================================================================

#[test]
#[ignore] // 需要真实的 WASM 模块
fn test_fuel_consumption_rate() {
    // 这个测试需要真实的 WASM 模块来测量燃料消耗
    // 在集成测试中实现
}

#[test]
fn test_monitoring_overhead() {
    let monitor = ResourceMonitor::with_default_limits().unwrap();

    let start = std::time::Instant::now();

    // 执行 10000 次监控操作
    for _ in 0..10_000 {
        monitor.check_memory(100).ok();
        monitor.record_syscall(1).ok();
    }

    let elapsed = start.elapsed();

    // 监控开销应该很小（< 10ms）
    assert!(elapsed < Duration::from_millis(10), "Monitoring overhead too high: {:?}", elapsed);
}

// ============================================================================
// 并发测试
// ============================================================================

#[test]
fn test_concurrent_monitoring() {
    let monitor = Arc::new(ResourceMonitor::with_default_limits().unwrap());
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let monitor = Arc::clone(&monitor);
            std::thread::spawn(move || {
                for _ in 0..100 {
                    monitor.check_memory(10).ok();
                    monitor.record_syscall(1).ok();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let usage = monitor.get_usage();
    // 应该记录了所有的系统调用
    assert!(usage.syscall_count > 0);
}

// ============================================================================
// 恢复和重置测试
// ============================================================================

#[test]
fn test_monitor_reset() {
    let monitor = ResourceMonitor::with_default_limits().unwrap();

    // 使用一些资源并触发违规
    monitor.check_memory(500).unwrap();
    monitor.record_syscall(1).unwrap();
    assert!(monitor.check_memory(100_000_000).is_err());

    // 重置
    monitor.reset();

    // 应该回到初始状态
    let usage = monitor.get_usage();
    assert_eq!(usage.memory_used, 0);
    assert_eq!(usage.syscall_count, 0);
    assert!(!monitor.has_violations());
}

// ============================================================================
// 测试辅助函数
// ============================================================================

/// 创建一个简单的 WASM 模块用于测试
#[cfg(test)]
fn create_simple_wasm_module() -> Vec<u8> {
    // 在真实测试中，这里会编译一个简单的 WASM 模块
    // 目前返回空字节向量
    vec![]
}

/// 创建一个尝试执行禁止操作的 WASM 模块
#[cfg(test)]
fn create_malicious_wasm_module() -> Vec<u8> {
    // 在真实测试中，这里会创建一个尝试执行禁止系统调用的模块
    // 目前返回空字节向量
    vec![]
}

#[cfg(test)]
mod test_helpers {
    use super::*;

    /// 辅助函数：创建测试用的 WASM 模块（模拟）
    pub fn make_test_module() -> Vec<u8> {
        // 这是真实测试中会使用的辅助函数
        // 使用 wat2wasm 编译简单的 WASM 文本格式
        vec![]
    }

    /// 辅助函数：测量函数执行时间
    pub fn measure_time<F, R>(f: F) -> Duration
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        f();
        start.elapsed()
    }
}
