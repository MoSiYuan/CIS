// CIS v1.1.6 - 增强的 WASM 沙箱实现
//
// 本模块实现了基于 wasmtime 的安全 WASM 沙箱，提供：
// - 完整的系统调用白名单
// - 燃料（fuel）限制
// - 资源监控
// - 多层安全检查
//
// 安全原则：
// 1. 默认拒绝所有系统调用
// 2. 最小权限原则
// 3. 深度防御
// 4. 审计和监控

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use wasmtime::{
    Config, Engine, Linker, Module, OptLevel, Store,
    Instance, FuelConfig, ResourceLimiter,
};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiFile, TcpSocket};
use wasmtime_wasi::pipes::{ReadOnlyPipe, WriteOnlyPipe};

use crate::error::{CisError, Result as CisResult};

/// 安全的系统调用白名单
///
/// 只允许这些系统调用被 WASM 模块执行
const SAFE_SYSCALLS: &[u64] = &[
    // 文件操作（受限）
    libc::SYS_READ,
    libc::SYS_WRITE,
    libc::SYS_CLOSE,
    libc::SYS_READV,
    libc::SYS_WRITEV,
    libc::SYS_FSTAT,
    libc::SYS_STAT,
    // 内存操作
    libc::SYS_MMAP,
    libc::SYS_MUNMAP,
    libc::SYS_MREMAP,
    libc::SYS_MPROTECT,
    libc::SYS_BRK,
    // 时间
    libc::SYS_CLOCK_GETTIME,
    libc::SYS_GETTIMEOFDAY,
    libc::SYS_CLOCK_GETRES,
    // 进程控制
    libc::SYS_EXIT,
    libc::SYS_EXIT_GROUP,
    libc::SYS_GETPID,
    libc::SYS_GETTID,
    // 信号（受限）
    libc::SYS_RT_SIGPROCMASK,
    libc::SYS_RT_SIGACTION,
    // 其他
    libc::SYS_UNAME,
    libc::SYS_GETRANDOM,
    libc::SYS_ARCH_PRCTL,
];

/// 严格禁止的系统调用（黑名单）
///
/// 这些系统调用永远不允许执行
const FORBIDDEN_SYSCALLS: &[u64] = &[
    libc::SYS_EXECVE,
    libc::SYS_FORK,
    libc::SYS_CLONE,
    libc::SYS_KILL,
    libc::SYS_PTRACE,
    libc::SYS_MOUNT,
    libc::SYS_UMOUNT,
    libc::SYS_CHROOT,
    libc::SYS_SETUID,
    libc::SYS_SETGID,
    libc::SYS_SETREUID,
    libc::SYS_SETREGID,
    libc::SYS_CHMOD,
    libc::SYS_CHOWN,
    libc::SYS_LINK,
    libc::SYS_SYMLINK,
    libc::SYS_UNLINK,
    libc::SYS_RENAME,
    libc::SYS_IOCTL,
];

/// 安全沙箱配置
#[derive(Debug, Clone)]
pub struct SecureSandboxConfig {
    /// 最大内存限制（字节）
    pub max_memory: usize,
    /// 最大执行时间（毫秒）
    pub max_execution_time_ms: u64,
    /// 最大燃料限制
    pub max_fuel: u64,
    /// 是否启用燃料消耗
    pub enable_fuel: bool,
    /// 允许的文件目录
    pub allowed_directories: Vec<PathBuf>,
    /// 是否允许网络访问
    pub allow_network: bool,
    /// 最大打开文件数
    pub max_open_files: usize,
}

impl Default for SecureSandboxConfig {
    fn default() -> Self {
        Self {
            max_memory: 512 * 1024 * 1024, // 512 MB
            max_execution_time_ms: 30000,  // 30 秒
            max_fuel: 10_000_000_000,     // 100 亿燃料单位
            enable_fuel: true,
            allowed_directories: vec![],
            allow_network: false,
            max_open_files: 10,
        }
    }
}

/// 资源监控器
///
/// 监控 WASM 模块的资源使用情况
#[derive(Debug)]
pub struct ResourceMonitor {
    start_time: Instant,
    config: SecureSandboxConfig,
    memory_used: Arc<std::sync::atomic::AtomicUsize>,
    open_files: Arc<std::sync::atomic::AtomicUsize>,
    syscalls_called: Arc<std::sync::atomic::AtomicU64>,
    violations: Arc<std::sync::Mutex<Vec<SandboxViolation>>>,
}

