//! # ACL Rules Engine
//!
//! Advanced access control with complex conditions:
//! - IP address matching (CIDR, ranges)
//! - Time window restrictions
//! - Rate limiting
//! - Geolocation (optional)
//!
//! ## Rule Evaluation
//!
//! Rules are evaluated in order (priority), first match wins.
//! If no rule matches, default action from NetworkMode applies.

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// ACL Action for rules
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AclAction {
    /// Allow connection
    Allow,
    /// Deny connection
    Deny,
    /// Allow but quarantine (restricted forwarding)
    Quarantine,
}

impl std::fmt::Display for AclAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AclAction::Allow => write!(f, "allow"),
            AclAction::Deny => write!(f, "deny"),
            AclAction::Quarantine => write!(f, "quarantine"),
        }
    }
}

/// Condition types for ACL rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// Match specific IP address or CIDR
    IpMatch {
        /// IP address or CIDR (e.g., "192.168.1.1" or "10.0.0.0/8")
        cidr: String,
    },
    /// Match IP range
    IpRange {
        /// Start IP (inclusive)
        start: String,
        /// End IP (inclusive)
        end: String,
    },
    /// Time window restriction
    TimeWindow {
        /// Start time in HH:MM format
        start: String,
        /// End time in HH:MM format
        end: String,
        /// Days of week (0=Sunday, 6=Saturday), empty means all days
        #[serde(default)]
        days: Vec<u8>,
        /// Timezone (default: UTC)
        #[serde(default)]
        timezone: String,
    },
    /// Connection rate limit
    RateLimit {
        /// Maximum connections per window
        max_requests: u32,
        /// Window size in seconds
        window_secs: u64,
    },
    /// Match specific DID pattern
    DidPattern {
        /// Pattern to match (supports * wildcard)
        pattern: String,
    },
    /// Match node capability
    Capability {
        /// Required capability
        required: String,
    },
}

/// Complex ACL Rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRule {
    /// Rule ID (unique)
    pub id: String,
    /// Rule name/description
    pub name: String,
    /// Target DID (optional, if None applies to all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did: Option<String>,
    /// Action to take when rule matches
    pub action: AclAction,
    /// Conditions that must all match (AND logic)
    pub conditions: Vec<Condition>,
    /// Rule priority (lower = higher priority, evaluated first)
    pub priority: i32,
    /// Whether rule is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional expiration timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    /// When rule was created
    #[serde(default = "now")]
    pub created_at: i64,
    /// Who created this rule
    pub created_by: String,
}

