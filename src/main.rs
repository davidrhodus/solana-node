mod config;
mod network;
mod storage;
mod transaction_processor;

use anyhow::Result;
use clap::Parser;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    /// Network to connect to (mainnet-beta, testnet, devnet)
    #[arg(short, long, default_value = "mainnet-beta")]
    network: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "solana_node=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    
    info!("Starting Solana node...");
    info!("Network: {}", args.network);
    
    // Load configuration
    let config = config::load_config(&args.config)?;
    
    // Initialize storage
    let storage = storage::Storage::new(&config.storage_path)?;
    
    // Start network services
    let network_service = network::NetworkService::new(config.clone(), storage.clone()).await?;
    
    // Run the node
    match network_service.run().await {
        Ok(_) => info!("Node shutdown gracefully"),
        Err(e) => error!("Node error: {}", e),
    }
    
    Ok(())
}
