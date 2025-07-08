use anyhow::Result;
use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter, RpcTransactionConfig},
    rpc_response::SlotUpdate,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
};
use solana_transaction_status::UiTransactionEncoding;
use std::time::Duration;
use tokio::{
    sync::mpsc,
    time::{interval, sleep},
};
use tracing::{info, error, warn, debug};
use futures::StreamExt;

use crate::{
    config::Config,
    storage::{Storage, StoredTransaction},
    transaction_processor::TransactionProcessor,
};

pub struct NetworkService {
    config: Config,
    storage: Storage,
    processor: TransactionProcessor,
}

impl NetworkService {
    pub async fn new(config: Config, storage: Storage) -> Result<Self> {
        Ok(Self {
            config,
            storage,
            processor: TransactionProcessor::new(),
        })
    }
    
    pub async fn run(&self) -> Result<()> {
        info!("Starting network service...");
        
        // Create channels for transaction processing
        let (tx_sender, tx_receiver) = mpsc::channel::<EncodedConfirmedTransactionWithStatusMeta>(1000);
        
        // Spawn transaction processor task
        let storage_clone = self.storage.clone();
        let processor_clone = self.processor.clone();
        tokio::spawn(Self::process_transactions(
            tx_receiver,
            storage_clone,
            processor_clone,
        ));
        
        // Spawn WebSocket listeners for each endpoint
        let mut handles = vec![];
        
        for endpoint in &self.config.network.websocket_endpoints {
            let endpoint_clone = endpoint.clone();
            let tx_sender_clone = tx_sender.clone();
            
            let handle = tokio::spawn(async move {
                loop {
                    match Self::subscribe_to_transactions(&endpoint_clone, tx_sender_clone.clone()).await {
                        Ok(_) => info!("WebSocket connection closed, reconnecting..."),
                        Err(e) => error!("WebSocket error: {}, reconnecting in 5s...", e),
                    }
                    sleep(Duration::from_secs(5)).await;
                }
            });
            
            handles.push(handle);
        }
        
        // Spawn statistics reporter
        let storage_clone = self.storage.clone();
        tokio::spawn(Self::report_statistics(storage_clone));
        
        // Wait for all tasks
        for handle in handles {
            handle.await?;
        }
        
        Ok(())
    }
    
    async fn subscribe_to_transactions(
        endpoint: &str,
        tx_sender: mpsc::Sender<EncodedConfirmedTransactionWithStatusMeta>,
    ) -> Result<()> {
        info!("Connecting to WebSocket: {}", endpoint);
        
        let pubsub_client = PubsubClient::new(endpoint).await?;
        
        // Subscribe to all transactions (you can filter by program ID if needed)
        let (mut stream, _unsub) = pubsub_client
            .logs_subscribe(
                RpcTransactionLogsFilter::All,
                RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::confirmed()),
                },
            )
            .await?;
        
        info!("Subscribed to transaction logs on {}", endpoint);
        
        // Also subscribe to slot updates for monitoring
        let (mut slot_stream, _slot_unsub) = pubsub_client
            .slot_updates_subscribe()
            .await?;
        
        // Process incoming messages
        loop {
            tokio::select! {
                Some(log) = stream.next() => {
                    debug!("Received transaction log: {}", log.value.signature);
                    
                    // Fetch full transaction details
                    match Self::fetch_transaction_details(&endpoint, &log.value.signature).await {
                        Ok(Some(tx)) => {
                            if let Err(e) = tx_sender.send(tx).await {
                                error!("Failed to send transaction to processor: {}", e);
                            }
                        }
                        Ok(None) => {
                            // Transaction might not be confirmed yet, skip for now
                            debug!("Transaction {} not found yet, might be pending", log.value.signature);
                        }
                        Err(e) => {
                            // Log as debug instead of error for expected cases
                            if e.to_string().contains("invalid type: null") {
                                debug!("Transaction {} not yet available: {}", log.value.signature, e);
                            } else {
                                error!("Failed to fetch transaction {}: {}", log.value.signature, e);
                            }
                        }
                    }
                }
                Some(slot_update) = slot_stream.next() => {
                    match slot_update {
                        SlotUpdate::FirstShredReceived { slot, .. } => {
                            debug!("First shred received for slot {}", slot);
                        }
                        SlotUpdate::Completed { slot, .. } => {
                            info!("Slot {} completed", slot);
                        }
                        _ => {}
                    }
                }
                else => break,
            }
        }
        
        Ok(())
    }
    
    async fn fetch_transaction_details(
        endpoint: &str,
        signature: &str,
    ) -> Result<Option<EncodedConfirmedTransactionWithStatusMeta>> {
        // Convert WebSocket URL to HTTP RPC URL
        let rpc_url = endpoint.replace("wss://", "https://").replace("ws://", "http://");
        
        let client = solana_client::nonblocking::rpc_client::RpcClient::new(rpc_url);
        
        let sig = signature.parse()?;
        
        // Configure to support versioned transactions
        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };
        
        match client.get_transaction_with_config(&sig, config).await {
            Ok(tx) => Ok(Some(tx)),
            Err(e) => {
                if e.to_string().contains("Transaction not found") {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }
    
    async fn process_transactions(
        mut rx: mpsc::Receiver<EncodedConfirmedTransactionWithStatusMeta>,
        storage: Storage,
        processor: TransactionProcessor,
    ) {
        let mut batch = Vec::new();
        let mut interval = interval(Duration::from_secs(5));
        
        loop {
            tokio::select! {
                Some(tx) = rx.recv() => {
                    // Process the transaction
                    match processor.process_encoded_transaction(&tx) {
                        Ok(processed) => {
                            if processor.should_store_transaction(&processed) {
                                info!("{}", processed.summary());
                                
                                let stored_tx = StoredTransaction {
                                    signature: processed.signature.clone(),
                                    slot: tx.slot,
                                    timestamp: tx.block_time.unwrap_or(0),
                                    transaction: tx,
                                };
                                
                                batch.push(stored_tx);
                                
                                // Store in batches for efficiency
                                if batch.len() >= 100 {
                                    if let Err(e) = storage.store_transactions_batch(&batch) {
                                        error!("Failed to store batch: {}", e);
                                    }
                                    batch.clear();
                                }
                            }
                        }
                        Err(e) => error!("Failed to process transaction: {}", e),
                    }
                }
                _ = interval.tick() => {
                    // Flush any remaining transactions
                    if !batch.is_empty() {
                        if let Err(e) = storage.store_transactions_batch(&batch) {
                            error!("Failed to store batch: {}", e);
                        }
                        batch.clear();
                    }
                }
            }
        }
    }
    
    async fn report_statistics(storage: Storage) {
        let mut interval = interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            match storage.get_stats() {
                Ok(stats) => {
                    info!(
                        "Storage stats - Transactions: {}, DB Size: {:.2} MB",
                        stats.transaction_count,
                        stats.db_size_bytes as f64 / 1_048_576.0
                    );
                }
                Err(e) => error!("Failed to get storage stats: {}", e),
            }
        }
    }
}

// Re-export for convenience
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta; 