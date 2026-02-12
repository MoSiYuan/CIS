// CIS v1.1.6 - WASM 沙箱集成测试示例
//
// 本文件展示如何使用 v2 安全增强的 WASM 沙箱
//
// 注意：这些测试需要 wasm-v2 feature：
// cargo test --package cis-core --features wasm-v2 --test integration_tests

use std::path::PathBuf;
use std::time::Duration;

#[cfg(feature = "wasm-v2")]
use cis_core::wasm::v2::{
    SecureSandbox, SecureSandboxConfig,
    FuelLimiter, FuelConfig,
    ResourceMonitor, ResourceLimits,
};

// ============================================================================
// 使用示例
// ============================================================================

/// 示例 1: 使用默认配置创建安全沙箱
#[cfg(feature = "wasm-v2")]
#[test]
fn example_1_default_sandbox() {
    let sandbox = SecureSandbox::with_default_config()
        .expect("Failed to create sandbox");

    // 验证系统调用
    assert!(sandbox.validate_syscall(libc::SYS_READ).is_ok());
    assert!(sandbox.validate_syscall(libc::SYS_EXECVE).is_err());
}

/// 示例 2: 使用自定义配置
#[cfg(feature = "wasm-v2")]
#[test]
fn example_2_custom_config() {
    let config = SecureSandboxConfig {
        max_memory: 256 * 1024 * 1024,  // 256 MB
        max_execution_time_ms: 10000,      // 10 秒
        max_fuel: 1_000_000_000,          // 10 亿燃料
        enable_fuel: true,
        allowed_directories: vec![
            PathBuf::from("/tmp/skill_data"),
        ],
        allow_network: false,
        max_open_files: 5,
    };

    let sandbox = SecureSandbox::new(config)
        .expect("Failed to create sandbox with custom config");

    // 验证配置
    assert_eq!(sandbox.config().max_memory, 256 * 1024 * 1024);
}

/// 示例 3: 使用燃料限制器
#[cfg(feature = "wasm-v2")]
#[test]
fn example_3_fuel_limiter() {
    let config = FuelConfig::new(1_000_000_000)
        .with_refill(1000, 100_000_000)  // 每秒补充 1 亿燃料
        .with_max_accumulation(5_000_000_000);

    let limiter = FuelLimiter::new(config)
        .expect("Failed to create fuel limiter");

    // 获取燃料统计
    assert_eq!(limiter.initial_fuel(), 1_000_000_000);
}

/// 示例 4: 使用资源监控器
#[cfg(feature = "wasm-v2")]
#[test]
fn example_4_resource_monitor() {
    let limits = ResourceLimits::new()
        .with_memory_limit(512 * 1024 * 1024)  // 512 MB
        .with_time_limit(30000);  // 30 秒

    let monitor = ResourceMonitor::new(limits)
        .expect("Failed to create resource monitor");

    // 使用一些资源
    monitor.check_memory(1024).expect("Memory check failed");
    monitor.record_syscall(1).expect("Syscall record failed");

    // 获取使用统计
    let usage = monitor.get_usage();
    assert_eq!(usage.memory_used, 1024);
    assert_eq!(usage.syscall_count, 1);

    // 生成报告
    let report = monitor.generate_report();
    assert!(report.is_success());
}

/// 示例 5: 综合使用
#[cfg(feature = "wasm-v2")]
#[test]
fn example_5_comprehensive_usage() {
    // 1. 创建沙箱配置
    let sandbox_config = SecureSandboxConfig {
        max_memory: 512 * 1024 * 1024,
        max_execution_time_ms: 30000,
        max_fuel: 10_000_000_000,
        enable_fuel: true,
        allowed_directories: vec![],
        allow_network: false,
        max_open_files: 10,
    };

    // 2. 创建沙箱
    let sandbox = SecureSandbox::new(sandbox_config)
        .expect("Failed to create sandbox");

    // 3. 获取资源监控器
    let monitor = sandbox.monitor();

    // 4. 执行一些操作
    monitor.check_memory(256 * 1024 * 1024).unwrap();
    monitor.record_file_open("/tmp/test".into()).unwrap();
    for _ in 0..1000 {
        monitor.record_syscall(1).ok();
    }

    // 5. 检查状态
    let usage = monitor.get_usage();
    let utilization = monitor.get_utilization();

    println!("Memory used: {} bytes", usage.memory_used);
    println!("Memory utilization: {:.1}%", utilization.memory_utilization * 100.0);
    println!("Syscalls: {}", usage.syscall_count);

    // 6. 生成报告
    let report = monitor.generate_report();
    println!("\n{}", report.to_string());

    assert!(report.is_success());
}

