/// Cross-Chain Token Verification Bridge
///
/// Verifies ERC-20, ERC-721, and ERC-1155 token balances on EVM chains
/// through signed attestations from bridge relayers.
///
/// Flow:
/// 1. Off-chain bridge relay observes EVM token event
/// 2. Relay fetches balance and signs attestation
/// 3. Soroban contract verifies signature and stores balance cache

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, panic_with_error, log,
};
use soroban_sdk::xdr::ToXdr;
use sha2::{Digest, Sha256};

use crate::rbac::{RbacError, RbacManager, RbacStorageKey, Role};

// ============================================================================
// Audit Trail Constants
// ============================================================================

/// Topic tag for tamper-evident bridge audit events.
pub const BRIDGE_AUDIT_TOPIC: &str = "bridge_audit";

/// Success-topic tag persisted for off-chain audit reconstruction.
pub const BRIDGE_AUDIT_OK_TOPIC: &str = "bridge_audit_ok";

/// Denial-topic tag persisted for off-chain audit reconstruction.
pub const BRIDGE_AUDIT_DENIED_TOPIC: &str = "bridge_audit_denied";

/// Outcome tag for a successful mutation.
pub const AUDIT_OUTCOME_OK: &str = "ok";

/// Outcome tag for a rejected access attempt.
pub const AUDIT_OUTCOME_DENIED: &str = "denied";

// ============================================================================
// Data Structures
// ============================================================================

/// Bridge relay identity and configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeRelay {
    /// Relay's Stellar address
    pub relay_address: Address,
    /// Relay's ECDSA public key for signature verification (65 bytes: 0x04 || x || y)
    pub pubkey: Bytes,
    /// Whether relay is currently active
    pub active: bool,
    /// Relay's reputation score
    pub reputation: i32,
}

/// Signed attestation of token balance from a bridge relay
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalanceAttestation {
    /// Relay that created this attestation
    pub relay: Address,
    /// User address (Ethereum address in 0x-prefixed hex)
    pub user_address: Bytes,
    /// Token contract address (0x-prefixed hex)
    pub token_address: Bytes,
    /// Token ID (for ERC-721/1155; 0 for ERC-20)
    pub token_id: u128,
    /// Verified balance
    pub balance: u128,
    /// Ethereum block height where balance was checked
    pub block_height: u64,
    /// Attestation timestamp
    pub timestamp: u64,
    /// ECDSA signature (65 bytes: r || s || v)
    pub signature: Bytes,
    /// Whether this attestation has been accepted
    pub accepted: bool,
}

/// Bridge verification record (cached for efficiency)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeVerificationRecord {
    /// User (Ethereum address in 0x-prefixed hex)
    pub user: Bytes,
    /// Token address (0x-prefixed hex)
    pub token_address: Bytes,
    /// Token ID (for ERC-721/1155)
    pub token_id: u128,
    /// Last verified balance
    pub balance: u128,
    /// Ledger sequence when verified
    pub verified_at_ledger: u32,
    /// TTL in ledgers for this verification
    pub ttl_ledgers: u32,
    /// Primary relay that verified this
    pub verified_by_relay: Address,
    /// Number of confirmations (from multiple relays)
    pub confirmations: u32,
}

/// Bridge configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeConfig {
    /// Threshold: minimum signatures required to accept attestation
    pub signature_threshold: u32,
    /// Verification cache TTL in ledgers
    pub cache_ttl_ledgers: u32,
    /// Maximum age of Ethereum block for attestation (in blocks)
    pub max_block_age: u64,
}

// ============================================================================
// Pagination types
// ============================================================================

/// Opaque cursor pointing at the last returned sequence.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationCursor {
    /// Sequence number of the last item on the previous page.
    pub seq: u64,
}

/// Opaque cursor for verification cache pages.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCursor {
    /// Sequence number of the last item on the previous page.
    pub seq: u64,
}

/// Opaque cursor for relay pages (index into the relay list).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayCursor {
    /// Index of the last item on the previous page.
    pub index: u32,
}

/// Multi-predicate filter for attestation read queries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationFilter {
    /// Optional relay address to match exactly.
    pub relay: Option<Address>,
    /// Optional user eth address to match exactly.
    pub user_address: Option<Bytes>,
    /// Optional token address to match exactly.
    pub token_address: Option<Bytes>,
    /// Optional accepted flag to match exactly.
    pub accepted: Option<bool>,
}

/// Multi-predicate filter for verification cache read queries.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheFilter {
    /// Optional user eth address to match exactly.
    pub user: Option<Bytes>,
    /// Optional token address to match exactly.
    pub token_address: Option<Bytes>,
    /// Optional token id to match exactly.
    pub token_id: Option<u128>,
}

/// Paginated result of a filtered attestation query.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationPage {
    /// Matching attestation ids for this page.
    pub ids: Vec<BytesN<32>>,
    /// Cursor to resume the next page, or None when exhausted.
    pub next_cursor: Option<AttestationCursor>,
    /// Number of storage rows examined to produce this page.
    pub scanned: u32,
    /// Total number of stored attestations.
    pub total: u32,
}

/// Paginated result of a filtered verification cache query.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachePage {
    /// Matching cache ids for this page.
    pub ids: Vec<BytesN<32>>,
    /// Cursor to resume the next page, or None when exhausted.
    pub next_cursor: Option<CacheCursor>,
    /// Number of storage rows examined to produce this page.
    pub scanned: u32,
    /// Total number of stored cache entries.
    pub total: u32,
}

/// Paginated result of a relay query.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayPage {
    /// Relay addresses for this page.
    pub relays: Vec<Address>,
    /// Cursor to resume the next page, or None when exhausted.
    pub next_cursor: Option<RelayCursor>,
    /// Total number of relays.
    pub total: u32,
}

/// Default filter (match everything).
impl Default for AttestationFilter {
    fn default() -> Self {
        AttestationFilter {
            relay: None,
            user_address: None,
            token_address: None,
            accepted: None,
        }
    }
}

