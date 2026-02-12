//! # Progress Indicators
//!
//! Utilities for showing progress during long-running operations.

use std::time::Duration;

/// Progress bar trait for abstraction over different progress implementations
pub trait Progress: Send + Sync {
    /// Set the current message
    fn set_message(&self, msg: &str);

    /// Increment the progress by 1
    fn inc(&self, delta: u64);

    /// Set the current progress
    fn set_position(&self, pos: u64);

    /// Finish the progress bar with a message
    fn finish(&self, msg: &str);

    /// Finish the progress bar with a success style
    fn finish_with_message(&self, msg: &str);

    /// Abandon the progress bar (error state)
    fn abandon(&self, msg: &str);
}

/// Progress bar implementation using indicatif
#[cfg(feature = "progress")]
pub struct ProgressBar {
    inner: indicatif::ProgressBar,
}

#[cfg(feature = "progress")]
impl ProgressBar {
    /// Create a new progress bar with a total count
    pub fn new(total: u64) -> Self {
        let pb = indicatif::ProgressBar::new(total);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .progress_chars("##>-")
        );
        Self { inner: pb }
    }

    /// Create a spinner for indeterminate progress
    pub fn spinner() -> Self {
        let pb = indicatif::ProgressBar::new_spinner();
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
        );
        Self { inner: pb }
    }

    /// Enable steady tick for spinner
    pub fn enable_steady_tick(&self, duration: Duration) {
        self.inner.enable_steady_tick(duration);
    }
}

#[cfg(feature = "progress")]
impl Progress for ProgressBar {
    fn set_message(&self, msg: &str) {
        self.inner.set_message(msg.to_string());
    }

    fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    fn set_position(&self, pos: u64) {
        self.inner.set_position(pos);
    }

    fn finish(&self, msg: &str) {
        self.inner.finish_with_message(msg.to_string());
    }

    fn finish_with_message(&self, msg: &str) {
        self.inner.finish_with_message(msg.to_string());
    }

    fn abandon(&self, msg: &str) {
        self.inner.abandon_with_message(msg.to_string());
    }
}

/// No-op progress implementation (when progress feature is disabled)
#[cfg(not(feature = "progress"))]
pub struct ProgressBar {
    _private: (),
}

#[cfg(not(feature = "progress"))]
impl ProgressBar {
    pub fn new(_total: u64) -> Self {
        Self { _private: () }
    }

    pub fn spinner() -> Self {
        Self { _private: () }
    }

    pub fn enable_steady_tick(&self, _duration: Duration) {}
}

#[cfg(not(feature = "progress"))]
impl Progress for ProgressBar {
    fn set_message(&self, _msg: &str) {}

    fn inc(&self, _delta: u64) {}

    fn set_position(&self, _pos: u64) {}

    fn finish(&self, _msg: &str) {}

    fn finish_with_message(&self, _msg: &str) {}

    fn abandon(&self, _msg: &str) {}
}

/// Create a progress bar with the given total
pub fn progress_bar(total: u64) -> ProgressBar {
    ProgressBar::new(total)
}

/// Create a spinner for indeterminate progress
pub fn spinner() -> ProgressBar {
    let spinner = ProgressBar::spinner();
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar() {
        let pb = progress_bar(10);
        pb.set_message("Testing");
        pb.inc(5);
        pb.set_position(7);
        pb.finish_with_message("Done");
    }

    #[test]
    fn test_spinner() {
        let sp = spinner();
        sp.set_message("Working...");
        sp.finish("Complete");
    }
}