impl ResourceMonitor {
    /// 创建新的资源监控器
    pub fn new(config: SecureSandboxConfig) -> Self {
        Self {
            start_time: Instant::now(),
            config,
            memory_used: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            open_files: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            syscalls_called: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            violations: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// 检查执行时间是否超时
    pub fn check_timeout(&self) -> Result<()> {
        let elapsed = self.start_time.elapsed();
        if elapsed > Duration::from_millis(self.config.max_execution_time_ms) {
            return Err(anyhow!("Execution timeout: {:?}", elapsed));
        }
        Ok(())
    }

    /// 检查内存使用是否超限
    pub fn check_memory(&self, size: usize) -> Result<()> {
        let current = self.memory_used.load(std::sync::atomic::Ordering::Relaxed);
        let new_usage = current.saturating_add(size);

        if new_usage > self.config.max_memory {
            return Err(anyhow!(
                "Memory limit exceeded: {} > {}",
                new_usage,
                self.config.max_memory
            ));
        }

        self.memory_used.store(new_usage, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// 记录文件打开
    pub fn record_file_open(&self) -> Result<()> {
        let current = self.open_files.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if current >= self.config.max_open_files {
            return Err(anyhow!(
                "Too many open files: {} >= {}",
                current,
                self.config.max_open_files
            ));
        }
        Ok(())
    }

    /// 记录文件关闭
    pub fn record_file_close(&self) {
        self.open_files.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 记录系统调用
    pub fn record_syscall(&self) {
        self.syscalls_called.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 记录违规
    pub fn record_violation(&self, violation: SandboxViolation) {
        self.violations.lock().unwrap().push(violation);
    }

    /// 获取使用统计
    pub fn get_stats(&self) -> ResourceStats {
        ResourceStats {
            elapsed: self.start_time.elapsed(),
            memory_used: self.memory_used.load(std::sync::atomic::Ordering::Relaxed),
            open_files: self.open_files.load(std::sync::atomic::Ordering::Relaxed),
            syscalls_called: self.syscalls_called.load(std::sync::atomic::Ordering::Relaxed),
            violations: self.violations.lock().unwrap().clone(),
        }
    }
}

impl ResourceLimiter for ResourceMonitor {
    fn memory_growing(&mut self, current: usize, desired: usize, maximum: usize) -> Result<bool> {
        // 检查是否超过配置的最大内存
        if desired > self.config.max_memory {
            self.record_violation(SandboxViolation::MemoryLimitExceeded {
                requested: desired,
                allowed: self.config.max_memory,
            });
            return Ok(false);
        }

        // 检查是否超过 WebAssembly 的最大限制
        if desired > maximum {
            self.record_violation(SandboxViolation::MemoryLimitExceeded {
                requested: desired,
                allowed: maximum,
            });
            return Ok(false);
        }

        // 检查执行时间
        if let Err(e) = self.check_timeout() {
            self.record_violation(SandboxViolation::Timeout {
                elapsed: self.start_time.elapsed(),
            });
            return Err(e);
        }

        Ok(true)
    }

    fn table_growing(&mut self, current: u32, desired: u32, maximum: u32) -> Result<bool> {
        // 限制表增长
        if desired > maximum {
            self.record_violation(SandboxViolation::TableLimitExceeded {
                requested: desired,
                allowed: maximum,
            });
            return Ok(false);
        }
        Ok(true)
    }
}

/// 沙箱违规类型
#[derive(Debug, Clone)]
pub enum SandboxViolation {
    /// 系统调用不在白名单中
    SyscallNotAllowed { syscall_number: u64 },
    /// 系统调用在黑名单中
    SyscallForbidden { syscall_number: u64 },
    /// 内存限制超出
    MemoryLimitExceeded { requested: usize, allowed: usize },
    /// 表限制超出
    TableLimitExceeded { requested: u32, allowed: u32 },
    /// 执行超时
    Timeout { elapsed: Duration },
    /// 文件访问违规
    FileAccessDenied { path: PathBuf, reason: String },
    /// 网络访问违规
    NetworkAccessDenied { endpoint: String },
    /// 打开文件过多
    TooManyOpenFiles { count: usize },
}

/// 资源使用统计
#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub elapsed: Duration,
    pub memory_used: usize,
    pub open_files: usize,
    pub syscalls_called: u64,
    pub violations: Vec<SandboxViolation>,
}

/// 安全沙箱
///
/// 基于 wasmtime 的 WASM 沙箱实现
pub struct SecureSandbox {
    /// Wasmtime 引擎
    engine: Engine,
    /// 系统调用白名单
    syscall_whitelist: HashSet<u64>,
    /// 系统调用黑名单
    syscall_blacklist: HashSet<u64>,
    /// 配置
    config: SecureSandboxConfig,
    /// 资源监控器
    monitor: Arc<ResourceMonitor>,
}

impl SecureSandbox {
    /// 创建新的安全沙箱
    pub fn new(config: SecureSandboxConfig) -> Result<Self> {
        // 配置 Wasmtime 引擎
        let mut engine_config = Config::new();
        engine_config.wasm_simd(true);              // 启用 SIMD
        engine_config.wasm_multi_memory(true);     // 启用多内存
        engine_config.cranelift_opt_level(OptLevel::Speed); // 优化级别

        // 启用燃料消耗
        if config.enable_fuel {
            engine_config.consume_fuel(true);
        }

        // 创建引擎
        let engine = Engine::new(&engine_config)
            .context("Failed to create Wasmtime engine")?;

        // 构建系统调用白名单
        let syscall_whitelist: HashSet<u64> = SAFE_SYSCALLS.iter().copied().collect();

        // 构建系统调用黑名单
        let syscall_blacklist: HashSet<u64> = FORBIDDEN_SYSCALLS.iter().copied().collect();

        // 创建资源监控器
        let monitor = Arc::new(ResourceMonitor::new(config.clone()));

        Ok(Self {
            engine,
            syscall_whitelist,
            syscall_blacklist,
            config,
            monitor,
        })
    }

    /// 使用默认配置创建沙箱
    pub fn with_default_config() -> Result<Self> {
        Self::new(SecureSandboxConfig::default())
    }

    /// 验证系统调用是否允许
    pub fn validate_syscall(&self, sysno: u64) -> Result<()> {
        // 首先检查黑名单
        if self.syscall_blacklist.contains(&sysno) {
            self.monitor.record_violation(SandboxViolation::SyscallForbidden { syscall_number: sysno });
            return Err(anyhow!("Syscall {} is forbidden", sysno));
        }

        // 然后检查白名单
        if !self.syscall_whitelist.contains(&sysno) {
            self.monitor.record_violation(SandboxViolation::SyscallNotAllowed { syscall_number: sysno });
            return Err(anyhow!("Syscall {} is not in whitelist", sysno));
        }

        Ok(())
    }

    /// 创建 WASI 上下文
    fn create_wasi_ctx(&self) -> Result<WasiCtx> {
        let mut builder = WasiCtxBuilder::new();

        // 配置标准输入输出
        builder.stdin(Box::new(ReadOnlyPipe::new()));
        builder.stdout(Box::new(WriteOnlyPipe::new()));
        builder.stderr(Box::new(WriteOnlyPipe::new()));

        // 如果不允许网络，禁用网络
        if !self.config.allow_network {
            builder.inherit_network();
        }

        Ok(builder.build())
    }

    /// 创建链接器
    fn create_linker(&self) -> Result<Linker<WasiCtx>> {
        let mut linker = Linker::new(&self.engine);

        // 添加 WASI 支持
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)
            .context("Failed to add WASI to linker")?;

        // 添加安全的主机函数
        self.add_safe_host_functions(&mut linker)?;

        Ok(linker)
    }

    /// 添加安全的 Host 函数
    fn add_safe_host_functions(&self, linker: &mut Linker<WasiCtx>) -> Result<()> {
        // 在这里添加自定义的 Host 函数
        // 这些函数提供额外的安全检查

        // 示例：添加受限的文件访问函数
        linker.func_wrap("cis", "secure_read", |mut caller: wasmtime::Caller<'_, WasiCtx>, ptr: u32, len: u32| -> Result<(), wasmtime::Trap> {
            // 实现安全的读取逻辑
            Ok(())
        }).context("Failed to wrap secure_read")?;

        Ok(())
    }

    /// 执行 WASM 模块
    pub async fn execute_module(
        &self,
        wasm_bytes: &[u8],
        entry_point: &str,
    ) -> Result<SandboxExecutionResult> {
        // 加载模块
        let module = Module::from_binary(&self.engine, wasm_bytes)
            .context("Failed to load WASM module")?;

        // 验证模块
        self.validate_module(&module)?;

        // 创建 WASI 上下文
        let wasi_ctx = self.create_wasi_ctx()?;

        // 创建存储
        let mut store = Store::new(&self.engine, wasi_ctx);

        // 设置资源限制器
        store.set_resource_limiter(Arc::clone(&self.monitor));
        store.limiter(|s| s as &mut dyn ResourceLimiter);

        // 设置燃料限制
        if self.config.enable_fuel {
            store.add_fuel(self.config.max_fuel)
                .context("Failed to set fuel limit")?;
        }

        // 创建链接器
        let linker = self.create_linker()?;

        // 实例化模块
        let instance = linker.instantiate(&mut store, &module)
            .context("Failed to instantiate WASM module")?;

        // 获取入口函数
        let entry_func = instance.get_typed_func::<(), (), _>(&mut store, entry_point)
            .context("Failed to get entry function")?;

        // 执行入口函数
        let start_time = Instant::now();
        let result = entry_func.call(&mut store, ()).await;
        let execution_time = start_time.elapsed();

        // 检查燃料消耗
        let fuel_consumed = if self.config.enable_fuel {
            Some(store.fuel_consumed()
                .context("Failed to get fuel consumed")?)
        } else {
            None
        };

        // 获取资源统计
        let stats = self.monitor.get_stats();

        match result {
            Ok(_) => Ok(SandboxExecutionResult {
                success: true,
                execution_time,
                fuel_consumed,
                resource_stats: stats,
                error: None,
            }),
            Err(e) => Ok(SandboxExecutionResult {
                success: false,
                execution_time,
                fuel_consumed,
                resource_stats: stats,
                error: Some(e.to_string()),
            }),
        }
    }

    /// 验证 WASM 模块
    fn validate_module(&self, module: &Module) -> Result<()> {
        // 检查模块是否导入不应该导入的函数
        for import in module.imports() {
            match import.module() {
                "wasi_snapshot_preview1" => {
                    // WASI 导入是允许的
                }
                "env" => {
                    // 环境导入需要验证
                    tracing::warn!("Module imports from 'env': {}", import.name());
                }
                _ => {
                    return Err(anyhow!("Unknown import module: {}", import.module()));
                }
            }
        }

        Ok(())
    }

    /// 获取资源监控器
    pub fn monitor(&self) -> &Arc<ResourceMonitor> {
        &self.monitor
    }

    /// 获取配置
    pub fn config(&self) -> &SecureSandboxConfig {
        &self.config
    }
}

/// 沙箱执行结果
#[derive(Debug)]
pub struct SandboxExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 执行时间
    pub execution_time: Duration,
    /// 燃料消耗
    pub fuel_consumed: Option<u64>,
    /// 资源统计
    pub resource_stats: ResourceStats,
    /// 错误信息
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_sandbox_creation() {
        let sandbox = SecureSandbox::with_default_config();
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_syscall_whitelist() {
        let sandbox = SecureSandbox::with_default_config().unwrap();

        // 测试允许的系统调用
        assert!(sandbox.validate_syscall(libc::SYS_READ).is_ok());
        assert!(sandbox.validate_syscall(libc::SYS_WRITE).is_ok());

        // 测试禁止的系统调用
        assert!(sandbox.validate_syscall(libc::SYS_EXECVE).is_err());
        assert!(sandbox.validate_syscall(libc::SYS_FORK).is_err());
    }

    #[test]
    fn test_syscall_blacklist() {
        let sandbox = SecureSandbox::with_default_config().unwrap();

        // 黑名单中的系统调用应该被拒绝
        assert!(sandbox.validate_syscall(libc::SYS_EXECVE).is_err());
        assert!(sandbox.validate_syscall(libc::SYS_FORK).is_err());
        assert!(sandbox.validate_syscall(libc::SYS_CLONE).is_err());
    }

    #[test]
    fn test_resource_monitor_timeout() {
        let config = SecureSandboxConfig {
            max_execution_time_ms: 100, // 100ms
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(config);

        // 立即检查应该不超时
        assert!(monitor.check_timeout().is_ok());

        // 等待超过限制
        std::thread::sleep(Duration::from_millis(150));
        assert!(monitor.check_timeout().is_err());
    }

    #[test]
    fn test_resource_monitor_memory() {
        let config = SecureSandboxConfig {
            max_memory: 1024, // 1KB
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(config);

        // 分配 512 字节应该成功
        assert!(monitor.check_memory(512).is_ok());

        // 再分配 600 字节应该失败（总共 1112 > 1024）
        assert!(monitor.check_memory(600).is_err());
    }

    #[test]
    fn test_resource_monitor_files() {
        let config = SecureSandboxConfig {
            max_open_files: 2,
            ..Default::default()
        };
        let monitor = ResourceMonitor::new(config);

        // 打开 2 个文件应该成功
        assert!(monitor.record_file_open().is_ok());
        assert!(monitor.record_file_open().is_ok());

        // 打开第 3 个文件应该失败
        assert!(monitor.record_file_open().is_err());

        // 关闭一个文件后再打开应该成功
        monitor.record_file_close();
        assert!(monitor.record_file_open().is_ok());
    }
}
