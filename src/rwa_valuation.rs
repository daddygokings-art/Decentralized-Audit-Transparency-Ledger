//! RWA Valuation Engine — appraisal submission, oracle feeds, and valuation history.
//!
//! Provides the `ValuationEngine` for submitting and querying asset valuations,
//! checking staleness, computing weighted-average values, and enforcing
//! minimum revaluation frequencies per asset class.
#![no_std]

use crate::rwa_types::{AssetClass, RwaToken, ValuationRecord};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum ValuationKey {
    /// Ordered valuation record IDs for a given token.
    TokenValuations(BytesN<32>),
    /// Individual valuation record by ID.
    Valuation(BytesN<32>),
    /// Latest valuation ID for a token (cache).
    LatestValuation(BytesN<32>),
    /// Authorized oracle addresses.
    AuthorizedOracle(Address),
    /// Global valuation count.
    ValuationCount,
}

// ── Valuation Engine ──────────────────────────────────────────────────────────

/// Valuation record management and freshness enforcement.
pub struct ValuationEngine;

impl ValuationEngine {
    // ── Oracle Management ─────────────────────────────────────────────────────

    /// Authorize an address as a trusted valuation oracle.
    pub fn authorize_oracle(env: &Env, oracle: &Address) {
        env.storage()
            .instance()
            .set(&ValuationKey::AuthorizedOracle(oracle.clone()), &true);
        env.events().publish(
            (
                Symbol::new(env, "rwa_valuation"),
                Symbol::new(env, "oracle_authorized"),
            ),
            oracle.clone(),
        );
    }

    /// Revoke oracle authorization.
    pub fn revoke_oracle(env: &Env, oracle: &Address) {
        env.storage()
            .instance()
            .remove(&ValuationKey::AuthorizedOracle(oracle.clone()));
        env.events().publish(
            (
                Symbol::new(env, "rwa_valuation"),
                Symbol::new(env, "oracle_revoked"),
            ),
            oracle.clone(),
        );
    }

    /// `true` if the given address is an authorized oracle.
    pub fn is_authorized_oracle(env: &Env, oracle: &Address) -> bool {
        env.storage()
            .instance()
            .get::<ValuationKey, bool>(&ValuationKey::AuthorizedOracle(oracle.clone()))
            .unwrap_or(false)
    }

    // ── Valuation Submission ──────────────────────────────────────────────────

    /// Submit a new valuation for a registered token.
    ///
    /// The caller must be an authorized oracle. The valuation is appended to
    /// the token's history and the latest-valuation cache is updated.
    pub fn submit_valuation(
        env: &Env,
        token: &mut RwaToken,
        appraiser: Address,
        value_usd_cents: u64,
        methodology: Bytes,
        valuation_date: u64,
        document_hash: BytesN<32>,
        confidence_score: u8,
        is_independent: bool,
    ) -> Result<ValuationRecord, &'static str> {
        if !Self::is_authorized_oracle(env, &appraiser) {
            return Err("Appraiser is not an authorized oracle");
        }
        if value_usd_cents == 0 {
            return Err("Valuation must be greater than zero");
        }
        if confidence_score > 100 {
            return Err("Confidence score must be 0-100");
        }
        if methodology.is_empty() {
            return Err("Methodology must be specified");
        }

        let now = env.ledger().timestamp();

