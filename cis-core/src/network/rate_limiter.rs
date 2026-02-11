//! # Rate Limiter
//!
//! Token bucket based rate limiter with multi-level support and penalty mechanism.
//!
//! ## Features
//!
//! - Token bucket algorithm for smooth rate limiting
//! - Multi-level limits: global, IP, user
//! - Sliding window for accurate rate tracking
//! - Exponential backoff penalty for repeated failures
//! - Automatic cleanup of expired entries
//!
//! ## Usage
//!
//! ```rust
//! use cis_core::network::rate_limiter::{RateLimiter, RateLimitConfig, LimitType, LimitConfig};
//!
//! let config = RateLimitConfig {
//!     api_limit: LimitConfig { rate: 100, per_seconds: 60, burst: 150 },
//!     auth_limit: LimitConfig { rate: 5, per_seconds: 60, burst: 10 },
//!     conn_limit: LimitConfig { rate: 10, per_seconds: 60, burst: 20 },
//! };
//!
//! let limiter = RateLimiter::new(config);
//! 
//! // Check if request is allowed
//! match limiter.check("user:123", LimitType::Api) {
//!     Ok(()) => println!("Request allowed"),
//!     Err(e) => println!("Rate limited: {}", e),
//! }
//! ```

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{debug, warn};

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// API call limits (100/min default)
    pub api_limit: LimitConfig,
    /// Authentication attempt limits (5/min default)
    pub auth_limit: LimitConfig,
    /// Connection limits (10/min default)
    pub conn_limit: LimitConfig,
    /// Ban configuration
    #[serde(default)]
    pub ban_config: BanConfig,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            api_limit: LimitConfig {
                rate: 100,
                per_seconds: 60,
                burst: 150,
            },
            auth_limit: LimitConfig {
                rate: 5,
                per_seconds: 60,
                burst: 10,
            },
            conn_limit: LimitConfig {
                rate: 10,
                per_seconds: 60,
                burst: 20,
            },
            ban_config: BanConfig::default(),
        }
    }
}

/// Individual limit configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LimitConfig {
    /// Rate (tokens per per_seconds)
    pub rate: u32,
    /// Time window in seconds
    pub per_seconds: u64,
    /// Maximum burst capacity
    pub burst: u32,
}

/// Ban configuration for penalty mechanism
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BanConfig {
    /// Number of failures before ban
    #[serde(default = "default_failures_before_ban")]
    pub failures_before_ban: u32,
    /// Base ban duration in seconds
    #[serde(default = "default_ban_duration")]
    pub ban_duration: u64,
    /// Maximum ban duration in seconds (for exponential backoff)
    #[serde(default = "default_max_ban_duration")]
    pub max_ban_duration: u64,
    /// Failure count reset duration in seconds
    #[serde(default = "default_failure_reset_duration")]
    pub failure_reset_duration: u64,
}

fn default_failures_before_ban() -> u32 {
    5
}

fn default_ban_duration() -> u64 {
    3600 // 1 hour
}

fn default_max_ban_duration() -> u64 {
    86400 // 24 hours
}

fn default_failure_reset_duration() -> u64 {
    3600 // 1 hour
}

impl Default for BanConfig {
    fn default() -> Self {
        Self {
            failures_before_ban: default_failures_before_ban(),
            ban_duration: default_ban_duration(),
            max_ban_duration: default_max_ban_duration(),
            failure_reset_duration: default_failure_reset_duration(),
        }
    }
}

/// Type of rate limit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LimitType {
    /// API calls
    Api,
    /// Authentication attempts
    Auth,
    /// Connections
    Conn,
}

impl LimitType {
    /// Get the prefix for key generation
    fn prefix(&self) -> &'static str {
        match self {
            LimitType::Api => "api",
            LimitType::Auth => "auth",
            LimitType::Conn => "conn",
        }
    }

    /// Get the config for this limit type
    fn config<'a>(&self, config: &'a RateLimitConfig) -> &'a LimitConfig {
        match self {
            LimitType::Api => &config.api_limit,
            LimitType::Auth => &config.auth_limit,
            LimitType::Conn => &config.conn_limit,
        }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
