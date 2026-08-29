#![no_std]

use crate::cbdc_types::{BatchSettlement, CBDCTransaction, OfflineStatus};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Represents an offline-signed transaction before reconciliation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineTransaction {
    /// Transaction hash (deterministic ID)
    pub tx_hash: BytesN<32>,
    /// The actual transaction
    pub transaction: CBDCTransaction,
    /// Cryptographic signature (e.g., Ed25519)
    pub signature: Bytes,
    /// Public key of signer
    pub signer_pubkey: Bytes,
    /// Reconciliation status
    pub status: u8, // OfflineStatus as u8
    /// Timestamp of offline creation
    pub created_at: u64,
    /// Timestamp of reconciliation (if any)
    pub reconciled_at: Option<u64>,
    /// Nonce to prevent replay attacks
    pub nonce: u32,
}

impl OfflineTransaction {
    /// Verify signature is valid (signature format check only, actual verification delegated)
    pub fn has_valid_signature_format(&self) -> bool {
        !self.signature.is_empty() && !self.signer_pubkey.is_empty()
    }

    /// Check if transaction is settled
    pub fn is_settled(&self) -> bool {
        self.status == OfflineStatus::Reconciled as u8
    }

    /// Check if transaction has failed
    pub fn is_failed(&self) -> bool {
        self.status == OfflineStatus::FailedReconciliation as u8
    }
}

/// Batch reconciliation state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciliationState {
    /// Total transactions in batch
    pub total_transactions: u32,
    /// Successfully reconciled transactions
    pub successful_count: u32,
    /// Failed transactions
    pub failed_count: u32,
    /// Pending reconciliation
    pub pending_count: u32,
    /// Dispute/reversal transactions
    pub disputed_count: u32,
}

impl ReconciliationState {
    pub fn new() -> Self {
        ReconciliationState {
            total_transactions: 0,
            successful_count: 0,
            failed_count: 0,
            pending_count: 0,
            disputed_count: 0,
        }
    }

    pub fn all_settled(&self) -> bool {
        self.successful_count == self.total_transactions
    }

    pub fn any_failed(&self) -> bool {
        self.failed_count > 0 || self.disputed_count > 0
    }

    pub fn reconciliation_rate(&self) -> u32 {
        if self.total_transactions == 0 {
            return 100;
        }
        ((self.successful_count as u64 * 100) / self.total_transactions as u64) as u32
    }
}

/// Offline transaction manager.
pub struct OfflineManager;

impl OfflineManager {
    /// Create offline transaction with signature
    pub fn create_offline_transaction(
        env: &Env,
        transaction: CBDCTransaction,
        signature: Bytes,
        signer_pubkey: Bytes,
        nonce: u32,
    ) -> Result<OfflineTransaction, &'static str> {
        if signature.is_empty() {
            return Err("Signature cannot be empty");
        }
        if signer_pubkey.is_empty() {
            return Err("Signer public key cannot be empty");
        }

        // Compute hash of transaction for offline verification
        let tx_hash = Self::compute_offline_tx_hash(env, &transaction, nonce);

