//! # Memory Command Group
//!
//! Memory storage and retrieval operations

use clap::{Parser, Subcommand};

use crate::cli::command::{Command, CommandCategory, CommandContext, CommandOutput, CommandError, Example};
use crate::cli::handlers::memory;

/// Memory commands - store, retrieve, and search memories
#[derive(Parser, Debug)]
pub struct MemoryGroup {
    #[command(subcommand)]
    pub action: MemoryAction,
}

impl MemoryGroup {
    pub fn examples() -> Vec<Example> {
        vec![
            Example {
                command: "cis memory get user.preference.theme".to_string(),
                description: "Get a memory value".to_string(),
            },
            Example {
                command: "cis memory set user.preference.theme dark --domain public".to_string(),
                description: "Set a memory value in the public domain".to_string(),
            },
            Example {
                command: "cis memory search \"theme\"".to_string(),
                description: "Search memories by keyword".to_string(),
            },
            Example {
                command: "cis memory vector \"what theme did i prefer\"".to_string(),
                description: "Semantic search using vector similarity".to_string(),
            },
            Example {
                command: "cis memory list --prefix user".to_string(),
                description: "List all memories with a prefix".to_string(),
            },
        ]
    }
}

/// Memory command actions
#[derive(Subcommand, Debug, Clone)]
pub enum MemoryAction {
    /// Get a memory value
    Get {
        /// Memory key
        key: String,
    },

    /// Set a memory value
    Set {
        /// Memory key
        key: String,
        /// Memory value
        value: String,
        /// Memory domain (public/private)
        #[arg(long, value_enum, default_value = "public")]
        domain: MemoryDomain,
        /// Memory category
        #[arg(long, value_enum, default_value = "context")]
        category: MemoryCategory,
        /// Create semantic index for search
        #[arg(long)]
        index: bool,
    },

    /// Delete a memory entry
    Delete {
        /// Memory key
        key: String,
    },

    /// Search memory entries (keyword-based)
    Search {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Semantic search using vector embeddings
    Vector {
        /// Natural language query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "5")]
        limit: usize,
        /// Similarity threshold (0.0-1.0)
        #[arg(short, long)]
        threshold: Option<f32>,
        /// Category filter
        #[arg(short, long)]
        category: Option<String>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "plain")]
        format: OutputFormat,
    },

    /// List memory keys
    List {
        /// Key prefix filter
        #[arg(long)]
        prefix: Option<String>,
        /// Domain filter
        #[arg(long, value_enum)]
        domain: Option<MemoryDomain>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "plain")]
        format: OutputFormat,
    },

    /// Export public memory
    Export {
        /// Export since timestamp (Unix epoch)
        #[arg(long)]
        since: Option<i64>,
        /// Output file (default: stdout)
        #[arg(long, short)]
        output: Option<String>,
    },

    /// Import memories from file
    Import {
        /// Input file
        #[arg(long, short)]
        input: String,
        /// Merge with existing data
        #[arg(long)]
        merge: bool,
    },

    /// Show memory statistics
    Stats {
        /// Show by domain
        #[arg(long)]
        by_domain: bool,
        /// Show by category
        #[arg(long)]
        by_category: bool,
    },
}

/// Memory domain
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MemoryDomain {
    Public,
    Private,
}

impl From<MemoryDomain> for cis_core::types::MemoryDomain {
    fn from(domain: MemoryDomain) -> Self {
        match domain {
            MemoryDomain::Public => cis_core::types::MemoryDomain::Public,
            MemoryDomain::Private => cis_core::types::MemoryDomain::Private,
        }
    }
}

/// Memory category
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MemoryCategory {
    Execution,
    Result,
    Error,
    Context,
    Skill,
}

impl From<MemoryCategory> for cis_core::types::MemoryCategory {
    fn from(category: MemoryCategory) -> Self {
        match category {
            MemoryCategory::Execution => cis_core::types::MemoryCategory::Execution,
            MemoryCategory::Result => cis_core::types::MemoryCategory::Result,
            MemoryCategory::Error => cis_core::types::MemoryCategory::Error,
            MemoryCategory::Context => cis_core::types::MemoryCategory::Context,
            MemoryCategory::Skill => cis_core::types::MemoryCategory::Skill,
        }
    }
}

/// Output format
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Plain,
    Json,
    Table,
}

impl Command for MemoryAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Get { .. } => "get",
            Self::Set { .. } => "set",
            Self::Delete { .. } => "delete",
            Self::Search { .. } => "search",
            Self::Vector { .. } => "vector",
            Self::List { .. } => "list",
            Self::Export { .. } => "export",
            Self::Import { .. } => "import",
            Self::Stats { .. } => "stats",
        }
    }

    fn about(&self) -> &'static str {
        match self {
            Self::Get { .. } => "Get a memory value",
            Self::Set { .. } => "Set a memory value",
            Self::Delete { .. } => "Delete a memory entry",
            Self::Search { .. } => "Search memory entries by keyword",
            Self::Vector { .. } => "Semantic search using vector embeddings",
            Self::List { .. } => "List memory keys",
            Self::Export { .. } => "Export public memory",
            Self::Import { .. } => "Import memories from file",
            Self::Stats { .. } => "Show memory statistics",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
        match self {
            Self::Get { key } => memory::get::execute(key, ctx),
            Self::Set { key, value, domain, category, index } => {
                memory::set::execute(key, value, *domain, *category, *index, ctx)
            }
            Self::Delete { key } => memory::delete::execute(key, ctx),
            Self::Search { query, limit } => memory::search::execute(query, *limit, ctx),
            Self::Vector { query, limit, threshold, category, format } => {
                memory::vector::execute(query, *limit, *threshold, category.clone(), *format, ctx)
            }
            Self::List { prefix, domain, format } => {
                memory::list::execute(prefix.clone(), domain.clone(), *format, ctx)
            }
            Self::Export { since, output } => memory::export::execute(*since, output.clone(), ctx),
            Self::Import { input, merge } => memory::import::execute(input.clone(), *merge, ctx),
            Self::Stats { by_domain, by_category } => memory::stats::execute(*by_domain, *by_category, ctx),
        }
    }

    fn examples(&self) -> Vec<Example> {
        MemoryGroup::examples()
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Memory
    }
}
