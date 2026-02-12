// CIS v1.1.6 - WASM 资源监控器
//
// 本模块实现了 WASM 模块执行的全面资源监控。
// 监控指标包括：
// - 内存使用（堆、栈、总内存）
// - CPU 时间
// - 文件句柄
// - 网络连接
// - 系统调用计数
// - 执行时间

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde::{Serialize, Deserialize};

use crate::error::{CisError, Result as CisResult};

/// 资源限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// 最大内存使用（字节）
    pub max_memory_bytes: usize,
    /// 最大堆大小（字节）
    pub max_heap_bytes: usize,
    /// 最大栈大小（字节）
    pub max_stack_bytes: usize,
    /// 最大执行时间（毫秒）
    pub max_execution_time_ms: u64,
    /// 最大打开文件数
    pub max_open_files: usize,
    /// 最大网络连接数
    pub max_network_connections: usize,
    /// 最大每秒系统调用数
    pub max_syscalls_per_second: u64,
    /// 最大总系统调用数
    pub max_total_syscalls: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024,  // 512 MB
            max_heap_bytes: 480 * 1024 * 1024,     // 480 MB
            max_stack_bytes: 8 * 1024 * 1024,     // 8 MB
            max_execution_time_ms: 30000,           // 30 秒
            max_open_files: 10,
            max_network_connections: 0,              // 默认禁止网络
            max_syscalls_per_second: 10000,
            max_total_syscalls: 1_000_000,
        }
    }
}

impl ResourceLimits {
    /// 创建新的资源限制
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置内存限制
    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = bytes;
        self.max_heap_bytes = (bytes as f64 * 0.95) as usize; // 堆占 95%
        self
    }

    /// 设置执行时间限制
    pub fn with_time_limit(mut self, ms: u64) -> Self {
        self.max_execution_time_ms = ms;
        self
    }

    /// 验证限制
    pub fn validate(&self) -> Result<()> {
        if self.max_memory_bytes == 0 {
            return Err(anyhow!("Memory limit cannot be zero"));
        }
        if self.max_execution_time_ms == 0 {
            return Err(anyhow!("Execution time limit cannot be zero"));
        }
        if self.max_heap_bytes > self.max_memory_bytes {
            return Err(anyhow!("Heap size cannot exceed total memory"));
        }
        Ok(())
    }
}

/// 资源使用统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// 已使用的内存（字节）
    pub memory_used: usize,
    /// 已使用的堆（字节）
    pub heap_used: usize,
    /// 已使用的栈（字节）
    pub stack_used: usize,
    /// 执行时间
    pub elapsed_time: Duration,
    /// CPU 时间（纳秒）
    pub cpu_time_ns: u128,
    /// 当前打开的文件数
    pub open_files: usize,
    /// 当前网络连接数
    pub network_connections: usize,
    /// 系统调用计数
    pub syscall_count: u64,
    /// 文件访问统计
    pub file_access_stats: FileAccessStats,
    /// 网络访问统计
    pub network_access_stats: NetworkAccessStats,
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            memory_used: 0,
            heap_used: 0,
            stack_used: 0,
            elapsed_time: Duration::ZERO,
            cpu_time_ns: 0,
            open_files: 0,
            network_connections: 0,
            syscall_count: 0,
            file_access_stats: FileAccessStats::default(),
            network_access_stats: NetworkAccessStats::default(),
        }
    }
}

/// 文件访问统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileAccessStats {
    /// 读取次数
    pub read_count: u64,
    /// 写入次数
    pub write_count: u64,
    /// 打开的文件列表
    pub opened_files: Vec<PathBuf>,
    /// 访问被拒绝的文件
    pub denied_files: Vec<(PathBuf, String)>,
}

/// 网络访问统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkAccessStats {
    /// 连接尝试次数
    pub connection_attempts: u64,
    /// 成功连接数
    pub successful_connections: u64,
    /// 发送字节数
    pub bytes_sent: u64,
    /// 接收字节数
    pub bytes_received: u64,
    /// 连接的端点
    pub connected_endpoints: Vec<String>,
}

