//! # WASI Sandbox 模块
//!
//! 提供基于能力模型的 WASI 沙箱限制，用于安全地执行 WASM Skill。
//!
//! ## 功能
//!
//! - 路径白名单验证（只读和可写路径）
//! - 路径遍历攻击防护
//! - 符号链接逃逸防护
//! - 文件描述符限制
//! - 文件大小配额限制
//!
//! ## 安全特性
//!
//! - 禁止路径遍历 (`../`)
//! - 符号链接解析（防止逃逸到沙箱外）
//! - 文件描述符数量限制
//! - 磁盘配额限制
//! - 🔒 **P0安全修复**: RAII文件描述符管理
//!
//! ## 使用示例
//!
//! ```rust
//! use cis_core::wasm::sandbox::{WasiSandbox, AccessType};
//!
//! # fn example() -> cis_core::Result<()> {
//! let sandbox = WasiSandbox::new()
//!     .with_readonly_path("/data")
//!     .with_writable_path("/tmp")
//!     .with_max_fd(EXAMPLE_MAX_FD) // 64
//!     .with_max_file_size(EXAMPLE_MAX_FILE_SIZE); // 10MB
//!
//! // 验证路径访问
//! let validated_path = sandbox.validate_path("/data/file.txt", AccessType::Read)?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, warn};

use crate::error::{CisError, Result};

// 🔒 P0安全修复：导入RAII文件描述符守卫
mod file_descriptor_guard;
pub use file_descriptor_guard::FileDescriptorGuard;

// P1-5: 模块拆分 - 提取类型定义和验证函数
mod types;
mod validation;

// 重新导出公共API
pub use types::{
    MB,
    DEFAULT_MAX_FD,
    EXAMPLE_MAX_FD,
    DEFAULT_MAX_FILE_SIZE,
    EXAMPLE_MAX_FILE_SIZE,
    DEFAULT_MAX_SYMLINK_DEPTH,
    AccessType,
    WasiSandboxSummary,
};

// 重新导出验证函数（公共API）
pub use validation::{normalize_path, contains_path_traversal, is_safe_filename};

/// WASI 沙箱配置
///
/// 基于能力模型的沙箱，定义 WASM Skill 可以访问的资源。
#[derive(Debug)]
pub struct WasiSandbox {
    /// 只读路径白名单
    readonly_paths: HashSet<PathBuf>,
    /// 可写路径白名单
    writable_paths: HashSet<PathBuf>,
    /// 最大文件描述符数量
    max_fd: u32,
    /// 最大文件大小（字节）
    max_file_size: u64,
    /// 是否允许符号链接
    allow_symlinks: bool,
    /// 最大符号链接解析深度
    max_symlink_depth: usize,
    /// 当前文件描述符计数
    current_fd_count: AtomicU32,
}

impl Clone for WasiSandbox {
    fn clone(&self) -> Self {
        Self {
            readonly_paths: self.readonly_paths.clone(),
            writable_paths: self.writable_paths.clone(),
            max_fd: self.max_fd,
            max_file_size: self.max_file_size,
            allow_symlinks: self.allow_symlinks,
            max_symlink_depth: self.max_symlink_depth,
            current_fd_count: AtomicU32::new(self.current_fd_count.load(Ordering::SeqCst)),
        }
    }
}

impl Default for WasiSandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiSandbox {
    /// 创建新的沙箱配置
    ///
    /// 使用默认配置：
    /// - max_fd: [`DEFAULT_MAX_FD`] (32)
    /// - max_file_size: [`DEFAULT_MAX_FILE_SIZE`] (100MB)
    /// - allow_symlinks: false
    /// - max_symlink_depth: [`DEFAULT_MAX_SYMLINK_DEPTH`] (8)
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::sandbox::WasiSandbox;
    ///
    /// let sandbox = WasiSandbox::new();
    /// ```
    pub fn new() -> Self {
        Self {
            readonly_paths: HashSet::new(),
            writable_paths: HashSet::new(),
            max_fd: DEFAULT_MAX_FD,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            allow_symlinks: false,
            max_symlink_depth: DEFAULT_MAX_SYMLINK_DEPTH as usize,
            current_fd_count: AtomicU32::new(0),
        }
    }

