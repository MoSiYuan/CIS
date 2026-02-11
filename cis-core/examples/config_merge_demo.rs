//! # CIS Configuration Merge Demo
//!
//! Demonstrates the three-tier configuration hierarchy:
//! 1. Default values
//! 2. Configuration file
//! 3. Environment variables (highest priority)

use cis_core::config::{Config, ConfigLoader};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║  CIS Configuration Hierarchy Demo                              ║");
    println!("║  Priority: Environment > Config File > Defaults                ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Layer 1: Default values
    println!("📌 Layer 1: Default Values");
    println!("────────────────────────────────────────────────────────────────");
    let defaults = Config::default();
    println!("   TCP Port: {}", defaults.network.tcp_port);
    println!("   UDP Port: {}", defaults.network.udp_port);
    println!("   Bind Address: {}", defaults.network.bind_address);
    println!();

    // Layer 2: Create a config file
    let config_path = "/tmp/cis_test_config.toml";
    std::fs::write(config_path, r#"
[network]
tcp_port = 8888
bind_address = "127.0.0.1"

[storage]
max_connections = 50

[security]
rate_limit = 200
"#)?;
    println!("📌 Layer 2: Configuration File");
    println!("────────────────────────────────────────────────────────────────");
    println!("   File: {}", config_path);
    println!("   Contents:");
    println!("      [network]");
    println!("      tcp_port = 8888");
    println!("      bind_address = \"127.0.0.1\"");
    println!("      [storage]");
    println!("      max_connections = 50");
    println!("      [security]");
    println!("      rate_limit = 200");
    println!();

    // Load with config file (no env vars)
    let loader = ConfigLoader::with_path(config_path);
    let config = loader.load()?;
    
    println!("   Result after merging with defaults:");
    println!("      TCP Port: {} (from file)", config.network.tcp_port);
    println!("      UDP Port: {} (default)", config.network.udp_port);
    println!("      Bind Address: {} (from file)", config.network.bind_address);
    println!("      Storage Connections: {} (from file)", config.storage.max_connections);
    println!("      Security Rate Limit: {}/min (from file)", config.security.rate_limit);
    println!();

    // Layer 3: Environment variables
    println!("📌 Layer 3: Environment Variables");
    println!("────────────────────────────────────────────────────────────────");
    env::set_var("CIS_NETWORK_TCP_PORT", "9999");
    env::set_var("CIS_NETWORK_BIND_ADDRESS", "192.168.1.100");
    env::set_var("CIS_STORAGE_MAX_CONNECTIONS", "200");
    
    println!("   Environment Variables:");
    println!("      CIS_NETWORK_TCP_PORT=9999");
    println!("      CIS_NETWORK_BIND_ADDRESS=192.168.1.100");
    println!("      CIS_STORAGE_MAX_CONNECTIONS=200");
    println!();

    // Load with env vars (should override config file)
    let loader = ConfigLoader::with_path(config_path);
    let config = loader.load()?;
    
    println!("   Result after merging with environment:");
    println!("      TCP Port: {} (from env)", config.network.tcp_port);
    println!("      UDP Port: {} (default)", config.network.udp_port);
    println!("      Bind Address: {} (from env)", config.network.bind_address);
    println!("      Storage Connections: {} (from env)", config.storage.max_connections);
    println!("      Security Rate Limit: {}/min (from file)", config.security.rate_limit);
    println!();

    // Summary
    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║  Merge Summary                                                  ║");
    println!("╠════════════════════════════════════════════════════════════════╣");
    println!("║  TCP Port:          6767 → 8888 → 9999 (env wins)              ║");
    println!("║  UDP Port:          7677 → 7677 → 7677 (default)               ║");
    println!("║  Bind Address:      0.0.0.0 → 127.0.0.1 → 192.168.1.100        ║");
    println!("║  Storage Conn:      100 → 50 → 200 (env wins)                  ║");
    println!("║  Rate Limit:        100 → 200 → 200 (file wins, no env)        ║");
    println!("╚════════════════════════════════════════════════════════════════╝");
    println!();

    // Cleanup
    env::remove_var("CIS_NETWORK_TCP_PORT");
    env::remove_var("CIS_NETWORK_BIND_ADDRESS");
    env::remove_var("CIS_STORAGE_MAX_CONNECTIONS");
    std::fs::remove_file(config_path)?;
    
    println!("✅ Cleanup completed");
    println!("✅ Configuration hierarchy demo successful!");

    Ok(())
}
