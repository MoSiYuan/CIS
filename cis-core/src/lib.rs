//! # CIS Core Library
//!
//! Core library for CIS (Cluster of Independent Systems).
//!
//! This library provides the foundational components for building
//! hardware-bound, sovereign distributed computing systems.
//!
//! ## Architecture
//!
//! - **Types**: Core data structures and domain types
//! - **Sandbox**: Security and path validation
//! - **Scheduler**: DAG-based task scheduling
//! - **Storage**: Cross-platform storage with data isolation and hot-swappable skills
//! - **Skill**: Hot-swappable skill management
//! - **Agent**: LLM Agent abstraction with bidirectional integration
//! - **Project**: Project-level configuration and local skills
//! - **P2P**: Peer-to-peer communication (planned)
//! - **Identity**: DID management (planned)
//!
//! ## Philosophy
//!
//! CIS follows first principles and Occam's razor:
//! - Hardware-binding for identity
//! - Local memory sovereignty
//! - Zero-token inter-node communication
//! - Pure P2P architecture (no central coordinator)
//!
//! ## Bidirectional Agent Integration
//!
//! CIS serves as infrastructure with memory as data. The integration is bidirectional:
//!
//! ```text
//! CIS → Agent: CIS calls external LLM Agent through AgentProvider
//! Agent → CIS: External Agent calls CIS through CLI/API
//! ```
//!
//! ## Phase 3 Migration Note
//!
//! The following modules have been migrated to cis-common workspace crates:
//! - `types` → `cis-types`
//! - `storage` → `cis-storage`
//! - `memory` → `cis-memory`
//! - `scheduler` → `cis-scheduler`
//! - `vector` → `cis-vector`
//! - `p2p` → `cis-p2p`
//!
//! For backward compatibility, these modules are re-exported from cis-common.

pub use cis_types::*;
pub use cis_traits::*;

// MODULES MIGRATED TO CIS-COMMON (Phase 3)
// ========================================
// The following modules have been migrated to cis-common workspace crates:
// - types → cis-types
// - storage → cis-storage
// - memory → cis-memory
// - scheduler → cis-scheduler
// - vector → cis-vector
// - p2p → cis-p2p
//
// For backward compatibility, these modules are now re-exported via cis_types/cis_traits.
// Original module files are kept for reference not be used directly but should.

// pub mod types;  // MIGRATED → cis-types (use cis_types::* instead)
// pub mod storage;  // MIGRATED → cis-storage (use cis_storage::* instead)
// pub mod memory;  // MIGRATED → cis-memory (use cis_memory::* instead)
// pub mod scheduler;  // MIGRATED → cis-scheduler (use cis_scheduler::* instead)
// pub mod vector;  // MIGRATED → cis-vector (use cis_vector::* instead)
// pub mod p2p;  // MIGRATED → cis-p2p (use cis_p2p::* instead)

pub mod error;

// Lock timeout module - Lock management with timeout and monitoring
pub mod lock_timeout;

// Configuration module - unified configuration center
pub mod config;

// Core modules (Note: scheduler, memory, storage migrated to cis-common)
// pub mod scheduler;  // MIGRATED → cis-scheduler
// pub mod memory;  // MIGRATED → cis-memory
// pub mod storage;  // MIGRATED → cis-storage

// Sandbox module - Security and path validation
pub mod sandbox;

// Cache module - LRU cache for memory queries
pub mod cache;

// Skill module - Hot-swappable skill management
pub mod skill;

// Intent module - Natural language intent parsing
pub mod intent;

// AI module - AI Provider implementations
pub mod ai;

// Agent module - LLM Agent abstraction with bidirectional integration
pub mod agent;

// Project module - Project-level configuration
pub mod project;

// Wizard module - Initialization and onboarding
pub mod wizard;

// Init module - New initialization wizard with full flow
pub mod init;

// WASM module - WASM Runtime for sandboxed skills
#[cfg(feature = "wasm")]
pub mod wasm;

// Matrix protocol integration
pub mod matrix;

// Identity module - DID management
pub mod identity;

// Vector Intelligence module - Semantic search (MIGRATED to cis-common)
// pub mod vector;  // MIGRATED → cis-vector

// Conversation module - Session dialogue management
pub mod conversation;

