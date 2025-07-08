use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub storage_path: String,
    pub network: NetworkConfig,
    pub node: NodeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub rpc_endpoints: Vec<String>,
    pub websocket_endpoints: Vec<String>,
    pub gossip_entrypoints: Vec<String>,
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub identity_keypair_path: Option<String>,
    pub listen_port: u16,
    pub max_transaction_batch_size: usize,
    pub storage_retention_days: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage_path: "./solana_node_data".to_string(),
            network: NetworkConfig {
                rpc_endpoints: vec![
                    "https://api.mainnet-beta.solana.com".to_string(),
                ],
                websocket_endpoints: vec![
                    "wss://api.mainnet-beta.solana.com".to_string(),
                ],
                gossip_entrypoints: vec![
                    "entrypoint.mainnet-beta.solana.com:8001".to_string(),
                ],
                max_connections: 100,
            },
            node: NodeConfig {
                identity_keypair_path: None,
                listen_port: 8899,
                max_transaction_batch_size: 1000,
                storage_retention_days: 30,
            },
        }
    }
}

pub fn load_config(path: &str) -> Result<Config> {
    if !std::path::Path::new(path).exists() {
        // Create default config file if it doesn't exist
        let default_config = Config::default();
        let toml_string = toml::to_string_pretty(&default_config)?;
        fs::write(path, toml_string)?;
        return Ok(default_config);
    }
    
    let contents = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
} 