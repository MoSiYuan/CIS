//! # Matrix Server Management Commands
//!
//! Commands for managing the Matrix server (for Element client connections):
//! - `cis matrix start` - Start Matrix server
//! - `cis matrix stop` - Stop Matrix server
//! - `cis matrix status` - Show server status
//! - `cis matrix test` - Test Element connection

use anyhow::Result;
use clap::Subcommand;
use std::sync::Arc;
use tracing::info;

use cis_core::matrix::{
    MatrixServer, MatrixStore, element_detect,
    element_detect::{detect_element_apps, is_element_installed, print_element_status},
};

/// Matrix server management commands
#[derive(Debug, Subcommand)]
pub enum MatrixCommands {
    /// Start Matrix server for Element clients
    Start {
        /// Listen port (default: 7676)
        #[arg(short, long, default_value = "7676")]
        port: u16,
        
        /// Run in background (daemon mode)
        #[arg(long)]
        daemon: bool,
        
        /// Auto-launch Element after starting server
        #[arg(long)]
        launch: bool,
    },
    
    /// Stop Matrix server
    Stop,
    
    /// Show Matrix server status
    Status,
    
    /// Test Element app detection and connection
    Test {
        /// Test connecting to a specific homeserver
        #[arg(short = 's', long)]
        homeserver: Option<String>,
    },
    
    /// Detect installed Element apps
    Detect,
}

/// Handle matrix commands
pub async fn handle(cmd: MatrixCommands) -> Result<()> {
    match cmd {
        MatrixCommands::Start { port, daemon, launch } => {
            start_matrix_server(port, daemon, launch).await?;
        }
        MatrixCommands::Stop => {
            stop_matrix_server().await?;
        }
        MatrixCommands::Status => {
            show_matrix_status().await?;
        }
        MatrixCommands::Test { homeserver } => {
            test_element_connection(homeserver).await?;
        }
        MatrixCommands::Detect => {
            print_element_status();
        }
    }
    Ok(())
}

/// Start Matrix server
async fn start_matrix_server(port: u16, daemon: bool, launch: bool) -> Result<()> {
    println!("🚀 Starting Matrix server...");
    println!("   Port: {}", port);
    println!("   URL: http://localhost:{}", port);
    
    // Check if Element is installed
    if is_element_installed() {
        println!("\n✅ Element app detected");
        print_element_status();
    } else {
        println!("\n⚠️  Element app not detected");
        println!("   Download: https://element.io/download");
    }
    
    // Create store
    let store = Arc::new(MatrixStore::open_in_memory()?);
    
    // Create and start server
    let server = MatrixServer::new(port, store);
    
    println!("\n📡 Matrix server is ready!");
    println!("   Clients can connect to: http://localhost:{}", port);
    println!("   Well-known: http://localhost:{}/.well-known/matrix/client", port);
    
    if daemon {
        println!("\n👻 Running in daemon mode (not yet implemented)");
        println!("   Server will run in foreground for now");
    }
    
    // Auto-launch Element if requested
    if launch {
        if is_element_installed() {
            println!("\n🚀 Launching Element...");
            let homeserver = format!("http://localhost:{}", port);
            match element_detect::launch_element_with_homeserver(&homeserver) {
                Ok(_) => println!("✅ Element launched"),
                Err(e) => println!("⚠️  Failed to launch Element: {}", e),
            }
        } else {
            println!("\n⚠️  Cannot launch Element - not installed");
        }
    }
    
    println!("\n💡 Connection info for Element:");
    println!("   Homeserver URL: http://localhost:{}", port);
    println!("   (Use this when signing in to Element)");
    
    // Start the server (this blocks)
    info!("Starting Matrix server on port {}", port);
    server.run().await.map_err(|e| {
        anyhow::anyhow!("Matrix server error: {}", e)
    })?;
    
    Ok(())
}

/// Stop Matrix server
async fn stop_matrix_server() -> Result<()> {
    println!("🛑 Stopping Matrix server...");
    println!("   (Implementation: read PID from ~/.cis/matrix-server.pid and kill)");
    
    // TODO: Implement PID file tracking and graceful shutdown
    println!("   Not yet implemented - use Ctrl+C to stop");
    
    Ok(())
}

/// Show Matrix server status
async fn show_matrix_status() -> Result<()> {
    println!("📊 Matrix Server Status");
    println!("======================\n");
    
    // Check Element installation
    println!("Element Client:");
    print_element_status();
    
    println!("\nMatrix Server:");
    // TODO: Check if server is running via PID file
    println!("   Status: Unknown (PID tracking not implemented)");
    println!("   Default port: 7676");
    
    println!("\n💡 To start the server:");
    println!("   cis matrix start");
    println!("   cis matrix start --launch  # Auto-launch Element");
    
    Ok(())
}

/// Test Element connection
async fn test_element_connection(homeserver: Option<String>) -> Result<()> {
    println!("🧪 Testing Element Configuration");
    println!("==============================\n");
    
    // Test 1: Detect Element
    println!("1. Checking Element installation...");
    let apps = detect_element_apps();
    if apps.is_empty() {
        println!("   ❌ Element not found");
        println!("   Install from: https://element.io/download");
    } else {
        println!("   ✅ Found {} Element app(s)", apps.len());
        for app in &apps {
            println!("      - {} at {}", app.name, app.path.display());
        }
    }
    
    // Test 2: Check homeserver connectivity
    println!("\n2. Checking homeserver connectivity...");
    let hs_url = homeserver.unwrap_or_else(|| "http://localhost:7676".to_string());
    
    match reqwest::get(format!("{}/_matrix/client/versions", hs_url)).await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ Homeserver is reachable at {}", hs_url);
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(versions) = body.get("versions") {
                        println!("   Supported versions: {:?}", versions);
                    }
                }
            } else {
                println!("   ❌ Homeserver returned error: {}", resp.status());
            }
        }
        Err(e) => {
            println!("   ❌ Cannot connect to homeserver: {}", e);
            println!("   Make sure the server is running: cis matrix start");
        }
    }
    
    // Test 3: Well-known check
    println!("\n3. Checking .well-known configuration...");
    match reqwest::get(format!("{}/.well-known/matrix/client", hs_url)).await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ .well-known is configured");
            } else {
                println!("   ⚠️  .well-known not configured (optional)");
            }
        }
        Err(_) => {
            println!("   ⚠️  .well-known not accessible");
        }
    }
    
    println!("\n✨ Test complete!");
    
    Ok(())
}
