//! # CIS Configuration Demo
//!
//! This example demonstrates how to use the unified configuration center.

use cis_core::config::{Config, ConfigLoader, ValidateConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     CIS v1.1.4 Configuration Center Demo                 ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // Example 1: Load configuration with full hierarchy
    println!("📋 Example 1: Load Configuration");
    println!("─────────────────────────────────");
    match Config::load() {
        Ok(config) => {
            println!("✅ Configuration loaded successfully!");
            println!("   TCP Port: {}", config.network.tcp_port);
            println!("   UDP Port: {}", config.network.udp_port);
            println!("   HTTP Port: {}", config.network.http_port);
            println!("   Bind Address: {}", config.network.bind_address);
            println!("   Data Directory: {:?}", config.storage.data_dir);
        }
        Err(e) => {
            println!("⚠️  Using default configuration ({})", e);
            let config = Config::default();
            println!("   TCP Port: {}", config.network.tcp_port);
            println!("   UDP Port: {}", config.network.udp_port);
        }
    }
    println!();

    // Example 2: Create configuration with specific path
    println!("📋 Example 2: Configuration Loader");
    println!("─────────────────────────────────");
    let loader = ConfigLoader::new();
    println!("✅ Config file path: {:?}", loader.config_path());
    
    // Generate template
    let template = loader.create_template()?;
    println!("✅ Generated template ({} bytes)", template.len());
    println!();

    // Example 3: Access configuration values
    println!("📋 Example 3: Configuration Values");
    println!("─────────────────────────────────");
    let config = Config::default();
    
    println!("\n  🌐 Network:");
    println!("     TCP Bind: {}", config.tcp_bind_address());
    println!("     UDP Bind: {}", config.udp_bind_address());
    println!("     Max Connections: {}", config.network.max_connections);
    println!("     Rate Limit: {}/min", config.network.rate_limit);
    
    println!("\n  💾 Storage:");
    println!("     Data Dir: {:?}", config.storage.data_dir);
    println!("     Max Connections: {}", config.storage.max_connections);
    println!("     WAL Enabled: {}", config.storage.wal_enabled);
    
    println!("\n  🔒 Security:");
    println!("     Max Request Size: {} MB", config.security.max_request_size / 1024 / 1024);
    println!("     Rate Limit: {}/min", config.security.rate_limit);
    println!("     Min Password Length: {}", config.security.min_password_length);
    
    println!("\n  🚀 WASM:");
    println!("     Max Memory: {} MB", config.wasm.max_memory / 1024 / 1024);
    println!("     Max Execution Time: {:?}", config.wasm.max_execution_time);
    println!("     SIMD Enabled: {}", config.wasm.simd_enabled);
    
    println!("\n  🌐 P2P:");
    println!("     Enabled: {}", config.p2p.enabled);
    println!("     Max Peers: {}", config.p2p.max_peers);
    println!("     Protocol Version: {}", config.p2p.protocol_version);
    println!();

    // Example 4: Serialize to TOML
    println!("📋 Example 4: Serialization");
    println!("─────────────────────────────────");
    let toml_str = toml::to_string_pretty(&config)?;
    println!("✅ Serialized to TOML ({} bytes)", toml_str.len());
    println!("\n  First 5 lines:");
    for line in toml_str.lines().take(5) {
        println!("     {}", line);
    }
    println!();

    // Example 5: Validation
    println!("📋 Example 5: Configuration Validation");
    println!("─────────────────────────────────");
    match config.validate() {
        Ok(()) => println!("✅ Configuration is valid!"),
        Err(e) => println!("❌ Configuration error: {}", e),
    }
    println!();

    // Example 6: Environment variable integration
    println!("📋 Example 6: Environment Variables");
    println!("─────────────────────────────────");
    println!("  Supported environment variables:");
    println!("     CIS_NETWORK_TCP_PORT       - Override TCP port");
    println!("     CIS_NETWORK_UDP_PORT       - Override UDP port");
    println!("     CIS_NETWORK_BIND_ADDRESS   - Override bind address");
    println!("     CIS_STORAGE_DATA_DIR       - Override data directory");
    println!("     CIS_SECURITY_RATE_LIMIT    - Override rate limit");
    println!("     CIS_P2P_ENABLED            - Enable/disable P2P");
    println!("     CIS_P2P_MAX_PEERS          - Set max peers");
    println!();

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  Demo completed successfully!                            ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    Ok(())
}