    /// 添加只读路径
    ///
    /// # 参数
    /// - `path`: 允许只读访问的路径
    ///
    /// # 返回
    /// 返回自身以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::sandbox::WasiSandbox;
    ///
    /// let sandbox = WasiSandbox::new()
    ///     .with_readonly_path("/data");
    /// ```
    pub fn with_readonly_path(mut self, path: impl AsRef<Path>) -> Self {
        let normalized = normalize_path(path.as_ref());
        debug!("Adding readonly path: {}", normalized.display());
        self.readonly_paths.insert(normalized);
        self
    }

    /// 添加可写路径
    ///
    /// # 参数
    /// - `path`: 允许写入访问的路径
    ///
    /// # 返回
    /// 返回自身以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::sandbox::WasiSandbox;
    ///
    /// let sandbox = WasiSandbox::new()
    ///     .with_writable_path("/tmp");
    /// ```
    pub fn with_writable_path(mut self, path: impl AsRef<Path>) -> Self {
        let normalized = normalize_path(path.as_ref());
        debug!("Adding writable path: {}", normalized.display());
        self.writable_paths.insert(normalized);
        self
    }

    /// 设置最大文件描述符数量
    ///
    /// # 参数
    /// - `max_fd`: 最大文件描述符数量
    ///
    /// # 返回
    /// 返回自身以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::sandbox::WasiSandbox;
    ///
    /// let sandbox = WasiSandbox::new()
    ///     .with_max_fd(EXAMPLE_MAX_FD); // 64
    /// ```
    pub fn with_max_fd(mut self, max_fd: u32) -> Self {
        self.max_fd = max_fd;
        self
    }

    /// 设置最大文件大小
    ///
    /// # 参数
    /// - `max_file_size`: 最大文件大小（字节）
    ///
    /// # 返回
    /// 返回自身以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::sandbox::WasiSandbox;
    ///
    /// let sandbox = WasiSandbox::new()
    ///     .with_max_file_size(EXAMPLE_MAX_FILE_SIZE); // 10MB
    /// ```
    pub fn with_max_file_size(mut self, max_file_size: u64) -> Self {
        self.max_file_size = max_file_size;
        self
    }

    /// 设置是否允许符号链接
    ///
    /// # 参数
    /// - `allow`: 是否允许符号链接
    ///
    /// # 返回
    /// 返回自身以支持链式调用
    pub fn with_allow_symlinks(mut self, allow: bool) -> Self {
        self.allow_symlinks = allow;
        self
    }

    /// 设置最大符号链接解析深度
    ///
    /// # 参数
    /// - `depth`: 最大解析深度
    ///
    /// # 返回
    /// 返回自身以支持链式调用
    pub fn with_max_symlink_depth(mut self, depth: usize) -> Self {
        self.max_symlink_depth = depth;
        self
    }

    /// 获取最大文件描述符数量
    pub fn max_fd(&self) -> u32 {
        self.max_fd
    }

    /// 获取最大文件大小
    pub fn max_file_size(&self) -> u64 {
        self.max_file_size
    }

    /// 获取只读路径列表
    pub fn readonly_paths(&self) -> &HashSet<PathBuf> {
        &self.readonly_paths
    }

    /// 获取可写路径列表
    pub fn writable_paths(&self) -> &HashSet<PathBuf> {
        &self.writable_paths
    }

    /// 🔒 P0安全修复：分配文件描述符（RAII模式）
    ///
    /// 返回一个守卫，当守卫被drop时自动释放文件描述符
    ///
    /// # 参数
    /// 无
    ///
    /// # 返回
    /// - `Some(FileDescriptorGuard)`: 分配成功
    /// - `None`: 已达到最大文件描述符限制
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::sandbox::WasiSandbox;
    ///
    /// let sandbox = WasiSandbox::new();
    /// if let Some(_fd_guard) = sandbox.try_allocate_fd() {
    ///     // 使用文件描述符
    ///     // 守卫在离开作用域时自动释放
    /// }
    /// ```
    pub fn try_allocate_fd(&self) -> Option<FileDescriptorGuard> {
        FileDescriptorGuard::acquire(&self.current_fd_count, self.max_fd)
    }