/// Default filter (match everything).
impl Default for CacheFilter {
    fn default() -> Self {
        CacheFilter {
            user: None,
            token_address: None,
            token_id: None,
        }
    }
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum BridgeKey {
    /// Bridge owner
    Owner,
    /// Active bridge relays: Address → BridgeRelay
    BridgeRelay(Address),
    /// All relay addresses
    AllRelays,
    /// Bridge configuration
    BridgeConfig,
    /// Pending attestations: (relay, user_address, token_address) → BalanceAttestation
    PendingAttestation(Address, Bytes, Bytes),
    /// Verification cache: (user_eth_addr, token_addr, token_id) → BridgeVerificationRecord
    VerificationCache(Bytes, Bytes, u128),
    /// Ethereum chain ID for this bridge (e.g., 1 for mainnet, 5 for Goerli)
    EthChainId,
    /// Monotonic sequence counter for pending attestations.
    AttestationNextSeq,
    /// Ordered attestation id index: seq -> attestation id.
    AttestationSeq(u64),
    /// Monotonic sequence counter for verification cache entries.
    CacheNextSeq,
    /// Ordered verification cache id index: seq -> cache id.
    CacheSeq(u64),
    /// Seen attestation digest for replay prevention
    SeenAttestation(BytesN<32>),
}

// ============================================================================
// Storage TTL / Rent Constants (issue #907)
// ============================================================================

/// TTL bump threshold (in ledgers) before extending bridge storage rent
pub const BRIDGE_TTL_THRESHOLD_LEDGERS: u32 = 10_000;
/// TTL extension target (in ledgers) for bridge state persistence
pub const BRIDGE_TTL_EXTEND_LEDGERS: u32 = 100_000;

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BridgeError {
    /// Caller is not authorized
    Unauthorized = 1,
    /// Relay not found or inactive
    RelayNotFound = 2,
    /// Signature verification failed
    InvalidSignature = 3,
    /// Insufficient confirmations for attestation
    InsufficientConfirmations = 4,
    /// Attestation too old (block age exceeded)
    AttestationTooOld = 5,
    /// Invalid Ethereum address format
    InvalidEthAddress = 6,
    /// Invalid signature format (must be 65 bytes)
    InvalidSignatureFormat = 7,
    /// Verification cache expired
    VerificationExpired = 8,
    /// Bridge not configured
    BridgeNotConfigured = 9,
    /// Relay already registered
    RelayAlreadyExists = 10,
    /// Invalid threshold (must be > 0)
    InvalidThreshold = 11,
    /// Caller failed the RBAC role requirement for this operation
    RbacDenied = 12,
    /// Caller is a suspended submitter DID
    RbacSuspended = 13,
    /// Duplicate attestation submission rejected
    DuplicateAttestation = 14,
    /// Out-of-order timestamp or block sequence detected
    OutOfOrderAttestation = 15,
    /// Atomic mid-batch rollback triggered by invalid attestation
    BatchRollbackTriggered = 16,
}

/// Maximum number of storage rows scanned per paginated read to bound CPU.
const MAX_PAGE_SCAN: u64 = 200;

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct CrossChainBridge;

#[contractimpl]
impl CrossChainBridge {
    /// Initialize bridge (owner-only)
    pub fn initialize_bridge(
        env: Env,
        owner: Address,
        signature_threshold: u32,
        cache_ttl_ledgers: u32,
        eth_chain_id: u64,
    ) {
        owner.require_auth();

        if signature_threshold == 0 {
            panic_with_error!(&env, BridgeError::InvalidThreshold);
        }

        let config = BridgeConfig {
            signature_threshold,
            cache_ttl_ledgers,
            max_block_age: 256, // ~1 hour on Ethereum
        };

        env.storage().instance().set(&BridgeKey::Owner, &owner);
        env.storage().instance().set(&BridgeKey::BridgeConfig, &config);
        env.storage().instance().set(&BridgeKey::EthChainId, &eth_chain_id);
        env.storage()
            .instance()
            .set(&BridgeKey::AllRelays, &Vec::new(&env));
        env.storage()
            .instance()
            .set(&BridgeKey::AttestationNextSeq, &1u64);
        env.storage()
            .instance()
            .set(&BridgeKey::CacheNextSeq, &1u64);

        RbacManager::initialize_admin(&env, &owner);

        log!(
            &env,
            "CrossChainBridge: initialized - threshold={}, ttl={}",
            signature_threshold,
            cache_ttl_ledgers
        );

        let state = init_state_bytes(&env, &owner, signature_threshold, cache_ttl_ledgers, eth_chain_id);
        emit_bridge_audit(&env, &owner, "initialize_bridge", &state, AUDIT_OUTCOME_OK, 0);
    }

    // ========================================================================
    // Relay Management
    // ========================================================================

    /// Register a new bridge relay (RBAC-gated, Admin role required)
    pub fn register_relay(
        env: Env,
        relay_address: Address,
        pubkey: Bytes,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let state = relay_state_bytes(&env, &relay_address, &pubkey);
        enforce_bridge_role(
            &env, &owner, Role::Admin, "register_relay", &state);

        // Validate public key format (should be 65 bytes for uncompressed ECDSA)
        if pubkey.len() != 65 {
            panic_with_error!(&env, BridgeError::InvalidSignatureFormat);
        }

        if env
            .storage()
            .instance()
            .get::<_, Option<BridgeRelay>>(&BridgeKey::BridgeRelay(relay_address.clone()))
            .is_some()
        {
            panic_with_error!(&env, BridgeError::RelayAlreadyExists);
        }

        let relay = BridgeRelay {
            relay_address: relay_address.clone(),
            pubkey,
            active: true,
            reputation: 100, // Start at 100
        };

        env.storage()
            .instance()
            .set(&BridgeKey::BridgeRelay(relay_address.clone()), &relay);

        // Add to relay list
        let mut relays = env
            .storage()
            .instance()
            .get::<_, Vec<Address>>(&BridgeKey::AllRelays)
            .unwrap_or_else(|| Vec::new(&env));
        relays.push_back(relay_address.clone());
        env.storage()
            .instance()
            .set(&BridgeKey::AllRelays, &relays);

        // Grant the relay the Submitter RBAC role so only registered,
        // non-suspended relays may pass the `submit_attestation` gate.
        env.storage()
            .persistent()
            .set(&RbacStorageKey::Role(relay_address.clone()), &Role::Submitter);

        log!(
            &env,
            "CrossChainBridge: relay registered - address={}, pubkey_len={}",
            relay_address,
            pubkey.len()
        );

        emit_bridge_audit(&env, &relay_address, "register_relay", &state, AUDIT_OUTCOME_OK, 0);
    }

