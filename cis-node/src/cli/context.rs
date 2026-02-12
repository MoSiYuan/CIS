//! # Command Context
//!
//! Execution context for CLI commands.

use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::cli::command::GlobalOptions;
use cis_core::config::GlobalConfig;

/// Command execution context
pub struct CommandContext {
    /// Global options passed to all commands
    pub global_opts: GlobalOptions,

    /// CIS global configuration
    pub config: Arc<GlobalConfig>,

    /// Tokio runtime (optional, for async commands)
    pub runtime: Option<Arc<Runtime>>,

    /// Current working directory
    pub work_dir: std::path::PathBuf,
}

impl CommandContext {
    /// Create a new command context
    pub fn new(
        global_opts: GlobalOptions,
        config: Arc<GlobalConfig>,
        runtime: Option<Arc<Runtime>>,
    ) -> Self {
        Self {
            global_opts,
            config,
            runtime,
            work_dir: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        }
    }

    /// Create a minimal context (for testing)
    pub fn minimal() -> Self {
        Self {
            global_opts: GlobalOptions {
                json: false,
                verbose: false,
                quiet: false,
            },
            config: Arc::new(GlobalConfig::default()),
            runtime: None,
            work_dir: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        }
    }

    /// Check if running in JSON mode
    pub fn is_json(&self) -> bool {
        self.global_opts.json
    }

    /// Check if running in verbose mode
    pub fn is_verbose(&self) -> bool {
        self.global_opts.verbose
    }

    /// Check if running in quiet mode
    pub fn is_quiet(&self) -> bool {
        self.global_opts.quiet
    }

    /// Get the tokio runtime (or create a default one)
    pub fn runtime(&self) -> Arc<Runtime> {
        self.runtime.clone().unwrap_or_else(|| {
            Arc::new(
                Runtime::new()
                    .expect("Failed to create tokio runtime")
            )
        })
    }

    /// Print verbose message if verbose mode is enabled
    pub fn verbose(&self, msg: &str) {
        if self.is_verbose() {
            eprintln!("[VERBOSE] {}", msg);
        }
    }
}

impl Default for CommandContext {
    fn default() -> Self {
        Self::minimal()
    }
}
