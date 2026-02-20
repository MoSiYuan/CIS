//! WASM模块
//!
//! 包含WASM运行时、沙箱和host函数接口

pub mod sandbox;
pub mod host;
pub mod runtime;

pub use sandbox::{WasiSandbox, AccessType, FileDescriptorGuard};
pub use host::{HostContext, HostFunctions};
pub use runtime::WasmRuntime;