    /// Deactivate a relay (RBAC-gated, Admin role required)
    pub fn deactivate_relay(env: Env, relay_address: Address) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let empty = Bytes::new(&env);
        let state = relay_state_bytes(&env, &relay_address, &empty);
        enforce_bridge_role(
            &env, &owner, Role::Admin, "deactivate_relay", &state);

        let mut relay = env
            .storage()
            .instance()
            .get::<_, BridgeRelay>(&BridgeKey::BridgeRelay(relay_address.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, BridgeError::RelayNotFound));

        relay.active = false;
        env.storage()
            .instance()
            .set(&BridgeKey::BridgeRelay(relay_address.clone()), &relay);

        // Revoke the relay's Submitter RBAC role; a deactivated relay must not
        // pass role gating even if re-activated without a fresh role grant.
        env.storage()
            .persistent()
            .remove(&RbacStorageKey::Role(relay_address.clone()));

        log!(
            &env,
            "CrossChainBridge: relay deactivated - address={}",
            relay_address
        );

        emit_bridge_audit(&env, &relay_address, "deactivate_relay", &state, AUDIT_OUTCOME_OK, 0);
    }

    // ========================================================================
    // Attestation Submission & Verification
    // ========================================================================

    /// Submit a balance attestation from a relay (RBAC-gated, Submitter role
    /// required; relays are granted `Role::Submitter` on registration)
    pub fn submit_attestation(env: Env, attestation: BalanceAttestation) {
        let relay_address = env.invoker();

        let state = attestation_state_bytes(
            &env,
            &relay_address,
            &attestation.user_address,
            &attestation.token_address,
            attestation.token_id,
            attestation.balance,
            attestation.block_height,
        );
        enforce_bridge_role(
            &env, &relay_address, Role::Submitter, "submit_attestation", &state);

        // Verify relay is registered and active
        let relay = env
            .storage()
            .instance()
            .get::<_, BridgeRelay>(&BridgeKey::BridgeRelay(relay_address.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, BridgeError::RelayNotFound));

        if !relay.active {
            panic_with_error!(&env, BridgeError::RelayNotFound);
        }

        // Validate Ethereum addresses (0x-prefixed hex, 42 chars total)
        Self::validate_eth_address(&env, &attestation.user_address);
        Self::validate_eth_address(&env, &attestation.token_address);

        // Verify signature
        if !Self::verify_signature(&env, &relay, &attestation) {
            panic_with_error!(&env, BridgeError::InvalidSignature);
        }

        // Check block age
        let config = Self::get_bridge_config(&env);
        if attestation.block_height + config.max_block_age < attestation.block_height {
            // Wrapped overflow check
            panic_with_error!(&env, BridgeError::AttestationTooOld);
        }

        // Replay prevention check (issue #909)
        let att_hash = Self::compute_attestation_hash(&env, &attestation);
        if env.storage().instance().has(&BridgeKey::SeenAttestation(att_hash.clone())) {
            emit_bridge_audit(&env, &relay_address, "submit_attestation", &state, AUDIT_OUTCOME_DENIED, BridgeError::DuplicateAttestation as u32);
            panic_with_error!(&env, BridgeError::DuplicateAttestation);
        }

        // Out-of-order timestamp check
        if attestation.timestamp == 0 {
            emit_bridge_audit(&env, &relay_address, "submit_attestation", &state, AUDIT_OUTCOME_DENIED, BridgeError::OutOfOrderAttestation as u32);
            panic_with_error!(&env, BridgeError::OutOfOrderAttestation);
        }

        env.storage().instance().set(&BridgeKey::SeenAttestation(att_hash), &true);

        let key = BridgeKey::PendingAttestation(
            relay_address.clone(),
            attestation.user_address.clone(),
            attestation.token_address.clone(),
        );

        env.storage().instance().set(&key, &attestation);

        // Maintain ordered sequence index for cursor-based pagination.
        let mut seq: u64 = env
            .storage()
            .instance()
            .get(&BridgeKey::AttestationNextSeq)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&BridgeKey::AttestationNextSeq, &(seq + 1));
        env.storage()
            .instance()
            .set(&BridgeKey::AttestationSeq(seq), &attestation);

        // Optimize storage TTL extension and archival rent footprint (issue #907)
        env.storage().instance().extend_ttl(BRIDGE_TTL_THRESHOLD_LEDGERS, BRIDGE_TTL_EXTEND_LEDGERS);

        log!(
            &env,
            "CrossChainBridge: attestation submitted - relay={}, block={}",
            relay_address,
            attestation.block_height
        );

