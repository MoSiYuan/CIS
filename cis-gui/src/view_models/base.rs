//! Base ViewModel trait and common functionality

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Base trait for all ViewModels
pub trait ViewModel: Send + Sync {
    /// Get the name of this ViewModel
    fn name(&self) -> &str;

    /// Notify that UI should refresh
    fn notify(&self) {
        // Default implementation does nothing
        // Subclasses can override to trigger UI updates
    }

    /// Check if UI needs to be refreshed
    fn needs_refresh(&self) -> bool {
        false
    }

    /// Mark as needing refresh
    fn mark_dirty(&self) {
        // Default implementation does nothing
    }

    /// Get the last update time
    fn last_update(&self) -> Option<Instant> {
        None
    }
}

/// Common state for ViewModels
#[derive(Clone, Debug)]
pub struct ViewModelState {
    /// Whether the ViewModel needs a UI refresh
    pub dirty: Arc<AtomicBool>,
    /// Last update timestamp
    pub last_update: Option<Instant>,
}

impl ViewModelState {
    pub fn new() -> Self {
        Self {
            dirty: Arc::new(AtomicBool::new(false)),
            last_update: None,
        }
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    pub fn mark_clean(&self) {
        self.dirty.store(false, Ordering::SeqCst);
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    pub fn update_timestamp(&mut self) {
        self.last_update = Some(Instant::now());
        self.mark_dirty();
    }
}

impl Default for ViewModelState {
    fn default() -> Self {
        Self::new()
    }
}
