//! CIS MCP Server
//! 
//! Exposes CIS capabilities via Model Context Protocol

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing::info;

mod mcp_protocol;
mod server;

use server::CisMcpServer;

#[derive(Parser)]
#[command(name = "cis-mcp")]
#[command(about = "CIS MCP Server - AI Agent integration")]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let level = if cli.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    
    tracing_subscriber::fmt()
        .with_max_level(level)
        .init();

    info!("Starting CIS MCP Server...");

    // Initialize capability layer
    let capability = cis_capability::CapabilityLayer::new().await?;
    
    // Create MCP server
    #[allow(clippy::arc_with_non_send_sync)]
    let server = CisMcpServer::new(Arc::new(capability));

    // Run in stdio mode
    server.run_stdio().await?;

    Ok(())
}