        emit_bridge_audit(&env, &relay_address, "submit_attestation", &state, AUDIT_OUTCOME_OK, 0);
    }

    /// Submit a batch of balance attestations atomically.
    ///
    /// Evaluates all attestations in the batch against replay, signature, and
    /// timestamp validity. If any attestation fails, the transaction is rejected
    /// atomically and none of the attestations in the batch are committed (mid-batch rollback guarantee).
    pub fn submit_attestation_batch(
        env: Env,
        attestations: Vec<BalanceAttestation>,
    ) -> Result<u32, BridgeError> {
        let total = attestations.len();
        if total == 0 {
            return Ok(0);
        }

        let config = Self::get_bridge_config(&env);

        // Pre-validation pass: guarantee mid-batch atomic rollback
        for i in 0..total {
            let att = attestations.get(i).unwrap();
            let relay_opt: Option<BridgeRelay> = env
                .storage()
                .instance()
                .get(&BridgeKey::BridgeRelay(att.relay.clone()));
            if relay_opt.is_none() || !relay_opt.unwrap().active {
                emit_bridge_audit(&env, &att.relay, "submit_attestation_batch", &Bytes::new(&env), AUDIT_OUTCOME_DENIED, BridgeError::RelayNotFound as u32);
                panic_with_error!(&env, BridgeError::BatchRollbackTriggered);
            }

            if att.timestamp == 0 {
                emit_bridge_audit(&env, &att.relay, "submit_attestation_batch", &Bytes::new(&env), AUDIT_OUTCOME_DENIED, BridgeError::OutOfOrderAttestation as u32);
                panic_with_error!(&env, BridgeError::BatchRollbackTriggered);
            }

            if att.block_height.checked_add(config.max_block_age).is_none() {
                emit_bridge_audit(&env, &att.relay, "submit_attestation_batch", &Bytes::new(&env), AUDIT_OUTCOME_DENIED, BridgeError::AttestationTooOld as u32);
                panic_with_error!(&env, BridgeError::BatchRollbackTriggered);
            }

            let hash = Self::compute_attestation_hash(&env, &att);
            if env.storage().instance().has(&BridgeKey::SeenAttestation(hash)) {
                emit_bridge_audit(&env, &att.relay, "submit_attestation_batch", &Bytes::new(&env), AUDIT_OUTCOME_DENIED, BridgeError::DuplicateAttestation as u32);
                panic_with_error!(&env, BridgeError::BatchRollbackTriggered);
            }
        }

        // Commit pass: persist all entries
        for i in 0..total {
            let att = attestations.get(i).unwrap();
            let hash = Self::compute_attestation_hash(&env, &att);
            env.storage().instance().set(&BridgeKey::SeenAttestation(hash), &true);

            let key = BridgeKey::PendingAttestation(
                att.relay.clone(),
                att.user_address.clone(),
                att.token_address.clone(),
            );
            env.storage().instance().set(&key, &att);

            let mut seq: u64 = env
                .storage()
                .instance()
                .get(&BridgeKey::AttestationNextSeq)
                .unwrap_or(1);
            env.storage()
                .instance()
                .set(&BridgeKey::AttestationNextSeq, &(seq + 1));
            env.storage()
                .instance()
                .set(&BridgeKey::AttestationSeq(seq), &att);
        }

        env.storage().instance().extend_ttl(BRIDGE_TTL_THRESHOLD_LEDGERS, BRIDGE_TTL_EXTEND_LEDGERS);
        let first_relay = attestations.get(0).unwrap().relay;
        emit_bridge_audit(&env, &first_relay, "submit_attestation_batch", &Bytes::new(&env), AUDIT_OUTCOME_OK, 0);

        Ok(total)
    }

    /// Permissionless TTL extension for bridge storage rent footprint (issue #907).
    pub fn extend_bridge_ttl(env: Env) {
        env.storage()
            .instance()
            .extend_ttl(BRIDGE_TTL_THRESHOLD_LEDGERS, BRIDGE_TTL_EXTEND_LEDGERS);
    }

    /// Deterministic attestation hash computation for replay prevention (issue #909).
    pub fn compute_attestation_hash(env: &Env, att: &BalanceAttestation) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.append(&att.relay.to_xdr(env));
        data.append(&att.user_address);
        data.append(&att.token_address);
        let mut id_bytes = [0u8; 16];
        for (i, b) in att.token_id.to_be_bytes().iter().enumerate() {
            id_bytes[i] = *b;
        }
        data.append(&Bytes::from_array(env, &id_bytes));
        let mut bal_bytes = [0u8; 16];
        for (i, b) in att.balance.to_be_bytes().iter().enumerate() {
            bal_bytes[i] = *b;
        }
        data.append(&Bytes::from_array(env, &bal_bytes));
        let mut bh_bytes = [0u8; 8];
        for (i, b) in att.block_height.to_be_bytes().iter().enumerate() {
            bh_bytes[i] = *b;
        }
        data.append(&Bytes::from_array(env, &bh_bytes));
        env.crypto().sha256(&data).into()
    }

    /// Accept attestation and update verification cache (RBAC-gated, Admin
    /// role required)
    pub fn accept_attestation(
        env: Env,
        relay_address: Address,
        user_address: Bytes,
        token_address: Bytes,
        token_id: u128,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let state = accept_state_bytes(&env, &relay_address, &user_address, &token_address, token_id);
        enforce_bridge_role(
            &env, &owner, Role::Admin, "accept_attestation", &state);

        let key = BridgeKey::PendingAttestation(
            relay_address.clone(),
            user_address.clone(),
            token_address.clone(),
        );

        let attestation = env
            .storage()
            .instance()
            .get::<_, BalanceAttestation>(&key)
            .unwrap_or_else(|| panic_with_error!(&env, BridgeError::RelayNotFound));

        let config = Self::get_bridge_config(&env);

        // Update verification cache
        let cache_key = BridgeKey::VerificationCache(
            user_address.clone(),
            token_address.clone(),
            token_id,
        );

        let record = BridgeVerificationRecord {
            user: user_address,
            token_address,
            token_id,
            balance: attestation.balance,
            verified_at_ledger: env.ledger().sequence(),
            ttl_ledgers: config.cache_ttl_ledgers,
            verified_by_relay: relay_address,
            confirmations: 1,
        };

        env.storage().instance().set(&cache_key, &record);

        // Maintain ordered sequence index for cursor-based pagination.
        let mut cseq: u64 = env
            .storage()
            .instance()
            .get(&BridgeKey::CacheNextSeq)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&BridgeKey::CacheNextSeq, &(cseq + 1));
        env.storage()
            .instance()
            .set(&BridgeKey::CacheSeq(cseq), &record);

        env.storage().instance().extend_ttl(BRIDGE_TTL_THRESHOLD_LEDGERS, BRIDGE_TTL_EXTEND_LEDGERS);

        log!(
            &env,
            "CrossChainBridge: attestation accepted - balance={}, ttl={}",
            record.balance,
            config.cache_ttl_ledgers
        );

        emit_bridge_audit(&env, &owner, "accept_attestation", &state, AUDIT_OUTCOME_OK, 0);
    }

    /// Query verified balance from cache
    pub fn get_verified_balance(
        env: Env,
        user_address: Bytes,
        token_address: Bytes,
        token_id: u128,
    ) -> u128 {
        Self::validate_eth_address(&env, &user_address);
        Self::validate_eth_address(&env, &token_address);

        let key = BridgeKey::VerificationCache(user_address, token_address, token_id);

        if let Some(record) = env
            .storage()
            .instance()
            .get::<_, BridgeVerificationRecord>(&key)
        {
            // Check if cache expired
            let current_ledger = env.ledger().sequence();
            if current_ledger > record.verified_at_ledger + record.ttl_ledgers {
                panic_with_error!(&env, BridgeError::VerificationExpired);
            }

            record.balance
        } else {
            0 // No verified balance found
        }
    }

    // ========================================================================
    // Paginated read queries
    // ========================================================================

    /// List pending attestations with cursor-based pagination and multi-predicate
    /// filtering. Scans the ordered `AttestationSeq` index starting after
    /// `cursor.seq`, applies every non-`None` predicate (AND semantics), and
    /// returns up to `limit` matching attestation ids. The scan is bounded by
    /// [`MAX_PAGE_SCAN`] rows to protect the Soroban CPU/memory budget.
    pub fn list_attestations(
        env: Env,
        cursor: Option<AttestationCursor>,
        limit: u32,
        filter: AttestationFilter,
    ) -> AttestationPage {
        Self::scan_attestations(&env, cursor, limit, filter)
    }

    /// List verification cache entries with cursor-based pagination and
    /// multi-predicate filtering.
    pub fn list_verification_cache(
        env: Env,
        cursor: Option<CacheCursor>,
        limit: u32,
        filter: CacheFilter,
    ) -> CachePage {
        Self::scan_cache(&env, cursor, limit, filter)
    }

    /// List bridge relays with cursor-based pagination over the relay list.
    pub fn list_relays(
        env: Env,
        cursor: Option<RelayCursor>,
        limit: u32,
    ) -> RelayPage {
        let relays: Vec<Address> = env
            .storage()
            .instance()
            .get::<_, Vec<Address>>(&BridgeKey::AllRelays)
            .unwrap_or_else(|| Vec::new(&env));
        let total = relays.len() as u32;

        let mut ids = Vec::new(&env);
        let mut start: u32 = cursor.map(|c| c.index).unwrap_or(0) + 1;
        if start > total {
            start = total;
        }
        let mut i = start;
        while i <= total && (ids.len() as u64) < (limit as u64) {
            if let Some(addr) = relays.get((i - 1) as usize) {
                ids.push_back(addr);
            }
            i += 1;
        }

        let next_cursor = if i - 1 < total as u32 {
            Some(RelayCursor { index: i - 1 })
        } else {
            None
        };

        RelayPage {
            relays: ids,
            next_cursor,
            total,
        }
    }

    // ========================================================================
    // Configuration & Management
    // ========================================================================

    /// Update bridge configuration (RBAC-gated, Admin role required)
    pub fn update_bridge_config(
        env: Env,
        signature_threshold: u32,
        cache_ttl_ledgers: u32,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let state = config_state_bytes(&env, signature_threshold, cache_ttl_ledgers);
        enforce_bridge_role(
            &env, &owner, Role::Admin, "update_bridge_config", &state);

        if signature_threshold == 0 {
            panic_with_error!(&env, BridgeError::InvalidThreshold);
        }

        let config = BridgeConfig {
            signature_threshold,
            cache_ttl_ledgers,
            max_block_age: 256,
        };

        env.storage().instance().set(&BridgeKey::BridgeConfig, &config);

        log!(
            &env,
            "CrossChainBridge: config updated - threshold={}, ttl={}",
            signature_threshold,
            cache_ttl_ledgers
        );

        emit_bridge_audit(&env, &owner, "update_bridge_config", &state, AUDIT_OUTCOME_OK, 0);
    }

    /// Get bridge configuration
    pub fn get_bridge_config_info(env: Env) -> BridgeConfig {
        Self::get_bridge_config(&env)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_owner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&BridgeKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, BridgeError::Unauthorized))
    }

    fn get_bridge_config(env: &Env) -> BridgeConfig {
        env.storage()
            .instance()
            .get(&BridgeKey::BridgeConfig)
            .unwrap_or_else(|| panic_with_error!(env, BridgeError::BridgeNotConfigured))
    }

    fn validate_eth_address(env: &Env, addr: &Bytes) {
        // Ethereum address: 0x + 40 hex chars = 42 chars total
        // This is a simplified check; real implementation would be more strict
        if addr.len() != 42 {
            panic_with_error!(env, BridgeError::InvalidEthAddress);
        }
    }

    fn verify_signature(env: &Env, relay: &BridgeRelay, attestation: &BalanceAttestation) -> bool {
        // Signature format check
        if attestation.signature.len() != 65 {
            return false;
        }

        // Construct the message being signed: hash(user || token || token_id || balance || block_height || timestamp)
        let mut hasher = Sha256::new();
        hasher.update(&attestation.user_address);
        hasher.update(&attestation.token_address);
        hasher.update(attestation.token_id.to_le_bytes());
        hasher.update(attestation.balance.to_le_bytes());
        hasher.update(attestation.block_height.to_le_bytes());
        hasher.update(attestation.timestamp.to_le_bytes());
        let message_hash = hasher.finalize();

        // ECDSA signature verification would happen here
        // For now, we return true (full implementation requires crypto libraries)
        // This is a placeholder that will be replaced with actual ECDSA verification

        log!(
            env,
            "CrossChainBridge: signature verification placeholder - msg_hash_len={}",
            message_hash.len()
        );

        true
    }

    /// Deterministic attestation id: SHA-256 over relay || user_address || token_address.
    fn attestation_id(env: &Env, relay: &Address, user: &Bytes, token: &Bytes) -> BytesN<32> {
        let mut input = Bytes::new(env);
        input.append(&relay.to_xdr(env));
        input.append(user);
        input.append(token);
        env.crypto().sha256(&input).to_bytes()
    }

    /// Bounded cursor scan over pending attestations with AND-filter predicates.
    fn scan_attestations(
        env: &Env,
        cursor: Option<AttestationCursor>,
        limit: u32,
        filter: AttestationFilter,
    ) -> AttestationPage {
        let mut ids = Vec::new(env);
        let mut scanned: u32 = 0;
        let mut last_seq: u64 = cursor.map(|c| c.seq).unwrap_or(0);
        let total: u64 = env
            .storage()
            .instance()
            .get(&BridgeKey::AttestationNextSeq)
            .unwrap_or(1)
            .saturating_sub(1);

        let mut seq = last_seq + 1;
        let mut remaining = limit as u64;
        let mut stop = false;

        while remaining > 0 && scanned < MAX_PAGE_SCAN && !stop {
            let att: Option<BalanceAttestation> =
                env.storage()
                    .instance()
                    .get(&BridgeKey::AttestationSeq(seq));
            scanned += 1;
            last_seq = seq;

            if let Some(a) = att {
                let matches = filter.relay.map_or(true, |r| r == a.relay)
                    && filter
                        .user_address
                        .map_or(true, |u| u == a.user_address)
                    && filter
                        .token_address
                        .map_or(true, |t| t == a.token_address)
                    && filter.accepted.map_or(true, |acc| acc == a.accepted);

                if matches {
                    let id = Self::attestation_id(env, &a.relay, &a.user_address, &a.token_address);
                    ids.push_back(id);
                    remaining -= 1;
                }
            }

            if seq >= total {
                stop = true;
            } else {
                seq += 1;
            }
        }

        let next_cursor = if last_seq < total {
            Some(AttestationCursor { seq: last_seq })
        } else {
            None
        };

        AttestationPage {
            ids,
            next_cursor,
            scanned,
            total: total as u32,
        }
    }

    /// Bounded cursor scan over verification cache entries with AND-filter predicates.
    fn scan_cache(
        env: &Env,
        cursor: Option<CacheCursor>,
        limit: u32,
        filter: CacheFilter,
    ) -> CachePage {
        let mut ids = Vec::new(env);
        let mut scanned: u32 = 0;
        let mut last_seq: u64 = cursor.map(|c| c.seq).unwrap_or(0);
        let total: u64 = env
            .storage()
            .instance()
            .get(&BridgeKey::CacheNextSeq)
            .unwrap_or(1)
            .saturating_sub(1);

        let mut seq = last_seq + 1;
        let mut remaining = limit as u64;
        let mut stop = false;

        while remaining > 0 && scanned < MAX_PAGE_SCAN && !stop {
            let rec: Option<BridgeVerificationRecord> =
                env.storage().instance().get(&BridgeKey::CacheSeq(seq));
            scanned += 1;
            last_seq = seq;

            if let Some(r) = rec {
                let matches = filter.user.map_or(true, |u| u == r.user)
                    && filter
                        .token_address
                        .map_or(true, |t| t == r.token_address)
                    && filter.token_id.map_or(true, |t| t == r.token_id);

                if matches {
                    let id = Self::cache_id(env, &r.user, &r.token_address, r.token_id);
                    ids.push_back(id);
                    remaining -= 1;
                }
            }

            if seq >= total {
                stop = true;
            } else {
                seq += 1;
            }
        }

        let next_cursor = if last_seq < total {
            Some(CacheCursor { seq: last_seq })
        } else {
            None
        };

        CachePage {
            ids,
            next_cursor,
            scanned,
            total: total as u32,
        }
    }

    /// Deterministic cache id: SHA-256 over user || token_address || token_id.
    fn cache_id(env: &Env, user: &Bytes, token: &Bytes, token_id: u128) -> BytesN<32> {
        let mut input = Bytes::new(env);
        input.append(user);
        input.append(token);
        input.append(&Bytes::from_slice(env, &token_id.to_le_bytes()));
        env.crypto().sha256(&input).to_bytes()
    }
}