/// 资源违规类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceViolation {
    /// 内存限制超出
    MemoryLimitExceeded {
        used: usize,
        limit: usize,
    },
    /// 执行时间超限
    ExecutionTimeout {
        elapsed: Duration,
        limit: Duration,
    },
    /// 打开文件过多
    TooManyOpenFiles {
        count: usize,
        limit: usize,
    },
    /// 网络连接过多
    TooManyConnections {
        count: usize,
        limit: usize,
    },
    /// 系统调用过多
    TooManySyscalls {
        count: u64,
        limit: u64,
    },
    /// 系统调用速率过高
    SyscallRateExceeded {
        rate: f64,
        limit: u64,
    },
    /// 文件访问被拒绝
    FileAccessDenied {
        path: PathBuf,
        reason: String,
    },
    /// 网络访问被拒绝
    NetworkAccessDenied {
        endpoint: String,
        reason: String,
    },
}

/// 资源监控器
///
/// 监控 WASM 模块执行的所有资源使用
pub struct ResourceMonitor {
    /// 资源限制
    limits: ResourceLimits,
    /// 当前资源使用
    usage: Arc<Mutex<ResourceUsage>>,
    /// 开始时间
    start_time: Instant,
    /// 违规记录
    violations: Arc<Mutex<Vec<ResourceViolation>>>,
    /// 系统调用时间窗口（用于速率限制）
    syscall_windows: Arc<Mutex<Vec<Instant>>>,
}

