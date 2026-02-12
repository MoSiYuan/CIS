//! # CLI Module
//!
//! Refactored command-line interface with grouped commands and extensible architecture.

pub mod command;
pub mod context;
pub mod error;
pub mod output;
pub mod progress;
pub mod registry;

pub mod groups;
pub mod handlers;

pub use command::{Command, CommandCategory, Example};
pub use context::CommandContext;
pub use error::CommandError;
pub use output::CommandOutput;
pub use registry::CommandRegistry;