        Ok(OfflineTransaction {
            tx_hash,
            transaction,
            signature,
            signer_pubkey,
            status: OfflineStatus::PendingReconciliation as u8,
            created_at: env.ledger().timestamp(),
            reconciled_at: None,
            nonce,
        })
    }

    /// Compute deterministic offline transaction hash
    pub fn compute_offline_tx_hash(
        env: &Env,
        transaction: &CBDCTransaction,
        nonce: u32,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);

        // Hash transaction fields
        input.append(&Bytes::from_slice(
            env,
            transaction.tx_id.as_ref(),
        ));
        input.append(&Bytes::from_slice(env, &transaction.source_pilot.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &transaction.dest_pilot.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &transaction.amount_source.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &transaction.amount_dest.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &transaction.exchange_rate.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &nonce.to_le_bytes()));

        env.crypto().sha256(&input)
    }

    /// Validate offline transaction before reconciliation
    pub fn validate_offline_transaction(
        tx: &OfflineTransaction,
    ) -> Result<(), &'static str> {
        if !tx.has_valid_signature_format() {
            return Err("Invalid signature format");
        }
        if tx.transaction.amount_source == 0 {
            return Err("Transaction amount cannot be zero");
        }
        if tx.nonce == 0 {
            // Nonce can be 1+, 0 is reserved
            return Err("Invalid nonce");
        }
        Ok(())
    }

    /// Reconcile offline transaction to on-chain state
    pub fn reconcile_transaction(
        env: &Env,
        offline_tx: &mut OfflineTransaction,
    ) -> Result<(), &'static str> {
        if offline_tx.status != OfflineStatus::PendingReconciliation as u8 {
            return Err("Transaction is not in pending state");
        }

        // Mark as reconciled
        offline_tx.status = OfflineStatus::Reconciled as u8;
        offline_tx.reconciled_at = Some(env.ledger().timestamp());

        Ok(())
    }

    /// Mark transaction as failed reconciliation
    pub fn fail_reconciliation(
        env: &Env,
        offline_tx: &mut OfflineTransaction,
    ) -> Result<(), &'static str> {
        if offline_tx.status == OfflineStatus::Reconciled as u8 {
            return Err("Cannot fail already reconciled transaction");
        }

        offline_tx.status = OfflineStatus::FailedReconciliation as u8;
        offline_tx.reconciled_at = Some(env.ledger().timestamp());

        Ok(())
    }

    /// Create batch settlement from offline transactions
    pub fn create_batch_settlement(
        env: &Env,
        tx_ids: Vec<BytesN<32>>,
        total_amount: u128,
    ) -> Result<BatchSettlement, &'static str> {
        if tx_ids.len() == 0 {
            return Err("Batch cannot be empty");
        }
        if total_amount == 0 {
            return Err("Batch amount cannot be zero");
        }

        let batch_id = Self::compute_batch_id(env, &tx_ids);

        Ok(BatchSettlement {
            batch_id,
            transaction_ids: tx_ids,
            total_amount,
            settlement_status: OfflineStatus::PendingReconciliation as u8,
            created_at: env.ledger().timestamp(),
            settled_at: None,
        })
    }

    /// Compute deterministic batch ID
    pub fn compute_batch_id(env: &Env, tx_ids: &Vec<BytesN<32>>) -> BytesN<32> {
        
        let mut input = Bytes::new(env);

        for tx_id in tx_ids.iter() {
            input.append(&Bytes::from_slice(env, tx_id.as_ref()));
        }

        env.crypto().sha256(&input)
    }

    /// Settle batch on-chain
    pub fn settle_batch(
        env: &Env,
        batch: &mut BatchSettlement,
    ) -> Result<(), &'static str> {
        if batch.settlement_status != OfflineStatus::PendingReconciliation as u8 {
            return Err("Batch is not in pending state");
        }

        batch.settlement_status = OfflineStatus::Reconciled as u8;
        batch.settled_at = Some(env.ledger().timestamp());

        Ok(())
    }

    /// Detect and prevent replay attacks using nonce
    pub fn verify_nonce(
        last_nonce: u32,
        current_nonce: u32,
    ) -> Result<(), &'static str> {
        if current_nonce <= last_nonce {
            return Err("Nonce replay detected or invalid sequence");
        }
        Ok(())
    }

    /// Compute reconciliation state from transaction statuses
    pub fn compute_reconciliation_state(
        statuses: &Vec<u8>,
    ) -> ReconciliationState {
        let mut state = ReconciliationState::new();
        state.total_transactions = statuses.len() as u32;

        for status in statuses.iter() {
            match *status {
                s if s == OfflineStatus::Reconciled as u8 => state.successful_count += 1,
                s if s == OfflineStatus::FailedReconciliation as u8 => state.failed_count += 1,
                s if s == OfflineStatus::PendingReconciliation as u8 => state.pending_count += 1,
                s if s == OfflineStatus::Disputed as u8 => state.disputed_count += 1,
                _ => {}
            }
        }

        state
    }

    /// Verify offline transaction integrity
    pub fn verify_transaction_integrity(
        env: &Env,
        offline_tx: &OfflineTransaction,
    ) -> bool {
        let recomputed_hash = Self::compute_offline_tx_hash(
            env,
            &offline_tx.transaction,
            offline_tx.nonce,
        );
        recomputed_hash == offline_tx.tx_hash
    }
}

