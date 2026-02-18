//! # Error Context
//!
//! Additional context for errors, including key-value pairs and source location.

use std::fmt;
use std::backtrace::Backtrace;
use std::panic::Location;

/// Additional context for an error
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Key-value pairs providing context
    pub context: Vec<(String, String)>,

    /// Source code location where error was created
    pub location: Option<&'static Location<'static>>,

    /// Backtrace if available
    pub backtrace: Option<Backtrace>,
}

impl ErrorContext {
    /// Create a new empty error context
    pub fn new() -> Self {
        Self {
            context: Vec::new(),
            location: None,
            backtrace: std::backtrace::Backtrace::capture().into(),
        }
    }

    /// Add a key-value pair to the context
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    /// Set the source location
    pub fn with_location(mut self, loc: &'static Location<'static>) -> Self {
        self.location = Some(loc);
        self
    }

    /// Get a context value by key
    pub fn get(&self, key: &str) -> Option<&String> {
        self.context.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.context.is_empty() {
            return Ok(());
        }

        writeln!(f, "\nContext:")?;
        for (key, value) in &self.context {
            writeln!(f, "  {}: {}", key, value)?;
        }

        if let Some(loc) = self.location {
            writeln!(f, "Location: {}:{}:{}", loc.file(), loc.line(), loc.column())?;
        }

        Ok(())
    }
}
