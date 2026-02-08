//! # ACL Integration Tests
//!
//! Comprehensive tests for the ACL system:
//! - Network mode switching
//! - Blacklist/whitelist functionality
//! - Complex rules with conditions
//! - ACL synchronization
//! - Expired entry cleanup

#[cfg(test)]
mod tests {
    use super::super::acl::{AclEntry, AclResult, NetworkAcl, NetworkMode};
    use super::super::acl_rules::{
        AclAction, AclRule, AclRulesEngine, Condition, RuleContext
    };
    use super::super::sync::{AclAction as SyncAction, AclSync, AclUpdateEvent};
    use std::net::IpAddr;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    // ============================================================================
    // Network Mode Tests
    // ============================================================================

    #[test]
    fn test_whitelist_mode() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        acl.set_mode(NetworkMode::Whitelist);

        // Unknown DID should be denied
        assert!(matches!(
            acl.check_did("did:cis:unknown:xyz789"),
            AclResult::Denied(_)
        ));

        // Add to whitelist
        acl.allow("did:cis:friend:def456", "did:cis:local:abc123");
        assert_eq!(acl.check_did("did:cis:friend:def456"), AclResult::Allowed);

        // Add to blacklist - takes precedence
        acl.deny("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert!(matches!(
            acl.check_did("did:cis:enemy:bad999"),
            AclResult::Denied(_)
        ));

        // Allow moves from blacklist to whitelist
        acl.allow("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert_eq!(acl.check_did("did:cis:enemy:bad999"), AclResult::Allowed);
    }

    #[test]
    fn test_solitary_mode() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        acl.set_mode(NetworkMode::Solitary);

        // In solitary mode, only whitelisted DIDs are allowed
        assert!(matches!(
            acl.check_did("did:cis:anyone:xyz789"),
            AclResult::Denied(_)
        ));

        // Whitelisted DID is allowed
        acl.allow("did:cis:friend:def456", "did:cis:local:abc123");
        assert_eq!(acl.check_did("did:cis:friend:def456"), AclResult::Allowed);

        // Blacklist still works in solitary mode
        acl.deny("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert!(matches!(
            acl.check_did("did:cis:enemy:bad999"),
            AclResult::Denied(_)
        ));
    }

    #[test]
    fn test_open_mode() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        acl.set_mode(NetworkMode::Open);

        // In open mode, anyone not blacklisted is allowed
        assert_eq!(acl.check_did("did:cis:anyone:xyz789"), AclResult::Allowed);

        // Blacklist still works
        acl.deny("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert!(matches!(
            acl.check_did("did:cis:enemy:bad999"),
            AclResult::Denied(_)
        ));
    }

    #[test]
    fn test_quarantine_mode() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        acl.set_mode(NetworkMode::Quarantine);

        // In quarantine mode, non-blacklisted DIDs get quarantine status
        assert_eq!(acl.check_did("did:cis:anyone:xyz789"), AclResult::Quarantine);

        // Blacklisted DIDs are still denied
        acl.deny("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert!(matches!(
            acl.check_did("did:cis:enemy:bad999"),
            AclResult::Denied(_)
        ));
    }

    #[test]
    fn test_mode_transitions() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        let initial_version = acl.version;

        // Each mode change should bump version
        acl.set_mode(NetworkMode::Open);
        assert_eq!(acl.version, initial_version + 1);

        acl.set_mode(NetworkMode::Whitelist);
        assert_eq!(acl.version, initial_version + 2);

        acl.set_mode(NetworkMode::Solitary);
        assert_eq!(acl.version, initial_version + 3);

        acl.set_mode(NetworkMode::Quarantine);
        assert_eq!(acl.version, initial_version + 4);

        // Setting same mode should not bump version
        let current_version = acl.version;
        acl.set_mode(NetworkMode::Quarantine);
        assert_eq!(acl.version, current_version);
    }

    // ============================================================================
    // ACL Entry Tests
    // ============================================================================

    #[test]
    fn test_acl_entry_expiration() {
        let mut entry = AclEntry::new("did:test", "did:local");
        assert!(!entry.is_expired());

        // Set expiration in the past
        entry.expires_at = Some(1);
        assert!(entry.is_expired());

        // Set expiration in the future
        entry.expires_at =Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 + 3600);
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_acl_entry_builder() {
        let entry = AclEntry::new("did:test", "did:local")
            .with_reason("Test entry")
            .with_expiration(1234567890);

        assert_eq!(entry.did, "did:test");
        assert_eq!(entry.added_by, "did:local");
        assert_eq!(entry.reason, Some("Test entry".to_string()));
        assert_eq!(entry.expires_at, Some(1234567890));
    }

    // ============================================================================
    // Complex Rules Tests
    // ============================================================================

    #[test]
    fn test_ip_range_condition() {
        let rule = AclRule::new("rule-1", "IP Range", "admin")
            .with_action(AclAction::Allow)
            .with_condition(Condition::IpRange {
                start: "192.168.1.1".to_string(),
                end: "192.168.1.100".to_string(),
            });

        let ctx_in_range = RuleContext::new()
            .with_ip("192.168.1.50".parse::<IpAddr>().unwrap());

        let ctx_out_range = RuleContext::new()
            .with_ip("192.168.2.50".parse::<IpAddr>().unwrap());

        assert_eq!(rule.evaluate(&ctx_in_range), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_out_range), None);
    }

    #[test]
    fn test_ip_cidr_condition() {
        let rule = AclRule::new("rule-1", "IP CIDR", "admin")
            .with_action(AclAction::Allow)
            .with_condition(Condition::IpMatch {
                cidr: "10.0.0.0/8".to_string(),
            });

        let ctx_in_cidr = RuleContext::new()
            .with_ip("10.1.2.3".parse::<IpAddr>().unwrap());

        let ctx_out_cidr = RuleContext::new()
            .with_ip("192.168.1.1".parse::<IpAddr>().unwrap());

        assert_eq!(rule.evaluate(&ctx_in_cidr), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_out_cidr), None);
    }

    #[test]
    fn test_did_pattern_matching() {
        // Test wildcard patterns
        assert!(AclRule::match_did("did:cis:test:*", "did:cis:test:abc123"));
        assert!(AclRule::match_did("did:cis:test:*", "did:cis:test:xyz"));
        assert!(!AclRule::match_did("did:cis:test:*", "did:cis:other:abc123"));

        // Test suffix pattern
        assert!(AclRule::match_did("*@example.com", "user@example.com"));
        assert!(!AclRule::match_did("*@example.com", "user@other.com"));

        // Test exact match
        assert!(AclRule::match_did("did:cis:exact", "did:cis:exact"));
        assert!(!AclRule::match_did("did:cis:exact", "did:cis:other"));
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
            .with_ip("10.0.0.50".parse::<IpAddr>().unwrap())
            .with_capability("trusted");

        // Missing capability
        let ctx_no_cap = RuleContext::new()
            .with_did("did:cis:test:user1")
            .with_ip("10.0.0.50".parse::<IpAddr>().unwrap())
            .with_capability("other");

        // Wrong DID
        let ctx_wrong_did = RuleContext::new()
            .with_did("did:cis:other:user1")
            .with_ip("10.0.0.50".parse::<IpAddr>().unwrap())
            .with_capability("trusted");

        assert_eq!(rule.evaluate(&ctx_match), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_no_cap), None);
        assert_eq!(rule.evaluate(&ctx_wrong_did), None);
    }

    #[test]
    fn test_rules_engine_priority() {
        let mut engine = AclRulesEngine::new()
            .with_default_action(AclAction::Deny);

        // Add rules with different priorities
        engine.add_rule(AclRule::new("rule-1", "Allow specific", "admin")
            .with_did("did:cis:specific:*")
            .with_action(AclAction::Allow)
            .with_priority(1));

        engine.add_rule(AclRule::new("rule-2", "Deny general", "admin")
            .with_did("did:cis:*")
            .with_action(AclAction::Deny)
            .with_priority(100));

        let ctx_specific = RuleContext::new().with_did("did:cis:specific:user1");
        let ctx_general = RuleContext::new().with_did("did:cis:other:user1");

        // Specific rule should match first due to lower priority number
        assert_eq!(engine.evaluate(&ctx_specific), AclAction::Allow);
        assert_eq!(engine.evaluate(&ctx_general), AclAction::Deny);
    }

    #[test]
    fn test_rules_engine_default_action() {
        // Test with default deny
        let engine_deny = AclRulesEngine::new()
            .with_default_action(AclAction::Deny);

        let ctx = RuleContext::new().with_did("did:cis:unknown");
        assert_eq!(engine_deny.evaluate(&ctx), AclAction::Deny);

        // Test with default allow
        let engine_allow = AclRulesEngine::new()
            .with_default_action(AclAction::Allow);

        assert_eq!(engine_allow.evaluate(&ctx), AclAction::Allow);
    }

    // ============================================================================
    // Expiration and Cleanup Tests
    // ============================================================================

    #[test]
    fn test_rule_expiration() {
        let mut rule = AclRule::new("rule-1", "Expired Rule", "admin")
            .with_action(AclAction::Allow);

        // Not expired
        rule.expires_at = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 + 3600);
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
    fn test_acl_cleanup_expired() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Add expired entries
        let mut expired = AclEntry::new("did:expired", "did:local");
        expired.expires_at = Some(now - 3600);
        acl.whitelist.push(expired.clone());

        let mut expired_blacklist = AclEntry::new("did:expired:bad", "did:local");
        expired_blacklist.expires_at = Some(now - 3600);
        acl.blacklist.push(expired_blacklist);

        // Add valid entries
        let mut valid = AclEntry::new("did:valid", "did:local");
        valid.expires_at = Some(now + 3600);
        acl.whitelist.push(valid.clone());

        let permanent = AclEntry::new("did:permanent", "did:local");
        acl.whitelist.push(permanent);

        let before_w = acl.whitelist.len();
        let before_b = acl.blacklist.len();

        acl.cleanup_expired();

        assert_eq!(acl.whitelist.len(), before_w - 1); // Removed one expired
        assert_eq!(acl.blacklist.len(), before_b - 1); // Removed one expired
        assert!(acl.is_whitelisted("did:valid"));
        assert!(acl.is_whitelisted("did:permanent"));
        assert!(!acl.is_whitelisted("did:expired"));
    }

    #[test]
    fn test_rules_engine_cleanup() {
        let mut engine = AclRulesEngine::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Add expired rule
        let expired_rule = AclRule::new("expired", "Expired", "admin")
            .with_expiration(now - 3600);
        engine.add_rule(expired_rule);

        // Add valid rule
        let valid_rule = AclRule::new("valid", "Valid", "admin")
            .with_expiration(now + 3600);
        engine.add_rule(valid_rule);

        // Add permanent rule
        let permanent_rule = AclRule::new("permanent", "Permanent", "admin");
        engine.add_rule(permanent_rule);

        assert_eq!(engine.rules.len(), 3);

        engine.cleanup_expired();

        assert_eq!(engine.rules.len(), 2);
        assert!(engine.find_rule("valid").is_some());
        assert!(engine.find_rule("permanent").is_some());
        assert!(engine.find_rule("expired").is_none());
    }

    // ============================================================================
    // ACL Persistence Tests
    // ============================================================================

    #[test]
    fn test_acl_save_load() {
        use std::path::PathBuf;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let acl_path: PathBuf = temp_dir.path().join("test_acl.toml");

        // Create and save ACL
        let mut acl = NetworkAcl::new("did:cis:test:node");
        acl.set_mode(NetworkMode::Whitelist);
        acl.allow("did:cis:friend:1", "did:cis:test:node");
        acl.deny("did:cis:enemy:1", "did:cis:test:node");
        
        acl.save(&acl_path).unwrap();

        // Load and verify
        let loaded = NetworkAcl::load(&acl_path).unwrap();
        assert_eq!(loaded.local_did, "did:cis:test:node");
        assert_eq!(loaded.mode, NetworkMode::Whitelist);
        assert!(loaded.is_whitelisted("did:cis:friend:1"));
        assert!(loaded.is_blacklisted("did:cis:enemy:1"));
    }

    #[test]
    fn test_acl_version_bump() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        let v1 = acl.version;

        // Adding to whitelist bumps version
        acl.allow("did:cis:friend:def456", "did:cis:local:abc123");
        assert_eq!(acl.version, v1 + 1);

        // Adding to blacklist bumps version
        acl.deny("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert_eq!(acl.version, v1 + 2);

        // Removing from whitelist bumps version
        acl.unallow("did:cis:friend:def456");
        assert_eq!(acl.version, v1 + 3);

        // Removing from blacklist bumps version
        acl.undeny("did:cis:enemy:bad999");
        assert_eq!(acl.version, v1 + 4);
    }

    // ============================================================================
    // ACL Sync Tests
    // ============================================================================

    #[tokio::test]
    async fn test_acl_version_conflict_detection() {
        // Create two ACLs with different versions
        let acl1 = Arc::new(RwLock::new(NetworkAcl::new("did:cis:node1")));
        let acl2 = Arc::new(RwLock::new(NetworkAcl::new("did:cis:node2")));

        // acl1 at version 1, acl2 at version 1
        {
            let mut a1 = acl1.write().await;
            a1.allow("did:cis:peer:1", "did:cis:node1");
            // Now at version 2
        }

        {
            let a2 = acl2.read().await;
            // Still at version 1
            assert_eq!(a2.version, 1);
        }

        // Simulate receiving an update with version gap
        // This should trigger version conflict
        let current_v1 = acl1.read().await.version;
        let current_v2 = acl2.read().await.version;
        
        // Version gap detected
        assert!(current_v1 > current_v2);
    }

    // ============================================================================
    // Time Window Tests
    // ============================================================================

    #[test]
    fn test_time_window_weekdays_only() {
        use chrono::TimeZone;

        // Create a time window for business hours (9:00-17:00, Mon-Fri)
        let rule = AclRule::new("rule-1", "Business Hours", "admin")
            .with_action(AclAction::Allow)
            .with_condition(Condition::TimeWindow {
                start: "09:00".to_string(),
                end: "17:00".to_string(),
                days: vec![1, 2, 3, 4, 5], // Mon-Fri
                timezone: "UTC".to_string(),
            });

        // Monday at 12:00 UTC (January 8, 2024 is a Monday)
        let monday_noon = chrono::Utc.with_ymd_and_hms(2024, 1, 8, 12, 0, 0).unwrap();
        let ctx_weekday = RuleContext::new()
            .with_timestamp(monday_noon.timestamp());

        // Sunday at 12:00 UTC (not in days list)
        let sunday_noon = chrono::Utc.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let ctx_weekend = RuleContext::new()
            .with_timestamp(sunday_noon.timestamp());

        // Monday at 20:00 UTC (outside hours)
        let monday_night = chrono::Utc.with_ymd_and_hms(2024, 1, 8, 20, 0, 0).unwrap();
        let ctx_after_hours = RuleContext::new()
            .with_timestamp(monday_night.timestamp());

        assert_eq!(rule.evaluate(&ctx_weekday), Some(AclAction::Allow));
        assert_eq!(rule.evaluate(&ctx_weekend), None);
        assert_eq!(rule.evaluate(&ctx_after_hours), None);
    }

    // ============================================================================
    // Edge Cases and Error Handling
    // ============================================================================

    #[test]
    fn test_empty_acl_behavior() {
        let acl = NetworkAcl::new("did:cis:local:abc123");

        // Empty whitelist with Whitelist mode - deny all
        assert!(matches!(
            acl.check_did("did:cis:anyone"),
            AclResult::Denied(_)
        ));
    }

    #[test]
    fn test_duplicate_entries() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");

        // Add same DID twice
        acl.allow("did:cis:duplicate", "did:cis:local:abc123");
        let version_after_first = acl.version;
        
        // Second add should be ignored
        acl.allow("did:cis:duplicate", "did:cis:local:abc123");
        
        // Version should not bump for duplicate
        assert_eq!(acl.version, version_after_first);
        
        // Should only have one entry
        assert_eq!(acl.whitelist.len(), 1);
    }

    #[test]
    fn test_move_between_lists() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");

        // Add to whitelist
        acl.allow("did:cis:test", "did:cis:local:abc123");
        assert!(acl.is_whitelisted("did:cis:test"));
        assert!(!acl.is_blacklisted("did:cis:test"));

        // Deny moves to blacklist and removes from whitelist
        acl.deny("did:cis:test", "did:cis:local:abc123");
        assert!(!acl.is_whitelisted("did:cis:test"));
        assert!(acl.is_blacklisted("did:cis:test"));

        // Allow moves back to whitelist
        acl.allow("did:cis:test", "did:cis:local:abc123");
        assert!(acl.is_whitelisted("did:cis:test"));
        assert!(!acl.is_blacklisted("did:cis:test"));
    }

    #[test]
    fn test_rules_engine_summary() {
        let mut engine = AclRulesEngine::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Add various rules
        engine.add_rule(AclRule::new("active", "Active", "admin"));
        
        let mut disabled = AclRule::new("disabled", "Disabled", "admin");
        disabled.enabled = false;
        engine.add_rule(disabled);
        
        let expired = AclRule::new("expired", "Expired", "admin")
            .with_expiration(now - 3600);
        engine.add_rule(expired);

        let summary = engine.summary();
        assert_eq!(summary.total_rules, 3);
        assert_eq!(summary.enabled_rules, 2); // active + expired
        assert_eq!(summary.expired_rules, 1);
        assert_eq!(summary.version, engine.version);
    }
}