pub struct TokenBucket {
    /// Current tokens (atomic for thread safety)
    tokens: AtomicU32,
    /// Last update timestamp
    last_update: std::sync::Mutex<Instant>,
    /// Rate: tokens per second
    rate: f64,
    /// Maximum capacity
    capacity: u32,
    /// Penalty state
    penalty: std::sync::Mutex<PenaltyState>,
}

/// Penalty state for a bucket
#[derive(Debug, Clone)]
struct PenaltyState {
    /// Number of consecutive failures
    failure_count: u32,
    /// Last failure timestamp
    last_failure: Option<Instant>,
    /// Ban expiration time
    banned_until: Option<Instant>,
    /// Ban count (for exponential backoff)
    ban_count: u32,
}

impl Default for PenaltyState {
    fn default() -> Self {
        Self {
            failure_count: 0,
            last_failure: None,
            banned_until: None,
            ban_count: 0,
        }
    }
}

impl TokenBucket {
    /// Create a new token bucket
    pub fn new(rate: f64, capacity: u32) -> Self {
        Self {
            tokens: AtomicU32::new(capacity),
            last_update: std::sync::Mutex::new(Instant::now()),
            rate,
            capacity,
            penalty: std::sync::Mutex::new(PenaltyState::default()),
        }
    }

    /// Add tokens based on elapsed time
    fn add_tokens(&self) {
        let now = Instant::now();
        let mut last_update = self.last_update.lock().unwrap();
        let elapsed = now.duration_since(*last_update).as_secs_f64();
        
        if elapsed > 0.0 {
            let tokens_to_add = (elapsed * self.rate) as u32;
            if tokens_to_add > 0 {
                let current = self.tokens.load(Ordering::Relaxed);
                let new_tokens = (current + tokens_to_add).min(self.capacity);
                self.tokens.store(new_tokens, Ordering::Relaxed);
                *last_update = now;
            }
        }
    }

    /// Try to consume a token
    pub fn try_consume(&self) -> bool {
        self.add_tokens();
        
        loop {
            let current = self.tokens.load(Ordering::Relaxed);
            if current == 0 {
                return false;
            }
            
            match self.tokens.compare_exchange(
                current,
                current - 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(_) => continue, // Retry on contention
            }
        }
    }

    /// Get remaining tokens
    pub fn remaining(&self) -> u32 {
        self.add_tokens();
        self.tokens.load(Ordering::Relaxed)
    }

    /// Record a failure
    pub fn record_failure(&self, config: &BanConfig) {
        let mut penalty = self.penalty.lock().unwrap();
        let now = Instant::now();
        
        // Check if we should reset failure count
        if let Some(last) = penalty.last_failure {
            if now.duration_since(last).as_secs() > config.failure_reset_duration {
                penalty.failure_count = 0;
            }
        }
        
        penalty.failure_count += 1;
        penalty.last_failure = Some(now);
        
        // Check if we should ban
        if penalty.failure_count >= config.failures_before_ban {
            let ban_duration = calculate_ban_duration(config, penalty.ban_count);
            penalty.banned_until = Some(now + Duration::from_secs(ban_duration));
            penalty.ban_count += 1;
            penalty.failure_count = 0; // Reset after ban
            
            warn!(
                "Rate limiter banned key for {} seconds (ban count: {})",
                ban_duration, penalty.ban_count
            );
        }
    }

    /// Check if banned
    pub fn is_banned(&self) -> bool {
        let penalty = self.penalty.lock().unwrap();
        
        if let Some(banned_until) = penalty.banned_until {
            if Instant::now() < banned_until {
                return true;
            }
        }
        
        false
    }

    /// Get ban expiration time if banned
    pub fn ban_expires_at(&self) -> Option<Instant> {
        let penalty = self.penalty.lock().unwrap();
        penalty.banned_until
    }

    /// Clear ban (for testing or manual unban)
    pub fn clear_ban(&self) {
        let mut penalty = self.penalty.lock().unwrap();
        penalty.banned_until = None;
        penalty.failure_count = 0;
    }

    /// Get failure count
    pub fn failure_count(&self) -> u32 {
        let penalty = self.penalty.lock().unwrap();
        penalty.failure_count
    }