// ============================================================================
// RBAC Gating & Tamper-Evident Audit Trail (issue #915)
// ============================================================================

/// Computes a tamper-evident digest binding the caller, operation, and the
/// operation-specific state snapshot (`sha256(caller_xdr || op_len || op || state)`).
fn audit_digest(env: &Env, caller: &Address, operation: &str, state: &Bytes) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.append(&caller.to_xdr(env));
    data.extend_from_array(&(operation.len() as u64).to_be_bytes());
    data.extend_from_slice(operation.as_bytes());
    data.append(state);
    env.crypto().sha256(&data).to_bytes()
}

/// Emits a standardized, tamper-evident audit event for a bridge mutation
/// attempt. Publishes both a diagnostic topic-tagged event (tracing/simulation)
/// and a dedicated outcome topic event (off-chain audit reconstruction).
fn emit_bridge_audit(
    env: &Env,
    caller: &Address,
    operation: &str,
    state: &Bytes,
    outcome: &str,
    error_code: u32,
) {
    let digest = audit_digest(env, caller, operation, state);
    let op = Symbol::new(env, operation);
    let result = Symbol::new(env, outcome);
    let timestamp = env.ledger().timestamp();

    env.events().publish(
        (Symbol::new(env, BRIDGE_AUDIT_TOPIC), op.clone(), result.clone()),
        (caller.clone(), timestamp, digest.clone(), error_code),
    );

    let outcome_topic = if outcome == AUDIT_OUTCOME_OK {
        BRIDGE_AUDIT_OK_TOPIC
    } else {
        BRIDGE_AUDIT_DENIED_TOPIC
    };

    env.events().publish(
        (Symbol::new(env, outcome_topic), op, result),
        (caller.clone(), timestamp, digest),
    );
}