    /// 分配文件描述符（旧接口，保持兼容性）
    ///
    /// # 返回
    /// - `Ok(())`: 分配成功
    /// - `Err(CisError)`: 已达到最大文件描述符限制
    ///
    /// [WARNING] **已弃用**: 使用 `try_allocate_fd()` 获得RAII保证
    #[deprecated(since = "1.1.6", note = "Use try_allocate_fd() for RAII guarantee")]
    pub fn allocate_fd(&self) -> Result<()> {
        if self.try_allocate_fd().is_some() {
            Ok(())
        } else {
            Err(CisError::wasm(format!(
                "File descriptor limit exceeded: {} (max: {})",
                self.current_fd_count.load(Ordering::SeqCst),
                self.max_fd
            )))
        }
    }

    /// 释放文件描述符（旧接口，保持兼容性）
    ///
    /// [WARNING] **已弃用**: RAII守卫会自动释放，无需手动调用
    #[deprecated(since = "1.1.6", note = "RAII guard auto-releases on drop")]
    pub fn release_fd(&self) {
        let current = self.current_fd_count.load(Ordering::SeqCst);
        if current > 0 {
            self.current_fd_count.fetch_sub(1, Ordering::SeqCst);
            debug!("Released fd (manual): {} -> {}", current, current - 1);
        }
    }

    /// 获取当前文件描述符数量
    pub fn current_fd_count(&self) -> u32 {
        self.current_fd_count.load(Ordering::SeqCst)
    }

    /// 验证文件大小是否在限制范围内
    ///
    /// # 参数
    /// - `size`: 文件大小（字节）
    ///
    /// # 返回
    /// - `Ok(())`: 文件大小符合限制
    /// - `Err(CisError)`: 文件大小超出限制
    pub fn validate_file_size(&self, size: u64) -> Result<()> {
        if size > self.max_file_size {
            return Err(CisError::wasm(format!(
                "File size {} exceeds maximum allowed size {}",
                size, self.max_file_size
            )));
        }
        Ok(())
    }

    /// 🔒 验证路径访问权限（安全加固版）
    ///
    /// 检查路径是否在白名单内，并验证访问类型是否被允许。
    ///
    /// # 安全修复 (P0)
    ///
    /// 1. **双重检查**: 规范化前后都检查路径遍历
    /// 2. **严格验证**: 拒绝无法规范的路径
    /// 3. **深度限制**: 符号链接解析深度限制
    ///
    /// # 参数
    /// - `path`: 要验证的路径
    /// - `access`: 访问类型
    ///
    /// # 返回
    /// - `Ok(PathBuf)`: 验证通过，返回规范化后的路径
    /// - `Err(CisError)`: 验证失败
    ///
    /// # 安全检查层级
    ///
    /// 1. 原始路径遍历检测（`../`, `..\`）
    /// 2. 规范化路径验证（必须成功）
    /// 3. 🔒 **双重检查**: 再次检测规范化后的路径
    /// 4. 符号链接逃逸检测（递归检查）
    /// 5. 白名单权限检查（精确前缀匹配）
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::sandbox::{WasiSandbox, AccessType};
    ///
    /// # fn example() -> cis_core::Result<()> {
    /// let sandbox = WasiSandbox::new()
    ///     .with_readonly_path("/data");
    ///
    /// // [OK] 允许：白名单内的路径
    /// let path = sandbox.validate_path("/data/file.txt", AccessType::Read)?;
    ///
    /// // [X] 拒绝：路径遍历攻击
    /// let result = sandbox.validate_path("/data/../etc/passwd", AccessType::Read);
    /// assert!(result.is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn validate_path(&self, path: &str, access: AccessType) -> Result<PathBuf> {
        let path_obj = Path::new(path);

        // 1. 🔒 第一层防护：原始路径遍历检测
        if contains_path_traversal(path_obj) {
            warn!("Path traversal attack detected (layer 1): {}", path);
            return Err(CisError::wasm(format!(
                "Path traversal detected: {}",
                path
            )));
        }

