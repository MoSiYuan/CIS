//! # Session Monitor
//!
//! Advanced blockage detection and recovery for Agent sessions.
//! Uses multiple detection strategies: keyword matching, inactivity timeout, and pattern analysis.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};


use regex::Regex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::agent::cluster::{
    events::SessionState,
    manager::SessionManager,
    SessionId,
};
use crate::error::Result;

/// Default monitoring configuration
pub const DEFAULT_CHECK_INTERVAL_MS: u64 = 500;
pub const DEFAULT_INACTIVITY_TIMEOUT_SECS: u64 = 300; // 5 minutes
pub const DEFAULT_MAX_RUNTIME_SECS: u64 = 3600; // 1 hour

/// Blockage detection strategy
#[derive(Debug, Clone)]
pub enum DetectionStrategy {
    /// Keyword matching in output
    Keyword {
        keywords: Vec<String>,
        /// Number of consecutive matches required
        threshold: usize,
    },
    /// Inactivity timeout
    Inactivity {
        timeout_secs: u64,
    },
    /// Maximum runtime
    MaxRuntime {
        max_secs: u64,
    },
    /// Custom pattern (regex)
    Pattern {
        pattern: String,
        regex: Regex,
    },
}

/// Blockage detection result
#[derive(Debug, Clone)]
pub enum BlockageResult {
    /// No blockage detected
    Clear,
    /// Blockage detected with reason
    Blocked { reason: String, strategy: String },
    /// Timeout
    Timeout { reason: String },
}

/// Monitor configuration
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Check interval
    pub check_interval: Duration,
    /// Detection strategies
    pub strategies: Vec<DetectionStrategy>,
    /// Auto-recovery enabled
    pub auto_recovery: bool,
    /// Recovery prompt (sent to agent when unblocked)
    pub recovery_prompt: Option<String>,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_millis(DEFAULT_CHECK_INTERVAL_MS),
            strategies: vec![
                DetectionStrategy::default_keywords(),
                DetectionStrategy::inactivity(DEFAULT_INACTIVITY_TIMEOUT_SECS),
                DetectionStrategy::max_runtime(DEFAULT_MAX_RUNTIME_SECS),
            ],
            auto_recovery: false,
            recovery_prompt: Some("\n".to_string()),
        }
    }
}

impl DetectionStrategy {
    /// Default keyword-based detection
    pub fn default_keywords() -> Self {
        let keywords = vec![
            // Confirmation prompts
            "?".to_string(),
            "confirm".to_string(),
            "yes/no".to_string(),
            "y/n".to_string(),
            // Wait prompts
            "enter to continue".to_string(),
            "press any key".to_string(),
            "press any key to continue".to_string(),
            // Auth prompts
            "authentication required".to_string(),
            "password:".to_string(),
            "username:".to_string(),
            // Git conflicts
            "merge conflict".to_string(),
            "rebase conflict".to_string(),
            "conflict:".to_string(),
            // Error states
            "error:".to_string(),
            "fatal:".to_string(),
            // Tool prompts
            "waiting for input".to_string(),
            "input required".to_string(),
            // Shell prompts that might indicate completion
            "$ ".to_string(),
            "> ".to_string(),
        ];
        
        DetectionStrategy::Keyword {
            keywords,
            threshold: 1,
        }
    }
    
    /// Inactivity-based detection
    pub fn inactivity(timeout_secs: u64) -> Self {
        DetectionStrategy::Inactivity { timeout_secs }
    }
    
    /// Max runtime detection
    pub fn max_runtime(max_secs: u64) -> Self {
        DetectionStrategy::MaxRuntime { max_secs }
    }
    
    /// Pattern-based detection
    pub fn pattern(pattern: &str) -> Result<Self> {
        let regex = Regex::new(pattern)
            .map_err(|e| crate::error::CisError::invalid_input(format!("Invalid regex: {}", e)))?;
        
        Ok(DetectionStrategy::Pattern {
            pattern: pattern.to_string(),
            regex,
        })
    }
}

/// Session monitor for a single session
#[derive(Debug)]
pub struct SessionMonitor {
    /// Session ID
    session_id: SessionId,
    /// Monitor configuration
    config: MonitorConfig,
    /// Last check time
    last_check: Arc<RwLock<Instant>>,
    /// Last activity time
    last_activity: Arc<RwLock<Instant>>,
    /// Start time
    started_at: Instant,
    /// Keyword match counts
    keyword_matches: Arc<RwLock<HashMap<String, usize>>>,
    /// Current blockage state
    blocked_state: Arc<RwLock<Option<BlockageResult>>>,
    /// Monitor task handle
    task_handle: Option<JoinHandle<()>>,
}