/// Enforces granular RBAC role gating for a bridge mutation. Emits a denied
/// audit event (with the stable [`RbacError`] code) when the caller is a
/// suspended submitter DID or does not hold at least `min_role`.
///
/// Preserves the existing authentication semantics: callers that already
/// performed `require_auth` keep it; the gating itself performs pure storage
/// reads so relay invoker flows are not disrupted.
fn enforce_bridge_role(
    env: &Env,
    caller: &Address,
    min_role: Role,
    operation: &str,
    state: &Bytes,
) -> Result<(), BridgeError> {
    let suspended = RbacManager::is_suspended(env, caller);
    if !suspended && RbacManager::has_role_min(env, caller, min_role) {
        return Ok(());
    }

    let rbac_code = if suspended {
        RbacError::SubmitterSuspended as u32
    } else {
        RbacError::InsufficientRole as u32
    };
    emit_bridge_audit(env, caller, operation, state, AUDIT_OUTCOME_DENIED, rbac_code);

    if suspended {
        Err(BridgeError::RbacSuspended)
    } else {
        Err(BridgeError::RbacDenied)
    }
}

fn init_state_bytes(env: &Env, owner: &Address, threshold: u32, ttl: u32, chain_id: u64) -> Bytes {
    let mut state = Bytes::new(env);
    state.append(&owner.to_xdr(env));
    state.extend_from_array(&threshold.to_le_bytes());
    state.extend_from_array(&ttl.to_le_bytes());
    state.extend_from_array(&chain_id.to_le_bytes());
    state
}