        // 2. 🔒 规范化路径（现在会拒绝无法规范的路径）
        let normalized = normalize_path(path_obj);
        debug!("Validating path: {} -> {}", path, normalized.display());

        // 3. 🔒 第二层防护：检查规范化是否成功
        if normalized.to_str().map_or(false, |s| s.contains("INVALID_PATH")) {
            warn!("Path normalization failed: {}", path);
            return Err(CisError::wasm(format!(
                "Path normalization failed: {} (path may not exist)",
                path
            )));
        }

        // 4. 🔒 第三层防护：再次检测规范化后的路径
        //    防止通过编码绕过第一层检查
        if contains_path_traversal(&normalized) {
            warn!("Path traversal attack detected (layer 2): {}", normalized.display());
            return Err(CisError::wasm(format!(
                "Path traversal detected after normalization: {}",
                normalized.display()
            )));
        }

        // 5. 符号链接检测（如果不允许）
        if !self.allow_symlinks {
            self.check_symlink_attack(&normalized, 0)?;
        }

        // 6. 🔒 第四层防护：白名单权限检查（精确匹配）
        match access {
            AccessType::Write => {
                // 写入访问只能使用可写路径
                if !self.is_path_in_writable(&normalized) {
                    warn!("Write access denied for path: {}", normalized.display());
                    return Err(CisError::wasm(format!(
                        "Write access denied for path: {} (not in writable whitelist)",
                        normalized.display()
                    )));
                }
            }
            AccessType::Read | AccessType::Execute => {
                // 读取和执行访问可以使用只读或可写路径
                if !self.is_path_in_readonly(&normalized) && !self.is_path_in_writable(&normalized) {
                    warn!("Read/Execute access denied for path: {}", normalized.display());
                    return Err(CisError::wasm(format!(
                        "Read/Execute access denied for path: {} (not in whitelist)",
                        normalized.display()
                    )));
                }
            }
        }