impl SessionMonitor {
    /// Create new session monitor
    pub fn new(session_id: SessionId, config: MonitorConfig) -> Self {
        let now = Instant::now();
        Self {
            session_id,
            config,
            last_check: Arc::new(RwLock::new(now)),
            last_activity: Arc::new(RwLock::new(now)),
            started_at: now,
            keyword_matches: Arc::new(RwLock::new(HashMap::new())),
            blocked_state: Arc::new(RwLock::new(None)),
            task_handle: None,
        }
    }
    
    /// Start monitoring
    pub fn start(&mut self, manager: &'static SessionManager) {
        let session_id = self.session_id.clone();
        let config = self.config.clone();
        let last_check = self.last_check.clone();
        let last_activity = self.last_activity.clone();
        let started_at = self.started_at;
        let keyword_matches = self.keyword_matches.clone();
        let blocked_state = self.blocked_state.clone();
        
        let handle = tokio::spawn(async move {
            info!("Monitor started for session {}", session_id.short());
            
            let mut interval = tokio::time::interval(config.check_interval);
            
            loop {
                interval.tick().await;
                
                // Check if session still exists
                match manager.get_state(&session_id).await {
                    Ok(SessionState::Completed { .. }) | 
                    Ok(SessionState::Failed { .. }) => {
                        debug!("Session {} terminated, stopping monitor", session_id.short());
                        break;
                    }
                    Err(_) => {
                        warn!("Session {} not found, stopping monitor", session_id.short());
                        break;
                    }
                    _ => {}
                }
                
                // Get session output
                let output = match manager.get_output(&session_id).await {
                    Ok(out) => out,
                    Err(e) => {
                        debug!("Failed to get output for {}: {}", session_id.short(), e);
                        continue;
                    }
                };
                
                // Run detection strategies
                let result = Self::check_blockage(
                    &config.strategies,
                    &output,
                    &last_activity,
                    started_at,
                    &keyword_matches,
                ).await;
                
                // Update state
                match &result {
                    BlockageResult::Blocked { reason, .. } => {
                        let mut state = blocked_state.write().await;
                        if state.is_none() {
                            warn!("Blockage detected in {}: {}", session_id.short(), reason);
                            
                            // Mark session as blocked
                            let _ = manager.mark_blocked(&session_id, reason).await;
                            
                            *state = Some(result.clone());
                        }
                    }
                    BlockageResult::Timeout { reason } => {
                        warn!("Timeout detected in {}: {}", session_id.short(), reason);
                        let _ = manager.kill_session(&session_id, reason).await;
                        break;
                    }
                    BlockageResult::Clear => {
                        let mut state = blocked_state.write().await;
                        if state.is_some() {
                            info!("Session {} recovered from blockage", session_id.short());
                            
                            // Auto-recovery if enabled
                            if config.auto_recovery {
                                if let Some(ref prompt) = config.recovery_prompt {
                                    let _ = manager.send_input(&session_id, prompt.as_bytes()).await;
                                }
                                let _ = manager.mark_recovered(&session_id).await;
                            }
                            
                            *state = None;
                        }
                    }
                }
                
                // Update last check
                *last_check.write().await = Instant::now();
            }
            
            info!("Monitor stopped for session {}", session_id.short());
        });
        
        self.task_handle = Some(handle);
    }
    
    /// Check for blockage using configured strategies
    async fn check_blockage(
        strategies: &[DetectionStrategy],
        output: &str,
        last_activity: &Arc<RwLock<Instant>>,
        started_at: Instant,
        keyword_matches: &Arc<RwLock<HashMap<String, usize>>>,
    ) -> BlockageResult {
        for strategy in strategies {
            match strategy {
                DetectionStrategy::Keyword { keywords, threshold } => {
                    // Check last few lines
                    let recent_lines: Vec<&str> = output.lines().rev().take(10).collect();
                    let recent = recent_lines.join("\n").to_lowercase();
                    
                    for keyword in keywords {
                        if recent.contains(&keyword.to_lowercase()) {
                            let mut matches = keyword_matches.write().await;
                            let count = matches.entry(keyword.clone()).or_insert(0);
                            *count += 1;
                            
                            if *count >= *threshold {
                                return BlockageResult::Blocked {
                                    reason: format!("Keyword '{}' detected", keyword),
                                    strategy: "keyword".to_string(),
                                };
                            }
                        }
                    }
                }
                
                DetectionStrategy::Inactivity { timeout_secs } => {
                    let last = *last_activity.read().await;
                    let elapsed = last.elapsed().as_secs();
                    
                    if elapsed >= *timeout_secs {
                        return BlockageResult::Blocked {
                            reason: format!("Inactivity for {} seconds", elapsed),
                            strategy: "inactivity".to_string(),
                        };
                    }
                }
                
                DetectionStrategy::MaxRuntime { max_secs } => {
                    let elapsed = started_at.elapsed().as_secs();
                    
                    if elapsed >= *max_secs {
                        return BlockageResult::Timeout {
                            reason: format!("Max runtime exceeded ({}s)", elapsed),
                        };
                    }
                }
                
                DetectionStrategy::Pattern { pattern, regex } => {
                    if regex.is_match(output) {
                        return BlockageResult::Blocked {
                            reason: format!("Pattern '{}' matched", pattern),
                            strategy: "pattern".to_string(),
                        };
                    }
                }
            }
        }
        
        BlockageResult::Clear
    }
    