fn default_true() -> bool {
    true
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

impl AclRule {
    /// Create a new ACL rule
    pub fn new(id: impl Into<String>, name: impl Into<String>, created_by: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            did: None,
            action: AclAction::Deny,
            conditions: Vec::new(),
            priority: 100,
            enabled: true,
            expires_at: None,
            created_at: now(),
            created_by: created_by.into(),
        }
    }

    /// Set target DID
    pub fn with_did(mut self, did: impl Into<String>) -> Self {
        self.did = Some(did.into());
        self
    }

    /// Set action
    pub fn with_action(mut self, action: AclAction) -> Self {
        self.action = action;
        self
    }

    /// Add a condition
    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set expiration
    pub fn with_expiration(mut self, expires_at: i64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Check if rule is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| now() > exp).unwrap_or(false)
    }

    /// Evaluate rule against context
    pub fn evaluate(&self, ctx: &RuleContext) -> Option<AclAction> {
        // Check if rule is enabled and not expired
        if !self.enabled || self.is_expired() {
            return None;
        }

        // Check DID match if specified
        if let Some(ref rule_did) = self.did {
            if let Some(ref ctx_did) = ctx.did {
                if ! Self::match_did(rule_did, ctx_did) {
                    return None;
                }
            } else {
                return None;
            }
        }

        // Evaluate all conditions (AND logic)
        for condition in &self.conditions {
            if ! Self::evaluate_condition(condition, ctx) {
                return None;
            }
        }

        // All conditions matched
        Some(self.action)
    }

    /// Match DID against pattern (supports * wildcard)
    pub(crate) fn match_did(pattern: &str, did: &str) -> bool {
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                // Simple prefix/suffix matching
                if pattern.starts_with('*') {
                    did.ends_with(parts[1])
                } else if pattern.ends_with('*') {
                    did.starts_with(parts[0])
                } else {
                    // Contains pattern
                    did.starts_with(parts[0]) && did.ends_with(parts[1])
                }
            } else {
                // Multiple wildcards - convert to regex would be better
                // For now, simple equality
                pattern == did
            }
        } else {
            pattern == did
        }
    }

    /// Evaluate a single condition
    fn evaluate_condition(condition: &Condition, ctx: &RuleContext) -> bool {
        match condition {
            Condition::IpMatch { cidr } => {
                ctx.ip_addr.map_or(false, |ip| {
                    Self::ip_in_cidr(&ip, cidr)
                })
            }
            Condition::IpRange { start, end } => {
                ctx.ip_addr.map_or(false, |ip| {
                    Self::ip_in_range(&ip, start, end)
                })
            }
            Condition::TimeWindow { start, end, days, timezone } => {
                Self::check_time_window(start, end, days, timezone, ctx.timestamp)
            }
            Condition::RateLimit { max_requests: _, window_secs: _ } => {
                // Rate limiting is handled externally (stateful)
                // Here we just check if context indicates rate limit exceeded
                ctx.rate_limit_exceeded.unwrap_or(false)
            }
            Condition::DidPattern { pattern } => {
                ctx.did.as_ref().map_or(false, |did| {
                    Self::match_did(pattern, did)
                })
            }
            Condition::Capability { required } => {
                ctx.capabilities.contains(required)
            }
        }
    }

    /// Check if IP is in CIDR
    fn ip_in_cidr(ip: &IpAddr, cidr: &str) -> bool {
        match cidr.parse::<ipnetwork::IpNetwork>() {
            Ok(network) => network.contains(*ip),
            Err(_) => false,
        }
    }

    /// Check if IP is in range
    fn ip_in_range(ip: &IpAddr, start: &str, end: &str) -> bool {
        match (start.parse::<IpAddr>(), end.parse::<IpAddr>(), ip) {
            (Ok(s), Ok(e), IpAddr::V4(ip_v4)) => {
                if let (IpAddr::V4(s_v4), IpAddr::V4(e_v4)) = (s, e) {
                    let ip_u32 = u32::from(*ip_v4);
                    let start_u32 = u32::from(s_v4);
                    let end_u32 = u32::from(e_v4);
                    ip_u32 >= start_u32 && ip_u32 <= end_u32
                } else {
                    false
                }
            }
            (Ok(s), Ok(e), IpAddr::V6(ip_v6)) => {
                if let (IpAddr::V6(s_v6), IpAddr::V6(e_v6)) = (s, e) {
                    let ip_u128 = u128::from(*ip_v6);
                    let start_u128 = u128::from(s_v6);
                    let end_u128 = u128::from(e_v6);
                    ip_u128 >= start_u128 && ip_u128 <= end_u128
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check time window
    fn check_time_window(
        start: &str,
        end: &str,
        days: &[u8],
        _timezone: &str,
        timestamp: i64,
    ) -> bool {
        // Parse times
        let start_parts: Vec<&str> = start.split(':').collect();
        let end_parts: Vec<&str> = end.split(':').collect();
        
        if start_parts.len() != 2 || end_parts.len() != 2 {
            return false;
        }

        let start_hour: u32 = start_parts[0].parse().unwrap_or(0);
        let start_min: u32 = start_parts[1].parse().unwrap_or(0);
        let end_hour: u32 = end_parts[0].parse().unwrap_or(23);
        let end_min: u32 = end_parts[1].parse().unwrap_or(59);

        // Convert timestamp to chrono::DateTime
        let dt = chrono::DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| chrono::Utc::now());

        // Check day of week
        if !days.is_empty() {
            let day_of_week = dt.weekday().num_days_from_sunday() as u8;
            if !days.contains(&day_of_week) {
                return false;
            }
        }

        // Check time
        let current_hour = dt.hour();
        let current_min = dt.minute();

        let start_minutes = start_hour * 60 + start_min;
        let end_minutes = end_hour * 60 + end_min;
        let current_minutes = current_hour * 60 + current_min;

        current_minutes >= start_minutes && current_minutes <= end_minutes
    }
}

/// Context for rule evaluation
#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    /// Client IP address
    pub ip_addr: Option<IpAddr>,
    /// Client DID
    pub did: Option<String>,
    /// Current timestamp
    pub timestamp: i64,
    /// Connection count in current window
    pub connection_count: Option<u32>,
    /// Rate limit exceeded flag
    pub rate_limit_exceeded: Option<bool>,
    /// Node capabilities
    pub capabilities: Vec<String>,
}

impl RuleContext {
    /// Create new context
    pub fn new() -> Self {
        Self {
            ip_addr: None,
            did: None,
            timestamp: now(),
            connection_count: None,
            rate_limit_exceeded: None,
            capabilities: Vec::new(),
        }
    }

    /// Set IP address
    pub fn with_ip(mut self, ip: IpAddr) -> Self {
        self.ip_addr = Some(ip);
        self
    }

    /// Set DID
    pub fn with_did(mut self, did: impl Into<String>) -> Self {
        self.did = Some(did.into());
        self
    }

    /// Set timestamp
    pub fn with_timestamp(mut self, ts: i64) -> Self {
        self.timestamp = ts;
        self
    }

    /// Set rate limit status
    pub fn with_rate_limit_exceeded(mut self, exceeded: bool) -> Self {
        self.rate_limit_exceeded = Some(exceeded);
        self
    }

    /// Add capability
    pub fn with_capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.push(cap.into());
        self
    }
}