        // Compute sequential index for this token's valuations.
        let existing: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&ValuationKey::TokenValuations(token.token_id.clone()))
            .unwrap_or_else(|| Vec::new(env));
        let valuation_index = existing.len();

        let record_id = Self::compute_valuation_id(env, &token.token_id, valuation_index);

        let record = ValuationRecord {
            valuation_index,
            token_id: token.token_id.clone(),
            value_usd_cents,
            methodology,
            appraiser: appraiser.clone(),
            valuation_date,
            logged_at: now,
            document_hash,
            confidence_score,
            is_independent,
        };

        // Persist the record.
        env.storage()
            .instance()
            .set(&ValuationKey::Valuation(record_id.clone()), &record);

        // Append to per-token ordered list.
        let mut ids = existing;
        ids.push_back(record_id.clone());
        env.storage()
            .instance()
            .set(&ValuationKey::TokenValuations(token.token_id.clone()), &ids);

        // Update latest cache.
        env.storage()
            .instance()
            .set(&ValuationKey::LatestValuation(token.token_id.clone()), &record_id);

        // Update token's cached valuation fields.
        token.latest_valuation_usd_cents = value_usd_cents;
        token.valuation_timestamp = valuation_date;

        // Bump global count.
        let count: u32 = env
            .storage()
            .instance()
            .get(&ValuationKey::ValuationCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ValuationKey::ValuationCount, &(count + 1));

        env.events().publish(
            (
                Symbol::new(env, "rwa_valuation"),
                Symbol::new(env, "submitted"),
            ),
            (token.token_id.clone(), value_usd_cents, appraiser),
        );

        Ok(record)
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Retrieve a specific valuation record by its ID.
    pub fn get_valuation(
        env: &Env,
        record_id: &BytesN<32>,
    ) -> Result<ValuationRecord, &'static str> {
        env.storage()
            .instance()
            .get(&ValuationKey::Valuation(record_id.clone()))
            .ok_or("Valuation record not found")
    }

    /// Return all valuation record IDs for a token in chronological order.
    pub fn list_valuations(env: &Env, token_id: &BytesN<32>) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&ValuationKey::TokenValuations(token_id.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Number of valuations recorded for a specific token.
    pub fn valuation_count_for_token(env: &Env, token_id: &BytesN<32>) -> u32 {
        Self::list_valuations(env, token_id).len()
    }

    /// Retrieve the most recent valuation record for a token.
    pub fn latest_valuation(
        env: &Env,
        token_id: &BytesN<32>,
    ) -> Result<ValuationRecord, &'static str> {
        let latest_id: BytesN<32> = env
            .storage()
            .instance()
            .get(&ValuationKey::LatestValuation(token_id.clone()))
            .ok_or("No valuation on record for this token")?;
        Self::get_valuation(env, &latest_id)
    }

    /// Total valuation records across all tokens.
    pub fn total_valuations(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&ValuationKey::ValuationCount)
            .unwrap_or(0)
    }

    // ── Freshness / Staleness ─────────────────────────────────────────────────

    /// `true` if the token's latest valuation is older than the class-required frequency.
    pub fn is_valuation_stale(env: &Env, token: &RwaToken) -> bool {
        if token.valuation_timestamp == 0 {
            return true; // Never valued.
        }
        let class = match AssetClass::from_u8(token.asset_class) {
            Some(c) => c,
            None => return true,
        };
        let max_age_secs = class.min_valuation_frequency_days() as u64 * 86_400;
        let now = env.ledger().timestamp();
        now.saturating_sub(token.valuation_timestamp) > max_age_secs
    }

    /// Age of the current valuation in seconds. Returns `u64::MAX` if never valued.
    pub fn valuation_age_secs(env: &Env, token: &RwaToken) -> u64 {
        if token.valuation_timestamp == 0 {
            return u64::MAX;
        }
        env.ledger()
            .timestamp()
            .saturating_sub(token.valuation_timestamp)
    }

    // ── Weighted Average ──────────────────────────────────────────────────────

    /// Compute the simple average of all valuations on record for a token.
    /// Returns 0 if no valuations exist.
    pub fn average_valuation(env: &Env, token_id: &BytesN<32>) -> u64 {
        let ids = Self::list_valuations(env, token_id);
        if ids.is_empty() {
            return 0;
        }
        let mut sum: u128 = 0;
        for id in ids.iter() {
            if let Ok(rec) = Self::get_valuation(env, &id) {
                sum += rec.value_usd_cents as u128;
            }
        }
        let count = Self::valuation_count_for_token(env, token_id) as u128;
        if count == 0 {
            return 0;
        }
        (sum / count) as u64
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn compute_valuation_id(env: &Env, token_id: &BytesN<32>, index: u32) -> BytesN<32> {
                let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, token_id.as_ref()));
        input.append(&Bytes::from_slice(env, &index.to_le_bytes()));
        input.append(&Bytes::from_slice(
            env,
            &env.ledger().timestamp().to_le_bytes(),
        ));
        env.crypto().sha256(&input)
    }
}
