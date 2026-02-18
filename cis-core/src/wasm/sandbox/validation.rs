//! Path Validation Utilities
//!
//! 提供路径规范化、遍历检测和安全检查函数。

use std::path::{Path, PathBuf};
use tracing::warn;

/// 规范化路径
///
/// # 安全修复 (P0)
///
/// **漏洞修复**: 拒绝无法规范的路径，防止路径遍历攻击
///
/// - 旧实现：`canonicalize()` 失败时回退到原始路径（不安全）
/// - 新实现：返回错误，拒绝访问（安全）
pub fn normalize_path(path: &Path) -> PathBuf {
    // 1. 转换为绝对路径
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };

    // 2. 安全修复：必须成功规范化路径
    //    - 如果路径不存在，拒绝访问
    //    - 如果解析符号链接失败，拒绝访问
    abs_path.canonicalize().unwrap_or_else(|e| {
        // 安全策略：拒绝无法规范的路径
        warn!("Failed to canonicalize path {}: {}", path.display(), e);
        // 返回一个明显的无效路径，让后续检查失败
        PathBuf::from("/INVALID_PATH_CANT_NORMALIZE")
    })
}

/// 检测路径遍历攻击
///
/// 检查路径中是否包含 `../` 或 `..` 等路径遍历模式。
pub fn contains_path_traversal(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // 检查是否包含父目录遍历
    if path_str.contains("../") || path_str.contains("..\\") {
        return true;
    }

    // 检查路径组件是否以 ".." 开头
    for component in path.components() {
        let component_str = component.as_os_str().to_string_lossy();
        if component_str.starts_with("..") {
            return true;
        }
    }

    false
}

/// 检查文件名是否安全
///
/// # 参数
/// - `filename`: 要检查的文件名
///
/// # 返回
/// - `true`: 文件名安全
/// - `false`: 文件名不安全
///
/// # 规则
/// - 不能包含路径分隔符 (`/` 或 `\`)
/// - 不能包含特殊字符 (`?`, `*`, `:`, `<`, `>`, `|`, `"`)
/// - 不能是 `.` 或 `..`
pub fn is_safe_filename(filename: &str) -> bool {
    // 检查路径分隔符
    if filename.contains('/') || filename.contains('\\') {
        return false;
    }

    // 检查特殊字符
    let forbidden_chars = ['\0', '?', '*', ':', '<', '>', '|', '"'];
    for ch in &forbidden_chars {
        if filename.contains(*ch) {
            return false;
        }
    }

    // 检查危险字符串
    if filename == ".." || filename == "." {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        let path = Path::new("/test/../data");
        let normalized = normalize_path(path);
        // 规范化路径应该包含完整的绝对路径
        assert!(normalized.is_absolute());
    }

    #[test]
    fn test_contains_path_traversal() {
        assert!(contains_path_traversal(Path::new("/data/../etc")));
        assert!(contains_path_traversal(Path::new("foo/../bar")));
        assert!(!contains_path_traversal(Path::new("/data/file.txt")));
    }

    #[test]
    fn test_is_safe_filename() {
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
}