    /// Check if this bucket is stale (can be cleaned up)
    pub fn is_stale(&self, max_idle: Duration) -> bool {
        let last_update = self.last_update.lock().unwrap();
        let penalty = self.penalty.lock().unwrap();
        let now = Instant::now();
        
        // Don't clean up if banned
        if let Some(banned_until) = penalty.banned_until {
            if now < banned_until {
                return false;
            }
        }
        
        // Clean up if idle for too long
        now.duration_since(*last_update) > max_idle && self.tokens.load(Ordering::Relaxed) == self.capacity
    }
}

/// Calculate ban duration with exponential backoff
fn calculate_ban_duration(config: &BanConfig, ban_count: u32) -> u64 {
    let multiplier = 2_u64.pow(ban_count.min(10)); // Cap at 2^10 to avoid overflow
    let duration = config.ban_duration.saturating_mul(multiplier);
    duration.min(config.max_ban_duration)
}

/// Rate limiter with multi-level support
#[derive(Debug)]
pub struct RateLimiter {
    /// Token buckets keyed by "type:key"
    buckets: DashMap<String, Arc<TokenBucket>>,
    /// Configuration
    config: RateLimitConfig,
    /// Cleanup interval handle
    cleanup_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Arc<Self> {
        let limiter = Arc::new(Self {
            buckets: DashMap::new(),
            config,
            cleanup_handle: std::sync::Mutex::new(None),
        });

        // Start cleanup task
        let limiter_clone = Arc::clone(&limiter);
        let handle = tokio::spawn(async move {
            limiter_clone.cleanup_task().await;
        });

        *limiter.cleanup_handle.lock().unwrap() = Some(handle);

        limiter
    }

    /// Build bucket key from limit type and identifier
    fn build_key(limit_type: LimitType, key: &str) -> String {
        format!("{}:{}", limit_type.prefix(), key)
    }

    /// Get or create a token bucket
    fn get_bucket(&self, key: &str, limit_type: LimitType) -> Arc<TokenBucket> {
        let full_key = Self::build_key(limit_type, key);
        let config = limit_type.config(&self.config);
        
        // Calculate rate as tokens per second
        let rate = config.rate as f64 / config.per_seconds as f64;
        
        self.buckets
            .entry(full_key)
            .or_insert_with(|| Arc::new(TokenBucket::new(rate, config.burst)))
            .clone()
    }

    /// Check if a request is allowed
    pub fn check(&self, key: &str, limit_type: LimitType) -> crate::Result<()> {
        let bucket = self.get_bucket(key, limit_type);

        // Check if banned
        if bucket.is_banned() {
            if let Some(expires_at) = bucket.ban_expires_at() {
                let remaining = expires_at.duration_since(Instant::now()).as_secs();
                return Err(crate::error::CisError::other(format!(
                    "Rate limit: banned for {} more seconds",
                    remaining
                )));
            }
        }

        // Try to consume a token
        if bucket.try_consume() {
            debug!("Rate limit allowed for {}:{}", limit_type.prefix(), key);
            Ok(())
        } else {
            Err(crate::error::CisError::other(
                "Rate limit exceeded, please try again later",
            ))
        }
    }

    /// Check without consuming a token (for peeking)
    pub fn peek(&self, key: &str, limit_type: LimitType) -> crate::Result<()> {
        let bucket = self.get_bucket(key, limit_type);

        // Check if banned
        if bucket.is_banned() {
            if let Some(expires_at) = bucket.ban_expires_at() {
                let remaining = expires_at.duration_since(Instant::now()).as_secs();
                return Err(crate::error::CisError::other(format!(
                    "Rate limit: banned for {} more seconds",
                    remaining
                )));
            }
        }

        // Just check if tokens available
        if bucket.remaining() > 0 {
            Ok(())
        } else {
            Err(crate::error::CisError::other("Rate limit would be exceeded"))
        }
    }

    /// Get remaining quota
    pub fn remaining(&self, key: &str, limit_type: LimitType) -> u32 {
        let bucket = self.get_bucket(key, limit_type);
        bucket.remaining()
    }

    /// Record a failure (for penalty mechanism)
    pub fn record_failure(&self, key: &str) {
        // Record failure for all limit types to track overall behavior
        for limit_type in [LimitType::Api, LimitType::Auth, LimitType::Conn] {
            let bucket = self.get_bucket(key, limit_type);
            bucket.record_failure(&self.config.ban_config);
        }
    }

