//! # ACL System Demo
//!
//! Demonstrates the CIS ACL system features:
//! - Four network modes
//! - Blacklist/whitelist management
//! - Complex rules with conditions
//! - Expiration handling

use cis_core::network::acl::{AclEntry, AclResult, NetworkAcl, NetworkMode};
use cis_core::network::acl_rules::{
    AclAction, AclRule, AclRulesEngine, Condition, RuleContext
};
use std::net::IpAddr;

fn main() {
    println!("=== CIS ACL System Demo ===\n");

    // ============================================================================
    // Demo 1: Network Modes
    // ============================================================================
    println!("--- Demo 1: Network Modes ---");
    
    let mut acl = NetworkAcl::new("did:cis:local:demo");
    
    // Add some test entries
    acl.allow("did:cis:friend:alice", "did:cis:local:demo");
    acl.deny("did:cis:enemy:mallory", "did:cis:local:demo");
    
    // Whitelist mode (default)
    acl.set_mode(NetworkMode::Whitelist);
    println!("Mode: {:?}", acl.mode);
    println!("  Unknown DID: {:?}", acl.check_did("did:cis:unknown:bob"));
    println!("  Whitelisted: {:?}", acl.check_did("did:cis:friend:alice"));
    println!("  Blacklisted: {:?}", acl.check_did("did:cis:enemy:mallory"));
    
    // Open mode
    acl.set_mode(NetworkMode::Open);
    println!("\nMode: {:?}", acl.mode);
    println!("  Unknown DID: {:?}", acl.check_did("did:cis:unknown:bob"));
    println!("  Blacklisted: {:?}", acl.check_did("did:cis:enemy:mallory"));
    
    // Solitary mode
    acl.set_mode(NetworkMode::Solitary);
    println!("\nMode: {:?}", acl.mode);
    println!("  Unknown DID: {:?}", acl.check_did("did:cis:unknown:bob"));
    println!("  Whitelisted: {:?}", acl.check_did("did:cis:friend:alice"));
    
    // Quarantine mode
    acl.set_mode(NetworkMode::Quarantine);
    println!("\nMode: {:?}", acl.mode);
    println!("  Unknown DID: {:?}", acl.check_did("did:cis:unknown:bob"));
    println!("  Blacklisted: {:?}", acl.check_did("did:cis:enemy:mallory"));

    // ============================================================================
    // Demo 2: ACL Entries with Expiration
    // ============================================================================
    println!("\n--- Demo 2: ACL Entries with Expiration ---");
    
    let mut acl2 = NetworkAcl::new("did:cis:local:demo2");
    
    // Permanent entry
    let permanent = AclEntry::new("did:cis:permanent:user", "did:cis:local:demo2")
        .with_reason("Permanent access");
    acl2.whitelist.push(permanent);
    
    // Temporary entry (not expired)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let temporary = AclEntry::new("did:cis:temporary:user", "did:cis:local:demo2")
        .with_reason("Temporary access")
        .with_expiration(now + 86400); // Expires in 24 hours
    acl2.whitelist.push(temporary);
    
    // Expired entry
    let expired = AclEntry::new("did:cis:expired:user", "did:cis:local:demo2")
        .with_reason("Expired access")
        .with_expiration(now - 3600); // Expired 1 hour ago
    acl2.whitelist.push(expired);
    
    println!("Before cleanup: {} whitelist entries", acl2.whitelist.len());
    acl2.cleanup_expired();
    println!("After cleanup: {} whitelist entries", acl2.whitelist.len());

    // ============================================================================
    // Demo 3: Complex Rules with Conditions
    // ============================================================================
    println!("\n--- Demo 3: Complex Rules with Conditions ---");
    
    let mut engine = AclRulesEngine::new()
        .with_default_action(AclAction::Deny);
    
    // Rule 1: Allow internal network during business hours
    let internal_business = AclRule::new("internal-business", "Internal Business Hours", "admin")
        .with_action(AclAction::Allow)
        .with_priority(10)
        .with_condition(Condition::IpMatch {
            cidr: "10.0.0.0/8".to_string(),
        })
        .with_condition(Condition::TimeWindow {
            start: "09:00".to_string(),
            end: "17:00".to_string(),
            days: vec![1, 2, 3, 4, 5], // Mon-Fri
            timezone: "UTC".to_string(),
        });
    engine.add_rule(internal_business);
    
    // Rule 2: Allow specific DID pattern
    let trusted_pattern = AclRule::new("trusted-partners", "Trusted Partners", "admin")
        .with_action(AclAction::Allow)
        .with_priority(20)
        .with_condition(Condition::DidPattern {
            pattern: "did:cis:partner:*".to_string(),
        });
    engine.add_rule(trusted_pattern);
    
    // Rule 3: Quarantine suspicious IPs
    let suspicious = AclRule::new("suspicious-range", "Suspicious Range", "admin")
        .with_action(AclAction::Quarantine)
        .with_priority(5) // Higher priority
        .with_condition(Condition::IpRange {
            start: "192.168.100.1".to_string(),
            end: "192.168.100.255".to_string(),
        });
    engine.add_rule(suspicious);
    
    println!("Rules engine has {} rules", engine.rules.len());
    
    // Test rule evaluation
    let test_cases = vec![
        ("Internal IP + Business hours", RuleContext::new()
            .with_ip("10.1.2.3".parse::<IpAddr>().unwrap())
            .with_timestamp(chrono::Utc::now().timestamp())),
        ("Trusted partner DID", RuleContext::new()
            .with_did("did:cis:partner:acme-corp")),
        ("Suspicious IP range", RuleContext::new()
            .with_ip("192.168.100.50".parse::<IpAddr>().unwrap())),
        ("Unknown external", RuleContext::new()
            .with_ip("8.8.8.8".parse::<IpAddr>().unwrap())
            .with_did("did:cis:unknown:external")),
    ];
    
    for (name, ctx) in test_cases {
        let action = engine.evaluate(&ctx);
        println!("  {}: {:?}", name, action);
    }

    // ============================================================================
    // Demo 4: Rules Summary
    // ============================================================================
    println!("\n--- Demo 4: Rules Summary ---");
    
    let summary = engine.summary();
    println!("Total rules: {}", summary.total_rules);
    println!("Enabled rules: {}", summary.enabled_rules);
    println!("Expired rules: {}", summary.expired_rules);
    println!("Engine version: {}", summary.version);

    // ============================================================================
    // Demo 5: ACL Persistence
    // ============================================================================
    println!("\n--- Demo 5: ACL Persistence ---");
    
    let temp_dir = std::env::temp_dir().join("cis_acl_demo");
    std::fs::create_dir_all(&temp_dir).unwrap();
    
    let acl_path = temp_dir.join("network_acl.toml");
    
    // Save ACL
    let mut acl3 = NetworkAcl::new("did:cis:local:persist");
    acl3.allow("did:cis:friend:charlie", "did:cis:local:persist");
    acl3.deny("did:cis:enemy:eve", "did:cis:local:persist");
    acl3.save(&acl_path).unwrap();
    println!("Saved ACL to {:?}", acl_path);
    
    // Load ACL
    let loaded = NetworkAcl::load(&acl_path).unwrap();
    println!("Loaded ACL:");
    println!("  Local DID: {}", loaded.local_did);
    println!("  Whitelist: {} entries", loaded.whitelist.len());
    println!("  Blacklist: {} entries", loaded.blacklist.len());
    println!("  Version: {}", loaded.version);
    
    // Cleanup
    std::fs::remove_dir_all(&temp_dir).unwrap();

    println!("\n=== Demo Complete ===");
}