impl ResourceMonitor {
    /// 创建新的资源监控器
    pub fn new(limits: ResourceLimits) -> Result<Self> {
        limits.validate()?;

        Ok(Self {
            limits,
            usage: Arc::new(Mutex::new(ResourceUsage::default())),
            start_time: Instant::now(),
            violations: Arc::new(Mutex::new(Vec::new())),
            syscall_windows: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// 使用默认限制创建
    pub fn with_default_limits() -> Result<Self> {
        Self::new(ResourceLimits::default())
    }

    /// 检查内存限制
    pub fn check_memory(&self, additional_bytes: usize) -> Result<()> {
        let mut usage = self.usage.lock().unwrap();

        let new_usage = usage.memory_used.saturating_add(additional_bytes);
        if new_usage > self.limits.max_memory_bytes {
            let violation = ResourceViolation::MemoryLimitExceeded {
                used: new_usage,
                limit: self.limits.max_memory_bytes,
            };
            self.record_violation(violation.clone());
            return Err(anyhow!("Memory limit exceeded: {} > {}", new_usage, self.limits.max_memory_bytes));
        }

        usage.memory_used = new_usage;
        Ok(())
    }

    /// 检查执行时间
    pub fn check_timeout(&self) -> Result<()> {
        let elapsed = self.start_time.elapsed();
        let limit = Duration::from_millis(self.limits.max_execution_time_ms);

        if elapsed > limit {
            let violation = ResourceViolation::ExecutionTimeout {
                elapsed,
                limit,
            };
            self.record_violation(violation.clone());
            return Err(anyhow!("Execution timeout: {:?}", elapsed));
        }

        // 更新已用时间
        self.usage.lock().unwrap().elapsed_time = elapsed;
        Ok(())
    }

    /// 记录文件打开
    pub fn record_file_open(&self, path: PathBuf) -> Result<()> {
        let mut usage = self.usage.lock().unwrap();

        if usage.open_files >= self.limits.max_open_files {
            let violation = ResourceViolation::TooManyOpenFiles {
                count: usage.open_files,
                limit: self.limits.max_open_files,
            };
            self.record_violation(violation.clone());
            return Err(anyhow!("Too many open files: {} >= {}", usage.open_files, self.limits.max_open_files));
        }

        usage.open_files += 1;
        usage.file_access_stats.opened_files.push(path);
        Ok(())
    }

    /// 记录文件关闭
    pub fn record_file_close(&self) {
        let mut usage = self.usage.lock().unwrap();
        usage.open_files = usage.open_files.saturating_sub(1);
    }

    /// 记录文件读取
    pub fn record_file_read(&self, path: PathBuf, bytes: usize) -> Result<()> {
        // 检查内存
        self.check_memory(bytes)?;

        let mut usage = self.usage.lock().unwrap();
        usage.file_access_stats.read_count += 1;
        Ok(())
    }

    /// 记录文件写入
    pub fn record_file_write(&self, path: PathBuf, bytes: usize) -> Result<()> {
        // 检查内存
        self.check_memory(bytes)?;

        let mut usage = self.usage.lock().unwrap();
        usage.file_access_stats.write_count += 1;
        Ok(())
    }

    /// 记录文件访问被拒绝
    pub fn record_file_access_denied(&self, path: PathBuf, reason: String) {
        let violation = ResourceViolation::FileAccessDenied {
            path: path.clone(),
            reason: reason.clone(),
        };
        self.record_violation(violation);

        let mut usage = self.usage.lock().unwrap();
        usage.file_access_stats.denied_files.push((path, reason));
    }

    /// 记录网络连接
    pub fn record_network_connection(&self, endpoint: String) -> Result<()> {
        let mut usage = self.usage.lock().unwrap();

        if usage.network_connections >= self.limits.max_network_connections {
            let violation = ResourceViolation::TooManyConnections {
                count: usage.network_connections,
                limit: self.limits.max_network_connections,
            };
            self.record_violation(violation.clone());
            return Err(anyhow!("Too many network connections: {} >= {}",
                usage.network_connections, self.limits.max_network_connections));
        }

        usage.network_connections += 1;
        usage.network_access_stats.connection_attempts += 1;
        usage.network_access_stats.connected_endpoints.push(endpoint);
        Ok(())
    }

    /// 记录网络断开
    pub fn record_network_disconnect(&self) {
        let mut usage = self.usage.lock().unwrap();
        usage.network_connections = usage.network_connections.saturating_sub(1);
    }

    /// 记录系统调用
    pub fn record_syscall(&self, syscall_number: u64) -> Result<()> {
        let mut usage = self.usage.lock().unwrap();

        // 检查总系统调用数
        if usage.syscall_count >= self.limits.max_total_syscalls {
            let violation = ResourceViolation::TooManySyscalls {
                count: usage.syscall_count,
                limit: self.limits.max_total_syscalls,
            };
            self.record_violation(violation.clone());
            return Err(anyhow!("Too many syscalls: {} >= {}", usage.syscall_count, self.limits.max_total_syscalls));
        }

        usage.syscall_count += 1;

        // 检查系统调用速率
        drop(usage);
        self.check_syscall_rate()?;

        Ok(())
    }

    /// 检查系统调用速率
    fn check_syscall_rate(&self) -> Result<()> {
        let mut windows = self.syscall_windows.lock().unwrap();
        let now = Instant::now();

        // 移除超过 1 秒的记录
        windows.retain(|t| now.duration_since(*t) < Duration::from_secs(1));

        // 检查速率
        if windows.len() as u64 >= self.limits.max_syscalls_per_second {
            let rate = windows.len() as f64;
            let violation = ResourceViolation::SyscallRateExceeded {
                rate,
                limit: self.limits.max_syscalls_per_second,
            };
            self.record_violation(violation.clone());
            return Err(anyhow!("Syscall rate exceeded: {} >= {}", rate, self.limits.max_syscalls_per_second));
        }

        windows.push(now);
        Ok(())
    }

    /// 记录违规
    fn record_violation(&self, violation: ResourceViolation) {
        self.violations.lock().unwrap().push(violation);
    }

    /// 获取当前资源使用
    pub fn get_usage(&self) -> ResourceUsage {
        self.usage.lock().unwrap().clone()
    }

    /// 获取违规列表
    pub fn get_violations(&self) -> Vec<ResourceViolation> {
        self.violations.lock().unwrap().clone()
    }

    /// 获取资源限制
    pub fn get_limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// 检查是否有违规
    pub fn has_violations(&self) -> bool {
        !self.violations.lock().unwrap().is_empty()
    }

    /// 获取资源使用率（0.0 - 1.0）
    pub fn get_utilization(&self) -> ResourceUtilization {
        let usage = self.get_usage();
        let limits = &self.limits;

        ResourceUtilization {
            memory_utilization: usage.memory_used as f64 / limits.max_memory_bytes as f64,
            time_utilization: usage.elapsed_time.as_millis() as f64 / limits.max_execution_time_ms as f64,
            file_utilization: usage.open_files as f64 / limits.max_open_files as f64,
            network_utilization: usage.network_connections as f64 / limits.max_network_connections as f64,
            syscall_utilization: usage.syscall_count as f64 / limits.max_total_syscalls as f64,
        }
    }

    /// 生成监控报告
    pub fn generate_report(&self) -> MonitoringReport {
        let usage = self.get_usage();
        let violations = self.get_violations();
        let utilization = self.get_utilization();

        MonitoringReport {
            execution_time: usage.elapsed_time,
            resource_usage: usage,
            resource_limits: self.limits.clone(),
            utilization,
            violations,
            total_violations: violations.len(),
        }
    }

    /// 重置监控状态
    pub fn reset(&self) {
        self.usage.lock().unwrap().resource_usage_reset();
        self.violations.lock().unwrap().clear();
        self.syscall_windows.lock().unwrap().clear();
    }
}

impl ResourceUsage {
    fn resource_usage_reset(&mut self) {
        *self = Self::default();
    }
}

/// 资源利用率
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUtilization {
    /// 内存利用率（0.0 - 1.0）
    pub memory_utilization: f64,
    /// 时间利用率（0.0 - 1.0）
    pub time_utilization: f64,
    /// 文件利用率（0.0 - 1.0）
    pub file_utilization: f64,
    /// 网络利用率（0.0 - 1.0）
    pub network_utilization: f64,
    /// 系统调用利用率（0.0 - 1.0）
    pub syscall_utilization: f64,
}

impl ResourceUtilization {
    /// 检查是否有资源接近限制
    pub fn has_pressure(&self, threshold: f64) -> bool {
        self.memory_utilization > threshold
            || self.time_utilization > threshold
            || self.file_utilization > threshold
            || self.network_utilization > threshold
            || self.syscall_utilization > threshold
    }
}

/// 监控报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringReport {
    /// 执行时间
    pub execution_time: Duration,
    /// 资源使用
    pub resource_usage: ResourceUsage,
    /// 资源限制
    pub resource_limits: ResourceLimits,
    /// 利用率
    pub utilization: ResourceUtilization,
    /// 违规列表
    pub violations: Vec<ResourceViolation>,
    /// 总违规数
    pub total_violations: usize,
}

impl MonitoringReport {
    /// 是否成功（无违规）
    pub fn is_success(&self) -> bool {
        self.total_violations == 0
    }

    /// 生成人类可读的报告
    pub fn to_string(&self) -> String {
        let mut report = String::new();

        report.push_str("=== WASM 资源监控报告 ===\n\n");

        // 执行时间
        report.push_str(&format!("执行时间: {:.2}s\n", self.execution_time.as_secs_f64()));

        // 资源使用
        report.push_str("\n资源使用:\n");
        report.push_str(&format!("  内存: {} / {} MB ({:.1}%)\n",
            self.resource_usage.memory_used / 1024 / 1024,
            self.resource_limits.max_memory_bytes / 1024 / 1024,
            self.utilization.memory_utilization * 100.0
        ));
        report.push_str(&format!("  打开文件: {} / {} ({:.1}%)\n",
            self.resource_usage.open_files,
            self.resource_limits.max_open_files,
            self.utilization.file_utilization * 100.0
        ));
        report.push_str(&format!("  系统调用: {} / {} ({:.1}%)\n",
            self.resource_usage.syscall_count,
            self.resource_limits.max_total_syscalls,
            self.utilization.syscall_utilization * 100.0
        ));

        // 违规
        if !self.violations.is_empty() {
            report.push_str(&format!("\n违规 ({}):\n", self.total_violations));
            for (i, violation) in self.violations.iter().enumerate() {
                report.push_str(&format!("  {}. {:?}\n", i + 1, violation));
            }
        } else {
            report.push_str("\n无违规\n");
        }

        report.push_str("\n=== 报告结束 ===\n");

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_monitor_creation() {
        let monitor = ResourceMonitor::with_default_limits();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_memory_limit() {
        let limits = ResourceLimits {
            max_memory_bytes: 1024,
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(limits).unwrap();

        // 分配 512 字节应该成功
        assert!(monitor.check_memory(512).is_ok());

        // 再分配 600 字节应该失败
        assert!(monitor.check_memory(600).is_err());
    }

    #[test]
    fn test_timeout() {
        let limits = ResourceLimits {
            max_execution_time_ms: 100,
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(limits).unwrap();

        // 立即检查应该不超时
        assert!(monitor.check_timeout().is_ok());

        // 等待超过限制
        std::thread::sleep(Duration::from_millis(150));
        assert!(monitor.check_timeout().is_err());
    }

    #[test]
    fn test_file_limits() {
        let limits = ResourceLimits {
            max_open_files: 2,
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(limits).unwrap();

        // 打开 2 个文件应该成功
        assert!(monitor.record_file_open(PathBuf::from("/tmp/file1")).is_ok());
        assert!(monitor.record_file_open(PathBuf::from("/tmp/file2")).is_ok());

        // 打开第 3 个文件应该失败
        assert!(monitor.record_file_open(PathBuf::from("/tmp/file3")).is_err());

        // 关闭一个后应该能再打开
        monitor.record_file_close();
        assert!(monitor.record_file_open(PathBuf::from("/tmp/file4")).is_ok());
    }

    #[test]
    fn test_syscall_limits() {
        let limits = ResourceLimits {
            max_total_syscalls: 100,
            max_syscalls_per_second: 10,
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(limits).unwrap();

        // 执行 90 次系统调用应该成功
        for _ in 0..90 {
            assert!(monitor.record_syscall(1).is_ok());
        }

        // 再执行 11 次应该超过总数
        for _ in 0..11 {
            monitor.record_syscall(1);
        }

        // 应该有违规
        assert!(monitor.has_violations());
    }

    #[test]
    fn test_utilization() {
        let limits = ResourceLimits {
            max_memory_bytes: 1000,
            max_execution_time_ms: 1000,
            max_open_files: 10,
            max_network_connections: 5,
            max_total_syscalls: 1000,
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(limits).unwrap();

        // 使用一些资源
        monitor.check_memory(500).unwrap();
        monitor.record_file_open(PathBuf::from("/tmp/file")).unwrap();
        for _ in 0..500 {
            monitor.record_syscall(1).ok();
        }

        let utilization = monitor.get_utilization();
        assert_eq!(utilization.memory_utilization, 0.5);
        assert_eq!(utilization.file_utilization, 0.1);
        assert_eq!(utilization.syscall_utilization, 0.5);
    }

    #[test]
    fn test_report_generation() {
        let monitor = ResourceMonitor::with_default_limits().unwrap();
        let report = monitor.generate_report();

        assert!(report.is_success());
        assert_eq!(report.total_violations, 0);

        let report_str = report.to_string();
        assert!(report_str.contains("执行时间"));
        assert!(report_str.contains("资源使用"));
    }
}
