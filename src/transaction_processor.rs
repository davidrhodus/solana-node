use anyhow::{Result, Context};
use solana_sdk::{
    signature::Signature,
};
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta,
};
use std::str::FromStr;
use tracing::{debug};

#[derive(Clone)]
pub struct TransactionProcessor;

impl TransactionProcessor {
    pub fn new() -> Self {
        Self
    }
    
    /// Process an encoded transaction
    pub fn process_encoded_transaction(
        &self,
        encoded_tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> Result<ProcessedTransaction> {
        let slot = encoded_tx.slot;
        let block_time = encoded_tx.block_time;
        
        // Extract transaction data
        let transaction = &encoded_tx.transaction.transaction;
        
        // Get signatures
        let signatures = match transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_tx) => ui_tx.signatures.clone(),
            _ => return Err(anyhow::anyhow!("Unsupported transaction encoding")),
        };
        let primary_signature = signatures.first()
            .ok_or_else(|| anyhow::anyhow!("No signatures found"))?
            .clone();
        
        // Extract account keys
        let account_keys = Self::extract_account_keys(&transaction)?;
        
        // Check if it's a vote transaction
        let is_vote = Self::is_vote_transaction(&account_keys);
        
        // Extract fee
        let fee = encoded_tx.transaction.meta
            .as_ref()
            .map(|meta| meta.fee)
            .unwrap_or(0);
        
        // Extract error status
        let error = encoded_tx.transaction.meta
            .as_ref()
            .and_then(|meta| meta.err.clone())
            .map(|err| serde_json::to_value(err).unwrap_or(serde_json::Value::Null));
        
        let processed = ProcessedTransaction {
            signature: primary_signature,
            slot,
            block_time,
            fee,
            is_vote,
            error,
            account_keys,
            instruction_count: Self::count_instructions(&encoded_tx.transaction),
        };
        
        Ok(processed)
    }
    
    /// Extract account keys from transaction
    fn extract_account_keys(transaction: &solana_transaction_status::EncodedTransaction) -> Result<Vec<String>> {
        match transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed) => {
                        Ok(parsed.account_keys
                            .iter()
                            .map(|ak| ak.pubkey.clone())
                            .collect())
                    }
                    solana_transaction_status::UiMessage::Raw(raw) => {
                        Ok(raw.account_keys.clone())
                    }
                }
            }
            _ => Err(anyhow::anyhow!("Unsupported transaction encoding")),
        }
    }
    
    /// Check if transaction is a vote transaction
    fn is_vote_transaction(account_keys: &[String]) -> bool {
        const VOTE_PROGRAM_ID: &str = "Vote111111111111111111111111111111111111111";
        account_keys.contains(&VOTE_PROGRAM_ID.to_string())
    }
    
    /// Count number of instructions in transaction
    fn count_instructions(transaction_with_meta: &solana_transaction_status::EncodedTransactionWithStatusMeta) -> usize {
        match &transaction_with_meta.transaction {
            solana_transaction_status::EncodedTransaction::Json(ui_transaction) => {
                match &ui_transaction.message {
                    solana_transaction_status::UiMessage::Parsed(parsed) => {
                        parsed.instructions.len()
                    }
                    solana_transaction_status::UiMessage::Raw(raw) => {
                        raw.instructions.len()
                    }
                }
            }
            _ => 0,
        }
    }
    
    /// Validate transaction signature
    pub fn validate_signature(signature_str: &str) -> Result<Signature> {
        Signature::from_str(signature_str)
            .context("Invalid signature format")
    }
    
    /// Filter transactions based on criteria
    pub fn should_store_transaction(&self, tx: &ProcessedTransaction) -> bool {
        // Skip vote transactions if configured
        if tx.is_vote {
            debug!("Skipping vote transaction: {}", tx.signature);
            return false;
        }
        
        // Skip failed transactions if configured
        if tx.error.is_some() {
            debug!("Skipping failed transaction: {}", tx.signature);
            return false;
        }
        
        true
    }
}

#[derive(Debug, Clone)]
pub struct ProcessedTransaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub fee: u64,
    pub is_vote: bool,
    pub error: Option<serde_json::Value>,
    pub account_keys: Vec<String>,
    pub instruction_count: usize,
}

impl ProcessedTransaction {
    /// Get a summary of the transaction
    pub fn summary(&self) -> String {
        format!(
            "Tx {} | Slot: {} | Fee: {} | Instructions: {} | Accounts: {}",
            &self.signature[..8],
            self.slot,
            self.fee,
            self.instruction_count,
            self.account_keys.len()
        )
    }
} 