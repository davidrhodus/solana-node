use anyhow::{Result, Context};
use rocksdb::{DB, Options, WriteBatch};
use serde::{Deserialize, Serialize};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredTransaction {
    pub signature: String,
    pub slot: u64,
    pub timestamp: i64,
    pub transaction: EncodedConfirmedTransactionWithStatusMeta,
}

#[derive(Clone)]
pub struct Storage {
    db: Arc<DB>,
}

impl Storage {
    pub fn new(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        
        // Configure for write-heavy workload
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        opts.set_max_write_buffer_number(3);
        opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB
        
        let db = DB::open(&opts, path)
            .context("Failed to open RocksDB")?;
        
        info!("Storage initialized at: {}", path);
        
        Ok(Self {
            db: Arc::new(db),
        })
    }
    
    /// Store a single transaction
    pub fn store_transaction(&self, tx: &StoredTransaction) -> Result<()> {
        let key = format!("tx:{}", tx.signature);
        let value = serde_json::to_vec(tx)?;
        
        self.db.put(key.as_bytes(), &value)?;
        
        // Also store by slot for range queries
        let slot_key = format!("slot:{}:{}", tx.slot, tx.signature);
        self.db.put(slot_key.as_bytes(), tx.signature.as_bytes())?;
        
        Ok(())
    }
    
    /// Store multiple transactions in a batch
    pub fn store_transactions_batch(&self, transactions: &[StoredTransaction]) -> Result<()> {
        let mut batch = WriteBatch::default();
        
        for tx in transactions {
            let key = format!("tx:{}", tx.signature);
            let value = serde_json::to_vec(tx)?;
            batch.put(key.as_bytes(), &value);
            
            // Index by slot
            let slot_key = format!("slot:{}:{}", tx.slot, tx.signature);
            batch.put(slot_key.as_bytes(), tx.signature.as_bytes());
        }
        
        self.db.write(batch)?;
        info!("Stored batch of {} transactions", transactions.len());
        
        Ok(())
    }
    
    /// Retrieve a transaction by signature
    pub fn get_transaction(&self, signature: &str) -> Result<Option<StoredTransaction>> {
        let key = format!("tx:{}", signature);
        
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                let tx = serde_json::from_slice(&data)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }
    
    /// Get transactions by slot range
    pub fn get_transactions_by_slot_range(
        &self, 
        start_slot: u64, 
        end_slot: u64
    ) -> Result<Vec<StoredTransaction>> {
        let mut transactions = Vec::new();
        let start_key = format!("slot:{:020}:", start_slot);
        let end_key = format!("slot:{:020}:", end_slot + 1);
        
        let iter = self.db.iterator(rocksdb::IteratorMode::From(
            start_key.as_bytes(),
            rocksdb::Direction::Forward,
        ));
        
        for item in iter {
            let (key, value) = item?;
            let key_str = String::from_utf8_lossy(&key);
            if key_str.as_ref() >= end_key.as_str() {
                break;
            }
            
            if key_str.starts_with("slot:") {
                let signature = String::from_utf8_lossy(&value);
                if let Some(tx) = self.get_transaction(&signature)? {
                    transactions.push(tx);
                }
            }
        }
        
        Ok(transactions)
    }
    
    /// Get database statistics
    pub fn get_stats(&self) -> Result<StorageStats> {
        let mut tx_count = 0;
        let iter = self.db.prefix_iterator(b"tx:");
        
        for _ in iter {
            tx_count += 1;
        }
        
        Ok(StorageStats {
            transaction_count: tx_count,
            db_size_bytes: self.estimate_db_size()?,
        })
    }
    
    fn estimate_db_size(&self) -> Result<u64> {
        // This is a rough estimate
        let props = self.db.property_value("rocksdb.estimate-live-data-size")?
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        Ok(props)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageStats {
    pub transaction_count: u64,
    pub db_size_bytes: u64,
} 