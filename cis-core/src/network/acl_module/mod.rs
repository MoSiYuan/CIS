//! ACL (Access Control List) 模块
//!
//! 包含 ACL 条目的验证、签名和管理。

// 🔒 引用 acl/ 子目录
pub mod acl {
    include!("acl/mod.rs");
}

// Re-export 主要类型
pub use acl::{AclEntry, AclResult, AclSummary, NetworkAcl, NetworkMode};
pub use acl::{AclSigner, AclVerifier};
pub use acl::{AclValidator, AclValidationResult};