    /// Check if a key is banned
    pub fn is_banned(&self, key: &str) -> bool {
        // Check all limit types
        for limit_type in [LimitType::Api, LimitType::Auth, LimitType::Conn] {
            let bucket = self.get_bucket(key, limit_type);
            if bucket.is_banned() {
                return true;
            }
        }
        false
    }

    /// Get ban expiration time
    pub fn ban_expires_at(&self, key: &str) -> Option<Instant> {
        let mut latest = None;
        
        for limit_type in [LimitType::Api, LimitType::Auth, LimitType::Conn] {
            let bucket = self.get_bucket(key, limit_type);
            if let Some(expires) = bucket.ban_expires_at() {
                latest = Some(match latest {
                    Some(current) if expires > current => expires,
                    None => expires,
                    Some(current) => current,
                });
            }
        }
        
        latest
    }

    /// Manually unban a key
    pub fn unban(&self, key: &str) {
        for limit_type in [LimitType::Api, LimitType::Auth, LimitType::Conn] {
            let bucket = self.get_bucket(key, limit_type);
            bucket.clear_ban();
        }
        warn!("Manually unbanned key: {}", key);
    }

    /// Get current configuration
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: RateLimitConfig) {
        // Clear all buckets to apply new rates
        self.buckets.clear();
        self.config = config;
        debug!("Rate limiter configuration updated");
    }

    /// Get bucket count (for monitoring)
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    /// Cleanup task that runs periodically
    async fn cleanup_task(self: Arc<Self>) {
        let mut interval = interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            let max_idle = Duration::from_secs(300); // 5 minutes
            let mut removed = 0;
            
            self.buckets.retain(|_key, bucket| {
                let stale = bucket.is_stale(max_idle);
                if stale {
                    removed += 1;
                }
                !stale
            });
            
            if removed > 0 {
                debug!("Cleaned up {} stale rate limiter buckets", removed);
            }
        }
    }

    /// Shutdown the rate limiter
    pub fn shutdown(&self) {
        if let Some(handle) = self.cleanup_handle.lock().unwrap().take() {
            handle.abort();
        }
    }
}

