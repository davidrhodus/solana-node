# Solana Node

A lightweight Rust-based Solana node for monitoring and storing blockchain transactions.


## Configuration

Create a `config.toml` file:

```toml
# Storage path for transaction data
storage_path = "./solana_node_data"

[network]
# RPC endpoints for fetching transaction details
rpc_endpoints = [
    "https://api.mainnet-beta.solana.com",
]

# WebSocket endpoints for real-time transaction streaming
websocket_endpoints = [
    "wss://api.mainnet-beta.solana.com",
]

# Maximum number of concurrent connections
max_connections = 100

[node]
# Port to listen on for metrics/API
listen_port = 8899

# Maximum number of transactions to process in a batch
max_transaction_batch_size = 1000

# How many days to retain transaction data (0 = forever)
storage_retention_days = 30
```

### Network Configurations

**Mainnet:**
```toml
rpc_endpoints = ["https://api.mainnet-beta.solana.com"]
websocket_endpoints = ["wss://api.mainnet-beta.solana.com"]
```

**Devnet:**
```toml
rpc_endpoints = ["https://api.devnet.solana.com"]
websocket_endpoints = ["wss://api.devnet.solana.com"]
```

**Testnet:**
```toml
rpc_endpoints = ["https://api.testnet.solana.com"]
websocket_endpoints = ["wss://api.testnet.solana.com"]
```

## Running

Default configuration:
```bash
cargo run --release
```

Custom configuration:
```bash
cargo run --release -- --config my-config.toml
```

Specify network:
```bash
cargo run --release -- --network devnet
```

### Command Line Options

- `--config, -c`: Path to configuration file (default: `config.toml`)
- `--network, -n`: Network to connect to: `mainnet-beta`, `testnet`, or `devnet` (default: `mainnet-beta`)

### Logging

```bash
# Info level
RUST_LOG=solana_node=info cargo run --release

# Debug level
RUST_LOG=solana_node=debug cargo run --release
``` # solana-node