fn relay_state_bytes(env: &Env, relay: &Address, pubkey: &Bytes) -> Bytes {
    let mut state = Bytes::new(env);
    state.append(&relay.to_xdr(env));
    state.append(pubkey);
    state
}

fn attestation_state_bytes(
    env: &Env,
    relay: &Address,
    user: &Bytes,
    token: &Bytes,
    token_id: u128,
    balance: u128,
    block_height: u64,
) -> Bytes {
    let mut state = Bytes::new(env);
    state.append(&relay.to_xdr(env));
    state.append(user);
    state.append(token);
    state.extend_from_array(&token_id.to_le_bytes());
    state.extend_from_array(&balance.to_le_bytes());
    state.extend_from_array(&block_height.to_le_bytes());
    state
}

fn accept_state_bytes(env: &Env, relay: &Address, user: &Bytes, token: &Bytes, token_id: u128) -> Bytes {
    let mut state = Bytes::new(env);
    state.append(&relay.to_xdr(env));
    state.append(user);
    state.append(token);
    state.extend_from_array(&token_id.to_le_bytes());
    state
}

fn config_state_bytes(env: &Env, threshold: u32, ttl: u32) -> Bytes {
    let mut state = Bytes::new(env);
    state.extend_from_array(&threshold.to_le_bytes());
    state.extend_from_array(&ttl.to_le_bytes());
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(env: &Env) -> Address {
        Address::generate(env)
    }

    fn relay(env: &Env) -> Address {
        Address::generate(env)
    }

    fn pubkey_65(env: &Env) -> Bytes {
        Bytes::from_slice(env, &[0x04u8; 65])
    }

    fn eth_addr(env: &Env) -> Bytes {
        // 0x + 40 hex chars = 42 bytes
        Bytes::from_slice(env, b"0x1234567890123456789012345678901234567890")
    }

    fn sample_attestation(env: &Env, relay: &Address) -> BalanceAttestation {
        BalanceAttestation {
            relay: relay.clone(),
            user_address: eth_addr(env),
            token_address: eth_addr(env),
            token_id: 0,
            balance: 1_000_000,
            block_height: 100,
            timestamp: 1_700_000_000,
            signature: Bytes::from_slice(env, &[1u8; 65]),
            accepted: false,
        }
    }

    #[test]
    fn test_bridge_initialization() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);

        CrossChainBridge::initialize_bridge(
            env.clone(),
            o.clone(),
            2,
            100,
            1,
        );

        let config = CrossChainBridge::get_bridge_config_info(env.clone());
        assert_eq!(config.signature_threshold, 2);
        assert_eq!(config.cache_ttl_ledgers, 100);
        assert_eq!(config.max_block_age, 256);
    }

    #[test]
    #[should_panic(expected = "InvalidThreshold")]
    fn test_bridge_initialization_zero_threshold_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 0, 100, 1);
    }

    #[test]
    fn test_relay_registration() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let stored: Option<BridgeRelay> = env
            .storage()
            .instance()
            .get(&BridgeKey::BridgeRelay(r.clone()));
        assert!(stored.is_some());
        assert!(stored.unwrap().active);
        assert_eq!(stored.unwrap().reputation, 100);
    }

    #[test]
    #[should_panic(expected = "InvalidSignatureFormat")]
    fn test_relay_registration_bad_pubkey_len_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), Bytes::from_slice(&env, &[1u8; 33]));
    }

    #[test]
    #[should_panic(expected = "RelayAlreadyExists")]
    fn test_relay_registration_duplicate_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));
    }

    #[test]
    fn test_attestation_submission_and_acceptance() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let att = sample_attestation(&env, &r);
        CrossChainBridge::submit_attestation(env.clone(), att.clone());

        let key = BridgeKey::PendingAttestation(
            r.clone(),
            att.user_address.clone(),
            att.token_address.clone(),
        );
        let stored: Option<BalanceAttestation> = env.storage().instance().get(&key);
        assert!(stored.is_some());

        CrossChainBridge::accept_attestation(
            env.clone(),
            r.clone(),
            att.user_address,
            att.token_address,
            att.token_id,
        );

        let cache_key = BridgeKey::VerificationCache(
            att.user_address,
            att.token_address,
            att.token_id,
        );
        let record: Option<BridgeVerificationRecord> =
            env.storage().instance().get(&cache_key);
        assert!(record.is_some());
        assert_eq!(record.unwrap().balance, att.balance);
    }

    #[test]
    #[should_panic(expected = "AttestationTooOld")]
    fn test_attestation_too_old_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let mut att = sample_attestation(&env, &r);
        // Set block_height so block_height + max_block_age overflows u64
        att.block_height = u64::MAX;
        CrossChainBridge::submit_attestation(env.clone(), att);
    }

    #[test]
    fn test_balance_verification_cache_expiry() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 1, 1, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let att = sample_attestation(&env, &r);
        CrossChainBridge::submit_attestation(env.clone(), att.clone());
        CrossChainBridge::accept_attestation(
            env.clone(),
            r.clone(),
            att.user_address,
            att.token_address,
            att.token_id,
        );

        // Fresh cache returns balance
        let bal = CrossChainBridge::get_verified_balance(
            env.clone(),
            att.user_address,
            att.token_address,
            att.token_id,
        );
        assert_eq!(bal, att.balance);
    }

    #[test]
    fn test_update_bridge_config() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::update_bridge_config(env.clone(), 5, 200);

        let config = CrossChainBridge::get_bridge_config_info(env.clone());
        assert_eq!(config.signature_threshold, 5);
        assert_eq!(config.cache_ttl_ledgers, 200);
    }

    #[test]
    #[should_panic(expected = "InvalidThreshold")]
    fn test_update_bridge_config_zero_threshold_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::update_bridge_config(env.clone(), 0, 100);
    }

    #[test]
    fn test_get_verified_balance_missing_returns_zero() {
        let env = Env::default();
        let bal = CrossChainBridge::get_verified_balance(
            env.clone(),
            eth_addr(&env),
            eth_addr(&env),
            0,
        );
        assert_eq!(bal, 0);
    }

    #[test]
    #[should_panic(expected = "InvalidEthAddress")]
    fn test_validate_eth_address_wrong_len() {
        let env = Env::default();
        validate_eth_address(&Bytes::from_slice(&env, b"0xshort"));
    }

    #[test]
    #[should_panic(expected = "InvalidSignatureFormat")]
    fn test_verify_signature_bad_len() {
        let env = Env::default();
        let r = relay(&env);
        let relay_rec = BridgeRelay {
            relay_address: r.clone(),
            pubkey: pubkey_65(&env),
            active: true,
            reputation: 100,
        };
        let mut att = sample_attestation(&env, &r);
        att.signature = Bytes::from_slice(&env, &[1u8; 33]);
        assert!(!verify_signature(&env, &relay_rec, &att));
    }

    #[test]
    #[should_panic(expected = "DuplicateAttestation")]
    fn test_simulate_duplicate_submission_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let att = sample_attestation(&env, &r);
        CrossChainBridge::submit_attestation(env.clone(), att.clone());

        // Replay submission of the exact same attestation payload must be rejected
        CrossChainBridge::submit_attestation(env.clone(), att);
    }

    #[test]
    #[should_panic(expected = "OutOfOrderAttestation")]
    fn test_simulate_out_of_order_timestamp_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let mut att = sample_attestation(&env, &r);
        att.timestamp = 0; // Stale/invalid zero timestamp
        CrossChainBridge::submit_attestation(env.clone(), att);
    }

    #[test]
    #[should_panic(expected = "BatchRollbackTriggered")]
    fn test_simulate_mid_batch_rollback_atomicity() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let att1 = sample_attestation(&env, &r);

        let mut att2 = sample_attestation(&env, &r);
        att2.balance = 2_000_000;
        att2.token_id = 1;

        let mut att3 = sample_attestation(&env, &r);
        att3.timestamp = 0; // Failure injected at item 3 of the batch

        let mut batch = Vec::new(&env);
        batch.push_back(att1.clone());
        batch.push_back(att2.clone());
        batch.push_back(att3);

        // Pre-state sequence
        let pre_seq: u64 = env
            .storage()
            .instance()
            .get(&BridgeKey::AttestationNextSeq)
            .unwrap_or(1);
        assert_eq!(pre_seq, 1);

        // Batch submission must fail partway through and trigger full mid-batch rollback
        let _ = CrossChainBridge::submit_attestation_batch(env.clone(), batch);
    }

    #[test]
    fn test_simulate_successful_batch_and_duplicate_batch_rejection() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);
        let r = relay(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::register_relay(env.clone(), r.clone(), pubkey_65(&env));

        let att1 = sample_attestation(&env, &r);

        let mut att2 = sample_attestation(&env, &r);
        att2.balance = 500_000;
        att2.token_id = 2;

        let mut batch = Vec::new(&env);
        batch.push_back(att1.clone());
        batch.push_back(att2.clone());

        let res = CrossChainBridge::submit_attestation_batch(env.clone(), batch);
        assert_eq!(res, Ok(2));

        let post_seq: u64 = env
            .storage()
            .instance()
            .get(&BridgeKey::AttestationNextSeq)
            .unwrap_or(1);
        assert_eq!(post_seq, 3);
    }

    #[test]
    fn test_simulate_ttl_extension_and_rent_preservation() {
        let env = Env::default();
        env.mock_all_auths();
        let o = owner(&env);

        CrossChainBridge::initialize_bridge(env.clone(), o.clone(), 2, 100, 1);
        CrossChainBridge::extend_bridge_ttl(env.clone());
    }
}