// Telemetry module - Request logging and observability
pub mod telemetry;

// Task module - Task management and vector indexing
pub mod task;

// P2P module - Peer-to-peer networking (MIGRATED to cis-common)
// #[cfg(feature = "p2p")]
// pub mod p2p;  // MIGRATED → cis-p2p

// Network module - Access control and DID-based admission
pub mod network;

// GLM module - Cloud node API for GLM integration
pub mod glm;

// Service layer - Unified data access for CLI, GUI, API
pub mod service;

// Dependency injection container
pub mod container;

// Events module - Domain events for decoupled communication
pub mod events;

// Event bus module - Event pub/sub mechanism
pub mod event_bus;

// Service traits - Abstract interfaces for dependency injection
pub mod traits;

// CLI module - AI-Native CLI framework
pub mod cli;

// Decision module - Four-tier decision mechanism
pub mod decision;

// Engine scanner module - Game engine detection and code injection
pub mod engine;

// Test framework with mocks (only for testing)
#[cfg(any(test, feature = "test-utils"))]
pub mod test;
#[cfg(any(test, feature = "test-utils"))]
pub use test::mocks::{MockNetworkService, MockStorageService, MockEventBus, MockAiProvider, MockEmbeddingService, MockSkillExecutor};

pub use error::{CisError, Result};
pub use identity::DIDManager;

/// 安全边界检查工具函数
///
/// 验证索引是否在有效范围内，防止越界访问
/// 
/// # Arguments
/// * `index` - 要检查的索引
/// * `len` - 缓冲区/数组长度
/// 
/// # Returns
/// * `Ok(())` - 索引有效
/// * `Err(CisError)` - 索引越界
/// 
/// # Examples
/// ```
/// use cis_core::check_bounds;
/// 
/// let buf = vec![1, 2, 3];
/// assert!(check_bounds(0, buf.len()).is_ok());
/// assert!(check_bounds(3, buf.len()).is_err()); // 越界
/// ```
pub fn check_bounds(index: usize, len: usize) -> Result<()> {
    if index >= len {
        return Err(CisError::invalid_input(format!(
            "Index {} out of bounds (len: {})",
            index, len
        )));
    }
    Ok(())
}

/// 安全切片访问
///
/// 安全地获取切片的一部分，带边界检查
/// 
/// # Arguments
/// * `data` - 原始数据切片
/// * `start` - 起始索引
/// * `end` - 结束索引（不包含）
/// 
/// # Returns
/// * `Ok(&[T])` - 切片引用
/// * `Err(CisError)` - 参数无效或越界
/// 
/// # Examples
/// ```
/// use cis_core::safe_slice;
/// 
/// let data = vec![1, 2, 3, 4, 5];
/// let slice = safe_slice(&data, 1, 4).unwrap();
/// assert_eq!(slice, &[2, 3, 4]);
/// ```
pub fn safe_slice<T>(data: &[T], start: usize, end: usize) -> Result<&[T]> {
    if start > end {
        return Err(CisError::invalid_input(format!(
            "Invalid slice range: start ({}) > end ({})",
            start, end
        )));
    }
    if end > data.len() {
        return Err(CisError::invalid_input(format!(
            "Slice end {} out of bounds (len: {})",
            end, data.len()
        )));
    }
    Ok(&data[start..end])
}

/// 验证 WASM 字节码魔术数字
///
/// 确保 WASM 模块以正确的魔术数字开头，防止加载无效数据
/// 
/// # Arguments
/// * `wasm_bytes` - WASM 字节码
/// 
/// # Returns
/// * `Ok(())` - 魔术数字有效
/// * `Err(CisError)` - 无效的 WASM 数据
/// 
/// # Examples
/// ```
/// use cis_core::validate_wasm_magic;
/// 
/// let valid_wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
/// assert!(validate_wasm_magic(&valid_wasm).is_ok());
/// 
/// let invalid = vec![0x00, 0x00, 0x00, 0x00];
/// assert!(validate_wasm_magic(&invalid).is_err());
/// ```
pub fn validate_wasm_magic(wasm_bytes: &[u8]) -> Result<()> {
    const WASM_MAGIC: &[u8] = &[0x00, 0x61, 0x73, 0x6d];
    
    if wasm_bytes.len() < WASM_MAGIC.len() {
        return Err(CisError::invalid_input(
            "WASM data too short to contain magic number".to_string()
        ));
    }
    
    if &wasm_bytes[..WASM_MAGIC.len()] != WASM_MAGIC {
        return Err(CisError::invalid_input(
            "Invalid WASM magic number".to_string()
        ));
    }
    
    Ok(())
}