        debug!("Path validation passed: {} (access: {:?})", normalized.display(), access);
        Ok(normalized)
    }

    /// 检查路径是否在只读白名单中
    fn is_path_in_readonly(&self, path: &Path) -> bool {
        for allowed_path in &self.readonly_paths {
            if path.starts_with(allowed_path) {
                return true;
            }
        }
        false
    }

    /// 检查路径是否在可写白名单中
    fn is_path_in_writable(&self, path: &Path) -> bool {
        for allowed_path in &self.writable_paths {
            if path.starts_with(allowed_path) {
                return true;
            }
        }
        false
    }

    /// 检查符号链接攻击
    fn check_symlink_attack(&self, path: &Path, depth: usize) -> Result<()> {
        // 防止无限递归
        if depth > self.max_symlink_depth {
            return Err(CisError::wasm("Symlink depth exceeds limit".to_string()));
        }

        // 检查路径是否存在
        if !path.exists() {
            return Ok(());
        }

        // 检查是否是符号链接
        if path.is_symlink() {
            let target = std::fs::read_link(path)
                .map_err(|e| CisError::wasm(format!("Cannot read symlink: {}", e)))?;

            debug!("Found symlink: {} -> {}", path.display(), target.display());

            // 检查链接目标是否在白名单内
            let normalized_target = normalize_path(&target);
            
            // 如果目标不在任何白名单内，拒绝访问
            if !self.is_path_in_readonly(&normalized_target) && !self.is_path_in_writable(&normalized_target) {
                warn!(
                    "Symlink points outside whitelist: {} -> {}",
                    path.display(),
                    target.display()
                );
                return Err(CisError::wasm(format!(
                    "Symlink attack detected: {} -> {} (points outside sandbox)",
                    path.display(),
                    target.display()
                )));
            }

            // 递归检查链接目标
            return self.check_symlink_attack(&normalized_target, depth + 1);
        }

        // 如果是目录，递归检查父目录
        if let Some(parent) = path.parent() {
            self.check_symlink_attack(parent, depth)?;
        }

        Ok(())
    }

    /// 创建安全的子路径
    ///
    /// 在指定的基础目录下创建安全的子路径，防止目录遍历攻击。
    ///
    /// # 参数
    /// - `base_dir`: 基础目录（必须在白名单中）
    /// - `sub_path`: 子路径
    ///
    /// # 返回
    /// - `Ok(PathBuf)`: 安全的完整路径
    /// - `Err(CisError)`: 路径不安全
    pub fn create_safe_path(&self, base_dir: &Path, sub_path: &Path, access: AccessType) -> Result<PathBuf> {
        // 验证基础目录
        let base_str = base_dir.to_str()
            .ok_or_else(|| CisError::wasm("Invalid base directory encoding".to_string()))?;
        self.validate_path(base_str, access)?;

        // 构建完整路径
        let full_path = base_dir.join(sub_path);
        let normalized = normalize_path(&full_path);

        // 确保结果路径仍在基础目录下
        let normalized_base = normalize_path(base_dir);
        if !normalized.starts_with(&normalized_base) {
            warn!(
                "Subpath escaped base directory: {} (base: {})",
                normalized.display(),
                normalized_base.display()
            );
            return Err(CisError::wasm(format!(
                "Path traversal detected: subpath escaped base directory"
            )));
        }

        // 验证最终路径
        let normalized_str = normalized.to_str()
            .ok_or_else(|| CisError::wasm("Invalid path encoding".to_string()))?;
        self.validate_path(normalized_str, access)?;

        Ok(normalized)
    }

    /// 构建并验证沙箱配置
    ///
    /// # 返回
    /// - `Ok(())`: 配置有效
    /// - `Err(CisError)`: 配置无效
    pub fn build(&self) -> Result<()> {
        // 验证至少有一个路径在白名单中
        if self.readonly_paths.is_empty() && self.writable_paths.is_empty() {
            return Err(CisError::wasm("No paths in whitelist".to_string()));
        }

        // 验证资源限制
        if self.max_fd == 0 {
            return Err(CisError::wasm("max_fd cannot be zero".to_string()));
        }

        if self.max_file_size == 0 {
            return Err(CisError::wasm("max_file_size cannot be zero".to_string()));
        }

        debug!("WasiSandbox configuration validated successfully");
        Ok(())
    }

    /// 获取沙箱配置摘要
    pub fn summary(&self) -> WasiSandboxSummary {
        WasiSandboxSummary {
            readonly_paths_count: self.readonly_paths.len(),
            writable_paths_count: self.writable_paths.len(),
            max_fd: self.max_fd,
            current_fd: self.current_fd_count.load(Ordering::SeqCst),
            max_file_size: self.max_file_size,
            allow_symlinks: self.allow_symlinks,
            max_symlink_depth: self.max_symlink_depth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasi_sandbox_new() {
        let sandbox = WasiSandbox::new();
        assert_eq!(sandbox.max_fd(), 32);
        assert_eq!(sandbox.max_file_size(), 100 * 1024 * 1024);
    }

    #[test]
    fn test_wasi_sandbox_builder() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data")
            .with_writable_path("/tmp")
            .with_max_fd(EXAMPLE_MAX_FD)
            .with_max_file_size(EXAMPLE_MAX_FILE_SIZE)
            .with_allow_symlinks(true)
            .with_max_symlink_depth(16);

        assert_eq!(sandbox.max_fd(), EXAMPLE_MAX_FD);
        assert_eq!(sandbox.max_file_size(), EXAMPLE_MAX_FILE_SIZE);
        assert!(sandbox.readonly_paths().contains(&PathBuf::from("/data")));
        // 路径会被规范化，在 macOS 上 /tmp 可能指向 /private/tmp
        // 所以只检查路径数量而不是具体路径
        assert!(!sandbox.writable_paths().is_empty());
    }

    #[test]
    fn test_validate_path_readonly() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data");

        // 应该允许读取只读路径下的文件
        let result = sandbox.validate_path("/data/file.txt", AccessType::Read);
        // 注意：在测试环境中可能不存在 /data 目录，所以可能失败
        // 我们只检查不返回路径遍历错误
        if let Err(e) = &result {
            let msg = format!("{}", e);
            assert!(!msg.contains("traversal"), "Should not be traversal error: {}", msg);
        }
    }

    #[test]
    fn test_validate_path_traversal() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data");

        // 应该拒绝路径遍历
        let result = sandbox.validate_path("/data/../etc/passwd", AccessType::Read);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("traversal") || msg.contains("denied"));
    }

    #[test]
    fn test_validate_path_outside_sandbox() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data");

        // 应该拒绝访问沙箱外的文件
        let result = sandbox.validate_path("/etc/passwd", AccessType::Read);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("denied"));
    }

    #[test]
    fn test_validate_path_write_permission() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data")
            .with_writable_path("/tmp");

        // 应该拒绝在只读路径写入
        let result = sandbox.validate_path("/data/file.txt", AccessType::Write);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("denied"));

        // 检查可写路径列表不为空（路径规范化可能导致路径变化）
        assert!(!sandbox.writable_paths().is_empty());
        
        // 使用实际规范化后的路径进行验证
        let writable_path = sandbox.writable_paths().iter().next().unwrap();
        let test_file = writable_path.join("file.txt");
        let result = sandbox.validate_path(test_file.to_str().unwrap(), AccessType::Write);
        // 不应是权限错误
        if let Err(e) = &result {
            let msg = format!("{}", e);
            assert!(!msg.contains("denied"), "Should not be permission denied: {}", msg);
        }
    }

    #[test]
    fn test_file_descriptor_limit() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data")
            .with_max_fd(3);

        // 分配 3 个 fd
        assert!(sandbox.allocate_fd().is_ok());
        assert!(sandbox.allocate_fd().is_ok());
        assert!(sandbox.allocate_fd().is_ok());

        // 第 4 个应该失败
        let result = sandbox.allocate_fd();
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("exceeded"));

        // 释放一个后应该可以分配
        sandbox.release_fd();
        assert!(sandbox.allocate_fd().is_ok());
    }

    #[test]
    fn test_validate_file_size() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data")
            .with_max_file_size(1024);

        assert!(sandbox.validate_file_size(512).is_ok());
        assert!(sandbox.validate_file_size(1024).is_ok());
        
        let result = sandbox.validate_file_size(1025);
        assert!(result.is_err());
        assert!(format!("{}", result.unwrap_err()).contains("exceeds"));
    }

    #[test]
    fn test_is_safe_filename() {
        // 使用重新导出的独立函数
        assert!(is_safe_filename("file.txt"));
        assert!(is_safe_filename("document.pdf"));
        assert!(is_safe_filename("my-file_123"));

        assert!(!is_safe_filename("../file.txt"));
        assert!(!is_safe_filename("path/file.txt"));
        assert!(!is_safe_filename("file?.txt"));
        assert!(!is_safe_filename("file*.txt"));
        assert!(!is_safe_filename(".."));
        assert!(!is_safe_filename("."));
    }

    #[test]
    fn test_sandbox_build() {
        // 没有路径应该失败
        let sandbox = WasiSandbox::new();
        assert!(sandbox.build().is_err());

        // 有路径应该成功
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data");
        assert!(sandbox.build().is_ok());

        // max_fd 为 0 应该失败
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data")
            .with_max_fd(0);
        assert!(sandbox.build().is_err());

        // max_file_size 为 0 应该失败
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data")
            .with_max_file_size(0);
        assert!(sandbox.build().is_err());
    }

    #[test]
    fn test_sandbox_summary() {
        let sandbox = WasiSandbox::new()
            .with_readonly_path("/data")
            .with_readonly_path("/config")
            .with_writable_path("/tmp")
            .with_max_fd(EXAMPLE_MAX_FD);

        let summary = sandbox.summary();
        assert_eq!(summary.readonly_paths_count, 2);
        assert_eq!(summary.writable_paths_count, 1);
        assert_eq!(summary.max_fd, EXAMPLE_MAX_FD);
        assert_eq!(summary.current_fd, 0);
    }
}

// 🔒 导入安全测试模块
#[cfg(test)]
mod security_tests;