    /// Update activity timestamp (called when new output is received)
    pub async fn touch_activity(&self) {
        *self.last_activity.write().await = Instant::now();
    }
    
    /// Get current blockage state
    pub async fn get_state(&self) -> Option<BlockageResult> {
        self.blocked_state.read().await.clone()
    }
    
    /// Stop monitoring
    pub async fn stop(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
            info!("Monitor for {} stopped manually", self.session_id.short());
        }
    }
}

impl Drop for SessionMonitor {
    fn drop(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
    }
}

/// Global monitor coordinator
pub struct MonitorCoordinator {
    /// Active monitors
    monitors: Arc<RwLock<HashMap<SessionId, SessionMonitor>>>,
    /// Default config
    default_config: MonitorConfig,
}

impl MonitorCoordinator {
    /// Create new coordinator
    pub fn new() -> Self {
        Self {
            monitors: Arc::new(RwLock::new(HashMap::new())),
            default_config: MonitorConfig::default(),
        }
    }
    
    /// Create with custom config
    pub fn with_config(config: MonitorConfig) -> Self {
        Self {
            monitors: Arc::new(RwLock::new(HashMap::new())),
            default_config: config,
        }
    }
    
    /// Start monitoring a session
    pub async fn start_monitoring(
        &self,
        session_id: SessionId,
        manager: &'static SessionManager,
    ) -> Result<()> {
        let mut monitor = SessionMonitor::new(session_id.clone(), self.default_config.clone());
        monitor.start(manager);
        
        let mut monitors = self.monitors.write().await;
        monitors.insert(session_id, monitor);
        
        Ok(())
    }
    
    /// Start monitoring with custom config
    pub async fn start_monitoring_with_config(
        &self,
        session_id: SessionId,
        manager: &'static SessionManager,
        config: MonitorConfig,
    ) -> Result<()> {
        let mut monitor = SessionMonitor::new(session_id.clone(), config);
        monitor.start(manager);
        
        let mut monitors = self.monitors.write().await;
        monitors.insert(session_id, monitor);
        
        Ok(())
    }
    
    /// Stop monitoring a session
    pub async fn stop_monitoring(&self, session_id: &SessionId) {
        let mut monitors = self.monitors.write().await;
        if let Some(mut monitor) = monitors.remove(session_id) {
            monitor.stop().await;
        }
    }
    
    /// Get monitor state for a session
    pub async fn get_state(&self, session_id: &SessionId) -> Option<BlockageResult> {
        let monitors = self.monitors.read().await;
        if let Some(monitor) = monitors.get(session_id) {
            monitor.get_state().await
        } else {
            None
        }
    }
    
    /// Update activity for a session (called when output is received)
    pub async fn touch_activity(&self, session_id: &SessionId) {
        let monitors = self.monitors.read().await;
        if let Some(monitor) = monitors.get(session_id) {
            monitor.touch_activity().await;
        }
    }
    
    /// Stop all monitors
    pub async fn stop_all(&self) {
        let mut monitors = self.monitors.write().await;
        for (_, mut monitor) in monitors.drain() {
            monitor.stop().await;
        }
    }
    
    /// Get active monitor count
    pub async fn active_count(&self) -> usize {
        self.monitors.read().await.len()
    }
}

impl Default for MonitorCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_strategy_default() {
        let config = MonitorConfig::default();
        assert_eq!(config.strategies.len(), 3);
    }

    #[tokio::test]
    async fn test_session_monitor_creation() {
        let session_id = SessionId::new("run-1", "task-1");
        let monitor = SessionMonitor::new(session_id, MonitorConfig::default());
        
        assert!(monitor.get_state().await.is_none());
    }

    #[test]
    fn test_blockage_result_display() {
        let result = BlockageResult::Blocked {
            reason: "Test".to_string(),
            strategy: "keyword".to_string(),
        };
        
        match result {
            BlockageResult::Blocked { reason, strategy } => {
                assert_eq!(reason, "Test");
                assert_eq!(strategy, "keyword");
            }
            _ => panic!("Expected Blocked variant"),
        }
    }
}