// ============================================================================
// 实际 WASM 模块执行示例
// ============================================================================

/// 示例 6: 执行 WASM 模块（需要真实的 WASM 文件）
#[cfg(feature = "wasm-v2")]
#[test]
#[ignore]
async fn example_6_execute_wasm_module() {
    let sandbox = SecureSandbox::with_default_config()
        .expect("Failed to create sandbox");

    // 在实际测试中，这里会加载真实的 WASM 模块
    // let wasm_bytes = std::fs::read("test.wasm")
    //     .expect("Failed to load WASM file");

    // 执行模块
    // let result = sandbox.execute_module(&wasm_bytes, "main").await
    //     .expect("Failed to execute module");

    // assert!(result.success);
}

// ============================================================================
// 错误处理示例
// ============================================================================

/// 示例 7: 处理资源限制违规
#[cfg(feature = "wasm-v2")]
#[test]
fn example_7_handling_violations() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_memory_bytes: 1000,
            max_open_files: 2,
            ..Default::default()
        }
    ).expect("Failed to create monitor");

    // 尝试分配超过限制的内存
    monitor.check_memory(800).unwrap();
    assert!(monitor.check_memory(300).is_err());

    // 检查违规
    let violations = monitor.get_violations();
    assert!(!violations.is_empty());

    // 打印违规信息
    for violation in violations {
        println!("Violation: {:?}", violation);
    }
}

/// 示例 8: 资源利用率监控
#[cfg(feature = "wasm-v2")]
#[test]
fn example_8_utilization_monitoring() {
    let monitor = ResourceMonitor::new(
        ResourceLimits {
            max_memory_bytes: 10_000,
            max_execution_time_ms: 10_000,
            max_open_files: 10,
            max_network_connections: 0,
            max_total_syscalls: 1000,
            ..Default::default()
        }
    ).expect("Failed to create monitor");

    // 使用 70% 的内存
    monitor.check_memory(7_000).unwrap();

    // 使用 30% 的文件句柄
    monitor.record_file_open("/tmp/file1".into()).unwrap();
    monitor.record_file_open("/tmp/file2".into()).unwrap();
    monitor.record_file_open("/tmp/file3".into()).unwrap();

    // 检查压力
    let utilization = monitor.get_utilization();
    assert!(utilization.has_pressure(0.6));  // 60% 阈值
    assert!(!utilization.has_pressure(0.8)); // 80% 阈值
}

// ============================================================================
// 最佳实践示例
// ============================================================================

/// 示例 9: 生产环境配置
#[cfg(feature = "wasm-v2")]
#[test]
fn example_9_production_config() {
    // 生产环境的严格配置
    let config = SecureSandboxConfig {
        max_memory: 128 * 1024 * 1024,  // 128 MB
        max_execution_time_ms: 5000,       // 5 秒
        max_fuel: 5_000_000_000,          // 50 亿燃料
        enable_fuel: true,
        allowed_directories: vec![
            PathBuf::from("/var/lib/cis/skills"),
        ],
        allow_network: false,
        max_open_files: 3,
    };

    let sandbox = SecureSandbox::new(config);
    assert!(sandbox.is_ok());
}

/// 示例 10: 开发环境配置
#[cfg(feature = "wasm-v2")]
#[test]
fn example_10_development_config() {
    // 开发环境的宽松配置
    let config = SecureSandboxConfig {
        max_memory: 1024 * 1024 * 1024, // 1 GB
        max_execution_time_ms: 300000,    // 5 分钟
        max_fuel: 100_000_000_000,       // 1000 亿燃料
        enable_fuel: true,
        allowed_directories: vec![
            PathBuf::from("/tmp/cis_dev"),
            PathBuf::from("./test_data"),
        ],
        allow_network: true,  // 开发时允许网络
        max_open_files: 20,
    };

    let sandbox = SecureSandbox::new(config);
    assert!(sandbox.is_ok());
}

// ============================================================================
// 测试辅助函数
// ============================================================================

#[cfg(feature = "wasm-v2")]
mod test_helpers {
    use super::*;

    /// 创建测试用的 WASM 模块（占位符）
    pub fn create_test_wasm_module() -> Vec<u8> {
        // 在实际测试中，这里会编译 WASM 代码
        // 目前返回空向量
        vec![]
    }

    /// 测量执行时间
    pub fn measure_time<F, R>(f: F) -> Duration
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        f();
        start.elapsed()
    }

    #[test]
    fn test_measure_time() {
        let duration = measure_time(|| {
            std::thread::sleep(Duration::from_millis(100));
        });
        assert!(duration >= Duration::from_millis(100));
    }
}
