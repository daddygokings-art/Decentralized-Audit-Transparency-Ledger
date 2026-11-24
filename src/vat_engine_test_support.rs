#![cfg(test)]

use crate::merkle::{merkle_proof, merkle_root, verify_merkle_proof};
use crate::tax::{TaxJurisdiction, VATDetermination, VATExemptionReason, VATSupplyType, VATTransaction};
use crate::vat_engine::VATDeterminationEngine;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Bytes, BytesN, Env, Vec as SorobanVec};
use std::collections::BTreeSet;
use std::vec::Vec;

#[derive(Clone, Debug)]
pub(crate) struct VatInput {
    pub id: u8,
    pub timestamp: u64,
    pub amount: u64,
    pub rate: u32,
    pub jurisdiction: u8,
    pub customer_jurisdiction: u8,
    pub supply_type: u8,
    pub exemption: u8,
    pub is_b2b: bool,
}

pub(crate) fn make_transaction(env: &Env, input: &VatInput, supplier: &Address) -> VATTransaction {
    let jurisdictions = [
        TaxJurisdiction::EU,
        TaxJurisdiction::UK,
        TaxJurisdiction::US,
        TaxJurisdiction::Canada,
        TaxJurisdiction::Australia,
        TaxJurisdiction::India,
        TaxJurisdiction::Singapore,
        TaxJurisdiction::HongKong,
        TaxJurisdiction::Japan,
        TaxJurisdiction::Switzerland,
        TaxJurisdiction::UAE,
    ];
    let supply_types = [
        VATSupplyType::Goods,
        VATSupplyType::Services,
        VATSupplyType::Intangibles,
        VATSupplyType::DigitalServices,
        VATSupplyType::Construction,
        VATSupplyType::Transportation,
        VATSupplyType::Telecommunications,
    ];
    let exemptions = [
        VATExemptionReason::Financial,
        VATExemptionReason::Healthcare,
        VATExemptionReason::Education,
        VATExemptionReason::ExportedGoods,
        VATExemptionReason::IntraEUSupply,
        VATExemptionReason::CulturalActivities,
        VATExemptionReason::AgriculturalProduction,
        VATExemptionReason::None,
    ];

    VATTransaction {
        id: BytesN::from_array(env, &[input.id; 32]),
        supplier: supplier.clone(),
        customer: Address::generate(env),
        supply_type: supply_types[input.supply_type as usize % supply_types.len()],
        supply_amount: input.amount,
        currency: soroban_sdk::Symbol::new(env, "EUR"),
        timestamp: input.timestamp,
        place_of_supply: jurisdictions[input.jurisdiction as usize % jurisdictions.len()],
        customer_jurisdiction: jurisdictions[input.customer_jurisdiction as usize % jurisdictions.len()],
        is_b2b: input.is_b2b,
        reverse_charge: false,
        exemption_reason: exemptions[input.exemption as usize % exemptions.len()],
        vat_rate: input.rate,
        vat_amount: 0,
        description: Bytes::new(env),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JournalError {
    InvalidTransaction,
    DuplicateTransaction,
    OutOfOrderTimestamp,
    InjectedFailure,
}

pub(crate) struct VatJournal {
    env: Env,
    transactions: Vec<VATTransaction>,
    determinations: Vec<VATDetermination>,
    ids: BTreeSet<[u8; 32]>,
    leaves: Vec<BytesN<32>>,
    chain: Vec<BytesN<32>>,
    last_timestamp: Option<u64>,
}

impl VatJournal {
    pub(crate) fn new(env: Env) -> Self {
        Self {
            env,
            transactions: Vec::new(),
            determinations: Vec::new(),
            ids: BTreeSet::new(),
            leaves: Vec::new(),
            chain: Vec::new(),
            last_timestamp: None,
        }
    }

    pub(crate) fn apply(&mut self, transaction: VATTransaction) -> Result<(), JournalError> {
        if !VATDeterminationEngine::validate_transaction(&transaction) {
            return Err(JournalError::InvalidTransaction);
        }

        let id = transaction.id.to_array();
        if self.ids.contains(&id) {
            return Err(JournalError::DuplicateTransaction);
        }
        if self.last_timestamp.is_some_and(|last| transaction.timestamp < last) {
            return Err(JournalError::OutOfOrderTimestamp);
        }

        self.env.ledger().set_timestamp(transaction.timestamp);
        let determination = VATDeterminationEngine::determine_vat(&self.env, &transaction);
        let leaf = determination_leaf(&self.env, &transaction, &determination);
        let previous = self
            .chain
            .last()
            .cloned()
            .unwrap_or_else(|| BytesN::from_array(&self.env, &[0; 32]));
        let chain_hash = hash_chain_link(&self.env, &previous, &leaf);

        self.ids.insert(id);
        self.transactions.push(transaction.clone());
        self.determinations.push(determination);
        self.leaves.push(leaf);
        self.chain.push(chain_hash);
        self.last_timestamp = Some(transaction.timestamp);
        Ok(())
    }

    pub(crate) fn apply_batch(
        &mut self,
        transactions: &[VATTransaction],
        fail_at: Option<usize>,
    ) -> Result<(), JournalError> {
        let original_len = self.transactions.len();
        let original_ids = self.ids.clone();
        let original_timestamp = self.last_timestamp;

        let result = transactions.iter().enumerate().try_for_each(|(index, transaction)| {
            if fail_at == Some(index) {
                return Err(JournalError::InjectedFailure);
            }
            self.apply(transaction.clone())
        });

        if result.is_err() {
            self.transactions.truncate(original_len);
            self.determinations.truncate(original_len);
            self.leaves.truncate(original_len);
            self.chain.truncate(original_len);
            self.ids = original_ids;
            self.last_timestamp = original_timestamp;
            self.env.ledger().set_timestamp(original_timestamp.unwrap_or(0));
        }

        result
    }

    pub(crate) fn transactions(&self) -> &[VATTransaction] {
        &self.transactions
    }

    pub(crate) fn determinations(&self) -> &[VATDetermination] {
        &self.determinations
    }

    pub(crate) fn snapshot(&self) -> (Vec<[u8; 32]>, Vec<BytesN<32>>, Vec<BytesN<32>>, Option<u64>) {
        (
            self.transactions
                .iter()
                .map(|transaction| transaction.id.to_array())
                .collect(),
            self.leaves.clone(),
            self.chain.clone(),
            self.last_timestamp,
        )
    }

    pub(crate) fn recover(&self) -> Self {
        let mut recovered = Self::new(self.env.clone());
        for transaction in &self.transactions {
            recovered
                .apply(transaction.clone())
                .expect("accepted VAT transaction must replay");
        }
        recovered
    }

    pub(crate) fn assert_integrity(&self) -> bool {
        if self.transactions.len() != self.determinations.len()
            || self.transactions.len() != self.leaves.len()
            || self.transactions.len() != self.chain.len()
        {
            return false;
        }

        let mut previous = BytesN::from_array(&self.env, &[0; 32]);
        for index in 0..self.transactions.len() {
            let leaf = determination_leaf(&self.env, &self.transactions[index], &self.determinations[index]);
            if leaf != self.leaves[index] {
                return false;
            }
            let expected = hash_chain_link(&self.env, &previous, &leaf);
            if expected != self.chain[index] {
                return false;
            }
            previous = expected;
        }

        if self.leaves.is_empty() {
            return self.last_timestamp.is_none();
        }
        let mut leaves = SorobanVec::new(&self.env);
        for leaf in &self.leaves {
            leaves.push_back(leaf.clone());
        }
        let root = merkle_root(&self.env, &leaves);
        let all_proofs_verify = self.leaves.iter().enumerate().all(|(index, leaf)| {
            let Some(proof) = merkle_proof(&self.env, &leaves, index as u32) else {
                return false;
            };
            verify_merkle_proof(&self.env, leaf, index as u32, self.leaves.len() as u32, &proof, &root)
        });
        if !all_proofs_verify {
            return false;
        }

        let mut tampered_leaf = self.leaves[0].to_array();
        tampered_leaf[0] ^= 1;
        let tampered_leaf = BytesN::from_array(&self.env, &tampered_leaf);
        let Some(proof) = merkle_proof(&self.env, &leaves, 0) else {
            return false;
        };
        !verify_merkle_proof(&self.env, &tampered_leaf, 0, self.leaves.len() as u32, &proof, &root)
    }
}

fn determination_leaf(env: &Env, transaction: &VATTransaction, determination: &VATDetermination) -> BytesN<32> {
    let mut payload = Vec::with_capacity(32 + 8 * 3 + 4 * 5 + 2);
    payload.extend_from_slice(&transaction.id.to_array());
    payload.extend_from_slice(&transaction.timestamp.to_be_bytes());
    payload.extend_from_slice(&transaction.supply_amount.to_be_bytes());
    payload.extend_from_slice(&determination.determined_at.to_be_bytes());
    payload.extend_from_slice(&determination.vat_rate.to_be_bytes());
    payload.extend_from_slice(&determination.vat_amount.to_be_bytes());
    payload.extend_from_slice(&(determination.place_of_supply as u32).to_be_bytes());
    payload.extend_from_slice(&(determination.exemption_reason as u32).to_be_bytes());
    payload.push(u8::from(determination.is_exempt));
    payload.push(u8::from(determination.reverse_charge_applicable));
    env.crypto().sha256(&Bytes::from_slice(env, &payload)).to_bytes()
}

fn hash_chain_link(env: &Env, previous: &BytesN<32>, leaf: &BytesN<32>) -> BytesN<32> {
    let mut payload = Vec::with_capacity(65);
    payload.push(2);
    payload.extend_from_slice(&previous.to_array());
    payload.extend_from_slice(&leaf.to_array());
    env.crypto().sha256(&Bytes::from_slice(env, &payload)).to_bytes()
}
