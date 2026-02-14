//! # CIS Engine Scanner Module
//!
//! This module provides code scanning and injection point detection for game engines.
//!
//! ## Supported Engines
//!
//! - **Unreal Engine 5.7**: Full C++ and Blueprint scanning
//! - **Unity 2022**: C# and Unity-specific API detection
//! - **Godot 4.x**: GDScript and resource loading patterns
//! - **Custom**: Extensible pattern matching for custom engines
//!
//! ## Core Components
//!
//! - [`scanner`] - Main engine scanning implementation
//! - [`patterns`] - Injection pattern library
//! - [`types`] - Engine and injection data structures
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use cis_core::engine::{EngineScanner, EngineType};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let scanner = EngineScanner::new();
//! let results = scanner.scan_directory(PathBuf::from("/path/to/project")).await?;
//!
//! for location in results {
//!     println!("Found injection point: {:?} at {:?}:{}",
//!         location.injection_type,
//!         location.file_path,
//!         location.line_number
//!     );
//! }
//! # Ok(())
//! # }
//! ```

pub mod scanner;
pub mod patterns;
pub mod types;

// Re-export main types
pub use types::{
    EngineType,
    EngineInfo,
    InjectibleLocation,
    InjectionType,
    InjectionPattern,
};

pub use scanner::EngineScanner;
pub use patterns::PatternLibrary;