/// Rules engine that manages and evaluates ACL rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclRulesEngine {
    /// List of rules (sorted by priority)
    pub rules: Vec<AclRule>,
    /// Default action if no rule matches
    pub default_action: AclAction,
    /// Engine version
    pub version: u64,
    /// Last updated
    #[serde(default = "now")]
    pub updated_at: i64,
}

impl Default for AclRulesEngine {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            default_action: AclAction::Deny,
            version: 1,
            updated_at: now(),
        }
    }
}

impl AclRulesEngine {
    /// Create new rules engine
    pub fn new() -> Self {
        Self::default()
    }

    /// Set default action
    pub fn with_default_action(mut self, action: AclAction) -> Self {
        self.default_action = action;
        self
    }

    /// Add a rule
    pub fn add_rule(&mut self, rule: AclRule) {
        self.rules.push(rule);
        self.sort_rules();
        self.bump_version();
    }

    /// Remove a rule by ID
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != rule_id);
        if self.rules.len() < before {
            self.bump_version();
            true
        } else {
            false
        }
    }

    /// Find rule by ID
    pub fn find_rule(&self, rule_id: &str) -> Option<&AclRule> {
        self.rules.iter().find(|r| r.id == rule_id)
    }

    /// Find rule by ID (mutable)
    pub fn find_rule_mut(&mut self, rule_id: &str) -> Option<&mut AclRule> {
        self.rules.iter_mut().find(|r| r.id == rule_id)
    }

    /// Enable/disable rule
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) -> bool {
        if let Some(rule) = self.find_rule_mut(rule_id) {
            rule.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Sort rules by priority
    fn sort_rules(&mut self) {
        self.rules.sort_by_key(|r| r.priority);
    }

    /// Bump version
    fn bump_version(&mut self) {
        self.version += 1;
        self.updated_at = now();
    }

    /// Evaluate rules against context
    pub fn evaluate(&self, ctx: &RuleContext) -> AclAction {
        for rule in &self.rules {
            if let Some(action) = rule.evaluate(ctx) {
                return action;
            }
        }
        self.default_action
    }

    /// Clean up expired rules
    pub fn cleanup_expired(&mut self) {
        let before = self.rules.len();
        self.rules.retain(|r| !r.is_expired());
        if self.rules.len() < before {
            self.bump_version();
        }
    }

    /// Get all active rules
    pub fn active_rules(&self) -> Vec<&AclRule> {
        self.rules.iter().filter(|r| r.enabled && !r.is_expired()).collect()
    }

    /// Get rules summary
    pub fn summary(&self) -> RulesSummary {
        RulesSummary {
            total_rules: self.rules.len(),
            enabled_rules: self.rules.iter().filter(|r| r.enabled).count(),
            expired_rules: self.rules.iter().filter(|r| r.is_expired()).count(),
            version: self.version,
            updated_at: self.updated_at,
        }
    }
}

/// Rules summary
#[derive(Debug, Clone)]
pub struct RulesSummary {
    pub total_rules: usize,
    pub enabled_rules: usize,
    pub expired_rules: usize,
    pub version: u64,
    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_rule_creation() {
        let rule = AclRule::new("rule-1", "Test Rule", "did:local:admin")
            .with_did("did:cis:test:*")
            .with_action(AclAction::Allow)
            .with_priority(10);

        assert_eq!(rule.id, "rule-1");
        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.action, AclAction::Allow);
        assert_eq!(rule.priority, 10);
        assert!(rule.enabled);
    }

    #[test]
    fn test_did_pattern_matching() {
        assert!(AclRule::match_did("did:cis:test:*", "did:cis:test:abc123"));
        assert!(AclRule::match_did("did:cis:test:*", "did:cis:test:xyz"));
        assert!(!AclRule::match_did("did:cis:test:*", "did:cis:other:abc123"));
        assert!(!AclRule::match_did("did:cis:test:*", "did:cis:test"));

        assert!(AclRule::match_did("*@example.com", "user@example.com"));
        assert!(AclRule::match_did("*@example.com", "admin@example.com"));
        assert!(!AclRule::match_did("*@example.com", "user@other.com"));

        assert!(AclRule::match_did("did:cis:exact", "did:cis:exact"));
        assert!(!AclRule::match_did("did:cis:exact", "did:cis:other"));
    }

    #[test]
    fn test_rule_evaluation_basic() {
        let rule = AclRule::new("rule-1", "Allow Test", "admin")
            .with_did("did:cis:test:*")
            .with_action(AclAction::Allow);

        let ctx_match = RuleContext::new()
            .with_did("did:cis:test:abc123");

        let ctx_no_match = RuleContext::new()
            .with_did("did:cis:other:abc123");

        assert_eq!(rule.evaluate(&ctx_match), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_no_match), None);
    }

    #[test]
    fn test_rule_expiration() {
        let mut rule = AclRule::new("rule-1", "Expired Rule", "admin")
            .with_action(AclAction::Allow);

        // Not expired
        rule.expires_at = Some(now() + 3600);
        let ctx = RuleContext::new();
        assert_eq!(rule.evaluate(&ctx), Some(AclAction::Allow));

        // Expired
        rule.expires_at = Some(1); // Way in the past
        assert_eq!(rule.evaluate(&ctx), None);
    }

    #[test]
    fn test_rule_disabled() {
        let mut rule = AclRule::new("rule-1", "Disabled Rule", "admin")
            .with_action(AclAction::Allow);

        rule.enabled = false;
        let ctx = RuleContext::new();
        assert_eq!(rule.evaluate(&ctx), None);
    }

    #[test]
    fn test_ip_range_condition() {
        let rule = AclRule::new("rule-1", "IP Range", "admin")
            .with_action(AclAction::Allow)
            .with_condition(Condition::IpRange {
                start: "192.168.1.1".to_string(),
                end: "192.168.1.100".to_string(),
            });

        let ctx_in_range = RuleContext::new()
            .with_ip("192.168.1.50".parse().unwrap());

        let ctx_out_range = RuleContext::new()
            .with_ip("192.168.2.50".parse().unwrap());

        assert_eq!(rule.evaluate(&ctx_in_range), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_out_range), None);
    }

    #[test]
    fn test_time_window_condition() {
        // Create a time window for 9:00-17:00
        let rule = AclRule::new("rule-1", "Business Hours", "admin")
            .with_action(AclAction::Allow)
            .with_condition(Condition::TimeWindow {
                start: "09:00".to_string(),
                end: "17:00".to_string(),
                days: vec![1, 2, 3, 4, 5], // Mon-Fri
                timezone: "UTC".to_string(),
            });

        // Monday at 12:00 UTC
        let monday_noon = chrono::DateTime::parse_from_rfc3339("2024-01-08T12:00:00Z").unwrap();
        let ctx_weekday = RuleContext::new()
            .with_timestamp(monday_noon.timestamp());

        // Sunday at 12:00 UTC (not in days list)
        let sunday_noon = chrono::DateTime::parse_from_rfc3339("2024-01-07T12:00:00Z").unwrap();
        let ctx_weekend = RuleContext::new()
            .with_timestamp(sunday_noon.timestamp());

        assert_eq!(rule.evaluate(&ctx_weekday), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_weekend), None);
    }

    #[test]
    fn test_rules_engine_evaluation() {
        let mut engine = AclRulesEngine::new()
            .with_default_action(AclAction::Deny);

        // Add rules (lower priority = evaluated first)
        engine.add_rule(AclRule::new("rule-1", "Deny specific", "admin")
            .with_did("did:cis:bad:*")
            .with_action(AclAction::Deny)
            .with_priority(1));

        engine.add_rule(AclRule::new("rule-2", "Allow general", "admin")
            .with_did("did:cis:*")
            .with_action(AclAction::Allow)
            .with_priority(10));

        let ctx_bad = RuleContext::new().with_did("did:cis:bad:user1");
        let ctx_good = RuleContext::new().with_did("did:cis:good:user1");

        assert_eq!(engine.evaluate(&ctx_bad), AclAction::Deny);
        assert_eq!(engine.evaluate(&ctx_good), AclAction::Allow);
    }

    #[test]
    fn test_capability_condition() {
        let rule = AclRule::new("rule-1", "Require GPU", "admin")
            .with_action(AclAction::Allow)
            .with_condition(Condition::Capability {
                required: "gpu".to_string(),
            });

        let ctx_with_gpu = RuleContext::new()
            .with_capability("gpu")
            .with_capability("storage");

        let ctx_no_gpu = RuleContext::new()
            .with_capability("storage");

        assert_eq!(rule.evaluate(&ctx_with_gpu), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_no_gpu), None);
    }

    #[test]
    fn test_multiple_conditions_and_logic() {
        let rule = AclRule::new("rule-1", "Complex", "admin")
            .with_did("did:cis:test:*")
            .with_action(AclAction::Allow)
            .with_condition(Condition::IpRange {
                start: "10.0.0.1".to_string(),
                end: "10.0.0.255".to_string(),
            })
            .with_condition(Condition::Capability {
                required: "trusted".to_string(),
            });

        // Matches all conditions
        let ctx_match = RuleContext::new()
            .with_did("did:cis:test:user1")
            .with_ip("10.0.0.50".parse().unwrap())
            .with_capability("trusted");

        // Missing capability
        let ctx_no_cap = RuleContext::new()
            .with_did("did:cis:test:user1")
            .with_ip("10.0.0.50".parse().unwrap())
            .with_capability("other");

        assert_eq!(rule.evaluate(&ctx_match), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_no_cap), None);
    }
}