/// 安全的内存分配大小检查
///
/// 防止分配过大的内存块导致资源耗尽
/// 
/// # Arguments
/// * `size` - 请求的内存大小
/// * `max_size` - 允许的最大大小
/// 
/// # Returns
/// * `Ok(())` - 大小在允许范围内
/// * `Err(CisError)` - 大小超过限制
/// 
/// # Examples
/// ```
/// use cis_core::check_allocation_size;
/// 
/// assert!(check_allocation_size(1024, 64 * 1024 * 1024).is_ok());
/// assert!(check_allocation_size(128 * 1024 * 1024, 64 * 1024 * 1024).is_err());
/// ```
pub fn check_allocation_size(size: usize, max_size: usize) -> Result<()> {
    if size == 0 {
        return Err(CisError::invalid_input(
            "Allocation size cannot be zero".to_string()
        ));
    }
    
    if size > max_size {
        return Err(CisError::invalid_input(format!(
            "Allocation size {} exceeds maximum allowed {}",
            size, max_size
        )));
    }
    
    Ok(())
}

/// 安全的字符串长度检查
///
/// 验证字符串长度是否在合理范围内，防止内存耗尽攻击
/// 
/// # Arguments
/// * `s` - 输入字符串
/// * `max_len` - 允许的最大长度
/// 
/// # Returns
/// * `Ok(())` - 长度有效
/// * `Err(CisError)` - 长度超过限制
/// 
/// # Examples
/// ```
/// use cis_core::check_string_length;
/// 
/// assert!(check_string_length("hello", 100).is_ok());
/// assert!(check_string_length("a".repeat(1000).as_str(), 100).is_err());
/// ```
pub fn check_string_length(s: &str, max_len: usize) -> Result<()> {
    if s.len() > max_len {
        return Err(CisError::invalid_input(format!(
            "String length {} exceeds maximum allowed {}",
            s.len(), max_len
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_bounds_valid() {
        assert!(check_bounds(0, 10).is_ok());
        assert!(check_bounds(9, 10).is_ok());
    }

    #[test]
    fn test_check_bounds_invalid() {
        assert!(check_bounds(10, 10).is_err());
        assert!(check_bounds(11, 10).is_err());
    }

    #[test]
    fn test_safe_slice_valid() {
        let data = vec![1, 2, 3, 4, 5];
        assert_eq!(safe_slice(&data, 0, 5).unwrap(), &[1, 2, 3, 4, 5]);
        assert_eq!(safe_slice(&data, 1, 3).unwrap(), &[2, 3]);
    }

    #[test]
    fn test_safe_slice_invalid() {
        let data = vec![1, 2, 3];
        assert!(safe_slice(&data, 2, 1).is_err()); // start > end
        assert!(safe_slice(&data, 0, 5).is_err()); // end > len
    }

    #[test]
    fn test_validate_wasm_magic_valid() {
        let valid_wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        assert!(validate_wasm_magic(&valid_wasm).is_ok());
    }

    #[test]
    fn test_validate_wasm_magic_invalid() {
        let invalid = vec![0x00, 0x00, 0x00, 0x00];
        assert!(validate_wasm_magic(&invalid).is_err());
        
        let too_short = vec![0x00, 0x61];
        assert!(validate_wasm_magic(&too_short).is_err());
    }

    #[test]
    fn test_check_allocation_size() {
        assert!(check_allocation_size(1024, 64 * 1024 * 1024).is_ok());
        assert!(check_allocation_size(0, 100).is_err());
        assert!(check_allocation_size(101, 100).is_err());
    }

    #[test]
    fn test_check_string_length() {
        assert!(check_string_length("hello", 100).is_ok());
        assert!(check_string_length("", 100).is_ok());
        assert!(check_string_length("a".repeat(1000).as_str(), 100).is_err());
    }
}
