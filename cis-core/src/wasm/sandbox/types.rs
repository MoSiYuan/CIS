//! Sandbox Type Definitions
//!
//! 包含访问类型、配置常量和沙箱摘要结构。

/// 1MB 的字节数
pub const MB: u64 = 1024 * 1024;

/// 默认最大文件描述符数量
pub const DEFAULT_MAX_FD: u32 = 32;

/// 示例文件描述符数量 (用于文档和测试)
pub const EXAMPLE_MAX_FD: u32 = 64;

/// 默认最大文件大小 (100MB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 100 * MB;

/// 示例最大文件大小 (10MB)
pub const EXAMPLE_MAX_FILE_SIZE: u64 = 10 * MB;

/// 默认最大符号链接深度 (防止循环链接)
pub const DEFAULT_MAX_SYMLINK_DEPTH: u32 = 8;

/// 访问类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// 只读访问
    Read,
    /// 写入访问
    Write,
    /// 执行访问
    Execute,
}

impl AccessType {
    /// 检查此访问类型是否需要写权限
    pub fn requires_write(&self) -> bool {
        matches!(self, AccessType::Write)
    }

    /// 检查此访问类型是否需要执行权限
    pub fn requires_execute(&self) -> bool {
        matches!(self, AccessType::Execute)
    }
}

/// 沙箱配置摘要
///
/// 用于调试和日志记录，提供沙箱配置的只读视图。
#[derive(Debug, Clone)]
pub struct WasiSandboxSummary {
    /// 只读路径数量
    pub readonly_paths_count: usize,
    /// 可写路径数量
    pub writable_paths_count: usize,
    /// 最大文件描述符数量
    pub max_fd: u32,
    /// 当前已用文件描述符数量
    pub current_fd: u32,
    /// 最大文件大小（字节）
    pub max_file_size: u64,
    /// 是否允许符号链接
    pub allow_symlinks: bool,
    /// 最大符号链接解析深度
    pub max_symlink_depth: usize,
}