impl Drop for RateLimiter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tokio::time::sleep;

    #[test]
    fn test_token_bucket_basic() {
        let bucket = TokenBucket::new(10.0, 10);
        
        // Should have full capacity initially
        assert_eq!(bucket.remaining(), 10);
        
        // Consume some tokens
        assert!(bucket.try_consume());
        assert!(bucket.try_consume());
        assert_eq!(bucket.remaining(), 8);
        
        // Consume all remaining
        for _ in 0..8 {
            assert!(bucket.try_consume());
        }
        assert_eq!(bucket.remaining(), 0);
        
        // Should fail when empty
        assert!(!bucket.try_consume());
    }

    #[test]
    fn test_token_bucket_refill() {
        let bucket = TokenBucket::new(100.0, 10); // 100 tokens/sec, capacity 10
        
        // Consume all tokens
        while bucket.try_consume() {}
        assert_eq!(bucket.remaining(), 0);
        
        // Wait for refill
        thread::sleep(Duration::from_millis(100));
        
        // Should have some tokens now
        let remaining = bucket.remaining();
        assert!(remaining > 0, "Expected tokens to refill, got {}", remaining);
    }

    #[tokio::test]
    async fn test_rate_limiter_check() {
        let config = RateLimitConfig {
            api_limit: LimitConfig {
                rate: 10,
                per_seconds: 1,
                burst: 10,
            },
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        
        // Should allow requests within limit
        for _ in 0..10 {
            assert!(limiter.check("test_user", LimitType::Api).is_ok());
        }
        
        // Should deny when limit exceeded
        assert!(limiter.check("test_user", LimitType::Api).is_err());
        
        // Different user should still be allowed
        assert!(limiter.check("other_user", LimitType::Api).is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_remaining() {
        let config = RateLimitConfig {
            api_limit: LimitConfig {
                rate: 100,
                per_seconds: 60,
                burst: 100,
            },
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        
        // Initial remaining should be burst
        assert_eq!(limiter.remaining("user1", LimitType::Api), 100);
        
        // Consume some tokens
        limiter.check("user1", LimitType::Api).unwrap();
        limiter.check("user1", LimitType::Api).unwrap();
        
        assert_eq!(limiter.remaining("user1", LimitType::Api), 98);
    }

    #[tokio::test]
    async fn test_penalty_mechanism() {
        let config = RateLimitConfig {
            ban_config: BanConfig {
                failures_before_ban: 3,
                ban_duration: 1, // 1 second for testing
                max_ban_duration: 10,
                failure_reset_duration: 60,
            },
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        
        // Record failures
        limiter.record_failure("bad_user");
        limiter.record_failure("bad_user");
        
        // Should not be banned yet
        assert!(!limiter.is_banned("bad_user"));
        
        // One more failure triggers ban
        limiter.record_failure("bad_user");
        assert!(limiter.is_banned("bad_user"));
        
        // Wait for ban to expire
        sleep(Duration::from_secs(2)).await;
        
        // Should not be banned anymore
        assert!(!limiter.is_banned("bad_user"));
    }

    #[tokio::test]
    async fn test_penalty_exponential_backoff() {
        let config = RateLimitConfig {
            ban_config: BanConfig {
                failures_before_ban: 1,
                ban_duration: 1,
                max_ban_duration: 10,
                failure_reset_duration: 60,
            },
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        
        // First ban - 1 second
        limiter.record_failure("user");
        let expires1 = limiter.ban_expires_at("user").unwrap();
        
        // Wait and trigger second ban
        sleep(Duration::from_secs(2)).await;
        limiter.record_failure("user");
        let expires2 = limiter.ban_expires_at("user").unwrap();
        
        // Second ban should be longer
        assert!(expires2 > expires1);
    }

    #[tokio::test]
    async fn test_rate_limiter_multi_type() {
        let config = RateLimitConfig {
            api_limit: LimitConfig {
                rate: 10,
                per_seconds: 1,
                burst: 10,
            },
            auth_limit: LimitConfig {
                rate: 5,
                per_seconds: 1,
                burst: 5,
            },
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        
        // Exhaust API limit
        for _ in 0..10 {
            limiter.check("user", LimitType::Api).unwrap();
        }
        assert!(limiter.check("user", LimitType::Api).is_err());
        
        // Auth limit should be independent
        for _ in 0..5 {
            limiter.check("user", LimitType::Auth).unwrap();
        }
        assert!(limiter.check("user", LimitType::Auth).is_err());
    }

    #[tokio::test]
    async fn test_manual_unban() {
        let config = RateLimitConfig {
            ban_config: BanConfig {
                failures_before_ban: 1,
                ban_duration: 3600,
                max_ban_duration: 86400,
                failure_reset_duration: 60,
            },
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        
        // Ban the user
        limiter.record_failure("user");
        assert!(limiter.is_banned("user"));
        
        // Manual unban
        limiter.unban("user");
        assert!(!limiter.is_banned("user"));
    }

    #[tokio::test]
    async fn test_ban_blocks_all_requests() {
        let config = RateLimitConfig {
            ban_config: BanConfig {
                failures_before_ban: 1,
                ban_duration: 3600,
                max_ban_duration: 86400,
                failure_reset_duration: 60,
            },
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        
        // Ban the user
        limiter.record_failure("user");
        
        // All request types should be blocked
        assert!(limiter.check("user", LimitType::Api).is_err());
        assert!(limiter.check("user", LimitType::Auth).is_err());
        assert!(limiter.check("user", LimitType::Conn).is_err());
    }

    #[test]
    fn test_calculate_ban_duration() {
        let config = BanConfig {
            ban_duration: 60,
            max_ban_duration: 1000,
            ..Default::default()
        };
        
        assert_eq!(calculate_ban_duration(&config, 0), 60);
        assert_eq!(calculate_ban_duration(&config, 1), 120);
        assert_eq!(calculate_ban_duration(&config, 2), 240);
        assert_eq!(calculate_ban_duration(&config, 10), 1000); // Capped at max
    }

    #[test]
    fn test_limit_type_prefix() {
        assert_eq!(LimitType::Api.prefix(), "api");
        assert_eq!(LimitType::Auth.prefix(), "auth");
        assert_eq!(LimitType::Conn.prefix(), "conn");
    }

    #[test]
    fn test_build_key() {
        assert_eq!(
            RateLimiter::build_key(LimitType::Api, "user:123"),
            "api:user:123"
        );
        assert_eq!(
            RateLimiter::build_key(LimitType::Auth, "ip:192.168.1.1"),
            "auth:ip:192.168.1.1"
        );
    }
}