/// Offline transaction queue for batching reconciliations.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciliationQueue {
    /// Queue of transaction hashes pending reconciliation
    pub pending_tx_hashes: soroban_sdk::Vec<BytesN<32>>,
    /// Maximum queue size before flush
    pub max_queue_size: u32,
    /// Timestamp of last flush
    pub last_flush_time: u64,
}

impl ReconciliationQueue {
    pub fn new(env: &Env, max_size: u32) -> Self {
        ReconciliationQueue {
            pending_tx_hashes: Vec::new(env),
            max_queue_size: max_size,
            last_flush_time: 0,
        }
    }

    pub fn is_full(&self) -> bool {
        self.pending_tx_hashes.len() as u32 >= self.max_queue_size
    }

    pub fn add_transaction(&mut self, tx_hash: BytesN<32>) -> Result<(), &'static str> {
        if self.is_full() {
            return Err("Reconciliation queue is full");
        }
        self.pending_tx_hashes.push_back(tx_hash);
        Ok(())
    }

    pub fn clear(&mut self, timestamp: u64) {
        self.pending_tx_hashes.clear();
        self.last_flush_time = timestamp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offline_transaction_status() {
        let offline_tx = OfflineTransaction {
            tx_hash: BytesN::zero(),
            transaction: CBDCTransaction {
                tx_id: BytesN::zero(),
                source_pilot: 0,
                dest_pilot: 1,
                from: Address::generate(&soroban_sdk::Env::default()),
                to: Address::generate(&soroban_sdk::Env::default()),
                amount_source: 1000,
                amount_dest: 1200,
                exchange_rate: 1_200_000_000_000_000_000,
                timestamp: 1000,
                protocol: 0,
                privacy_tier: 0,
                offline_status: Some(0),
                metadata: Bytes::new(&soroban_sdk::Env::default()),
            },
            signature: Bytes::new(&soroban_sdk::Env::default()),
            signer_pubkey: Bytes::new(&soroban_sdk::Env::default()),
            status: OfflineStatus::PendingReconciliation as u8,
            created_at: 1000,
            reconciled_at: None,
            nonce: 1,
        };

        assert!(!offline_tx.is_settled());
        assert!(!offline_tx.is_failed());
    }

    #[test]
    fn test_reconciliation_state_calculation() {
        let statuses = Vec::from_array(
            &soroban_sdk::Env::default(),
            &[
                OfflineStatus::Reconciled as u8,
                OfflineStatus::Reconciled as u8,
                OfflineStatus::FailedReconciliation as u8,
            ],
        );

        let state = OfflineManager::compute_reconciliation_state(&statuses);
        assert_eq!(state.total_transactions, 3);
        assert_eq!(state.successful_count, 2);
        assert_eq!(state.failed_count, 1);
        assert_eq!(state.reconciliation_rate(), 66);
    }

    #[test]
    fn test_nonce_replay_detection() {
        assert!(OfflineManager::verify_nonce(1, 2).is_ok());
        assert!(OfflineManager::verify_nonce(5, 5).is_err()); // same nonce
        assert!(OfflineManager::verify_nonce(5, 3).is_err()); // lower nonce
    }
}
