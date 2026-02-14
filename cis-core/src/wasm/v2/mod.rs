// CIS v1.1.6 - WASM 沙箱 v2 实现
//
// 本模块包含增强的 WASM 沙箱实现，基于 wasmtime 和全面的安全措施。

pub mod sandbox;
pub mod fuel_limiter;
pub mod resource_monitor;

// 公开导出
pub use sandbox::{
    SecureSandbox,
    SecureSandboxConfig,
    ResourceMonitor as SandboxResourceMonitor,
    ResourceStats,
    SandboxViolation,
    SandboxExecutionResult,
};

pub use fuel_limiter::{
    FuelLimiter,
    FuelConfig,
    FuelStats,
    FuelAnalyzer,
    FuelTrend,
    FuelAnomaly,
};

pub use resource_monitor::{
    ResourceMonitor,
    ResourceLimits,
    ResourceUsage,
    ResourceViolation as ResourceViolationType,
    ResourceUtilization,
    MonitoringReport,
    FileAccessStats,
    NetworkAccessStats,
};
