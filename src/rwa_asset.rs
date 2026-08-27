//! RWA Asset Registry — token creation, approval, suspension, and redemption.
//!
//! The `AssetRegistry` struct provides all lifecycle-management functions for
//! real-world asset tokens: `register`, `submit_for_approval`, `approve`,
//! `reject`, `suspend`, `reactivate`, `begin_redemption`, and `mature`.
#![no_std]

use crate::rwa_types::{
    AssetClass, ComplianceFramework, RwaConfig, RwaToken, TokenizationStatus,
};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

// ── Storage Key ───────────────────────────────────────────────────────────────

/// Storage keys used by the asset registry within the host contract.
#[contracttype]
#[derive(Clone)]
pub enum AssetRegistryKey {
    /// Global RWA configuration.
    Config,
    /// Token record keyed by token ID.
    Token(BytesN<32>),
    /// Ordered list of token IDs (packed, each 32 bytes).
    TokenIndex,
    /// Index of tokens per issuer address.
    IssuerTokens(Address),
    /// Index of tokens per asset class (u8).
    ClassTokens(u8),
    /// Total registered token count cache.
    TokenCount,
}

// ── Asset Registry ────────────────────────────────────────────────────────────

/// Core asset registry manager.
pub struct AssetRegistry;

impl AssetRegistry {
    // ── Configuration ─────────────────────────────────────────────────────────

    /// Initialise the registry with provided config (call once on contract init).
    pub fn initialize(env: &Env, config: RwaConfig) {
        env.storage()
            .instance()
            .set(&AssetRegistryKey::Config, &config);
    }

    /// Read the current global configuration.
    pub fn get_config(env: &Env) -> RwaConfig {
        env.storage()
            .instance()
            .get(&AssetRegistryKey::Config)
            .unwrap_or_else(RwaConfig::default_config)
    }

    /// Persist an updated configuration (owner-only guard must be applied by caller).
    pub fn update_config(env: &Env, config: &RwaConfig) {
        env.storage()
            .instance()
            .set(&AssetRegistryKey::Config, config);
    }

    // ── Token Registration ────────────────────────────────────────────────────

    /// Register a new RWA token in `Draft` state.
    ///
    /// Returns the token or an error string.
    pub fn register_token(
        env: &Env,
        issuer: Address,
        external_id: Bytes,
        name: Bytes,
        asset_class: AssetClass,
        compliance_framework: ComplianceFramework,
        total_supply: u128,
        metadata: Bytes,
    ) -> Result<RwaToken, &'static str> {
        let mut config = Self::get_config(env);

        if !config.can_register_token() {
            return Err("Registry is paused or token cap reached");
        }
        if name.is_empty() {
            return Err("Token name must not be empty");
        }
        if external_id.is_empty() {
            return Err("External identifier must not be empty");
        }
        if total_supply == 0 {
            return Err("Total supply must be greater than zero");
        }
        if metadata.len() > config.max_metadata_size {
            return Err("Metadata exceeds maximum allowed size");
        }

        let token_id = Self::compute_token_id(env, &issuer, &external_id);

        // Reject duplicate registrations.
        if env
            .storage()
            .instance()
            .has(&AssetRegistryKey::Token(token_id.clone()))
        {
            return Err("Token with this ID already registered");
        }

        let genesis_hash = BytesN::from_array(env, &[0u8; 32]);
        let now = env.ledger().timestamp();

        let token = RwaToken::new(
            token_id.clone(),
            external_id,
            name,
            asset_class,
            issuer.clone(),
            compliance_framework,
            total_supply,
            metadata,
            now,
            genesis_hash,
        );

        // Persist the token record.
        env.storage()
            .instance()
            .set(&AssetRegistryKey::Token(token_id.clone()), &token);

        // Update the ordered index.
        let mut index: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&AssetRegistryKey::TokenIndex)
            .unwrap_or_else(|| Vec::new(env));
        index.push_back(token_id.clone());
        env.storage()
            .instance()
            .set(&AssetRegistryKey::TokenIndex, &index);

        // Update per-issuer index.
        let mut issuer_tokens: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&AssetRegistryKey::IssuerTokens(issuer.clone()))
            .unwrap_or_else(|| Vec::new(env));
        issuer_tokens.push_back(token_id.clone());
        env.storage()
            .instance()
            .set(&AssetRegistryKey::IssuerTokens(issuer), &issuer_tokens);

        // Update per-class index.
        let class_key = AssetRegistryKey::ClassTokens(asset_class as u8);
        let mut class_tokens: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&class_key)
            .unwrap_or_else(|| Vec::new(env));
        class_tokens.push_back(token_id.clone());
        env.storage().instance().set(&class_key, &class_tokens);

        // Bump count.
        config.token_count += 1;
        Self::update_config(env, &config);

        // Emit a Soroban event for off-chain indexers.
        env.events().publish(
            (Symbol::new(env, "rwa_asset"), Symbol::new(env, "registered")),
            (token_id, asset_class as u8),
        );

        Ok(token)
    }

    // ── Lifecycle Transitions ─────────────────────────────────────────────────

    /// Submit a Draft token for regulatory approval.
    pub fn submit_for_approval(
        env: &Env,
        token_id: &BytesN<32>,
        caller: &Address,
    ) -> Result<RwaToken, &'static str> {
        let mut token = Self::get_token(env, token_id)?;
        let current = TokenizationStatus::from_u8(token.status)
            .ok_or("Invalid token status")?;

        if token.issuer != *caller {
            return Err("Only the issuer can submit for approval");
        }
        if current != TokenizationStatus::Draft {
            return Err("Token must be in Draft state to submit for approval");
        }

        token.status = TokenizationStatus::PendingApproval as u8;
        token.updated_at = env.ledger().timestamp();
        Self::save_token(env, &token);

        env.events().publish(
            (Symbol::new(env, "rwa_asset"), Symbol::new(env, "submitted")),
            token_id.clone(),
        );

        Ok(token)
    }

    /// Approve a pending token (governance / compliance officer).
    pub fn approve_token(
        env: &Env,
        token_id: &BytesN<32>,
    ) -> Result<RwaToken, &'static str> {
        let mut token = Self::get_token(env, token_id)?;
        let current = TokenizationStatus::from_u8(token.status)
            .ok_or("Invalid token status")?;

        if current != TokenizationStatus::PendingApproval {
            return Err("Token must be PendingApproval to be approved");
        }

        token.status = TokenizationStatus::Active as u8;
        token.updated_at = env.ledger().timestamp();
        Self::save_token(env, &token);

        env.events().publish(
            (Symbol::new(env, "rwa_asset"), Symbol::new(env, "approved")),
            token_id.clone(),
        );

        Ok(token)
    }

    /// Reject a pending token.
    pub fn reject_token(
        env: &Env,
        token_id: &BytesN<32>,
        reason: Bytes,
    ) -> Result<RwaToken, &'static str> {
        let mut token = Self::get_token(env, token_id)?;
        let current = TokenizationStatus::from_u8(token.status)
            .ok_or("Invalid token status")?;

        if current != TokenizationStatus::PendingApproval {
            return Err("Token must be PendingApproval to be rejected");
        }

        token.status = TokenizationStatus::Rejected as u8;
        token.updated_at = env.ledger().timestamp();
        token.metadata = reason.clone();
        Self::save_token(env, &token);

        env.events().publish(
            (Symbol::new(env, "rwa_asset"), Symbol::new(env, "rejected")),
            (token_id.clone(), reason),
        );

        Ok(token)
    }

    /// Suspend an active token (regulatory hold or compliance issue).
    pub fn suspend_token(
        env: &Env,
        token_id: &BytesN<32>,
        reason: Bytes,
    ) -> Result<RwaToken, &'static str> {
        let mut token = Self::get_token(env, token_id)?;
        let current = TokenizationStatus::from_u8(token.status)
            .ok_or("Invalid token status")?;

        if current != TokenizationStatus::Active {
            return Err("Only Active tokens can be suspended");
        }

        token.status = TokenizationStatus::Suspended as u8;
        token.updated_at = env.ledger().timestamp();
        Self::save_token(env, &token);

        env.events().publish(
            (Symbol::new(env, "rwa_asset"), Symbol::new(env, "suspended")),
            (token_id.clone(), reason),
        );

        Ok(token)
    }

    /// Reactivate a suspended token after the hold is lifted.
    pub fn reactivate_token(
        env: &Env,
        token_id: &BytesN<32>,
    ) -> Result<RwaToken, &'static str> {
        let mut token = Self::get_token(env, token_id)?;
        let current = TokenizationStatus::from_u8(token.status)
            .ok_or("Invalid token status")?;

        if current != TokenizationStatus::Suspended {
            return Err("Only Suspended tokens can be reactivated");
        }

        token.status = TokenizationStatus::Active as u8;
        token.updated_at = env.ledger().timestamp();
        Self::save_token(env, &token);

        env.events().publish(
            (
                Symbol::new(env, "rwa_asset"),
                Symbol::new(env, "reactivated"),
            ),
            token_id.clone(),
        );

        Ok(token)
    }

    /// Open the redemption window for an active token.
    pub fn begin_redemption(
        env: &Env,
        token_id: &BytesN<32>,
    ) -> Result<RwaToken, &'static str> {
        let mut token = Self::get_token(env, token_id)?;
        let current = TokenizationStatus::from_u8(token.status)
            .ok_or("Invalid token status")?;

        if current != TokenizationStatus::Active {
            return Err("Only Active tokens can enter Redeeming state");
        }

        token.status = TokenizationStatus::Redeeming as u8;
        token.updated_at = env.ledger().timestamp();
        Self::save_token(env, &token);

        env.events().publish(
            (
                Symbol::new(env, "rwa_asset"),
                Symbol::new(env, "redeeming"),
            ),
            token_id.clone(),
        );

        Ok(token)
    }

    /// Mark a fully redeemed token as matured.
    pub fn mature_token(
        env: &Env,
        token_id: &BytesN<32>,
    ) -> Result<RwaToken, &'static str> {
        let mut token = Self::get_token(env, token_id)?;
        let current = TokenizationStatus::from_u8(token.status)
            .ok_or("Invalid token status")?;

        if current != TokenizationStatus::Redeeming {
            return Err("Token must be in Redeeming state to be matured");
        }

        token.status = TokenizationStatus::Matured as u8;
        token.updated_at = env.ledger().timestamp();
        Self::save_token(env, &token);

        env.events().publish(
            (Symbol::new(env, "rwa_asset"), Symbol::new(env, "matured")),
            token_id.clone(),
        );

        Ok(token)
    }

    // ── Token Queries ─────────────────────────────────────────────────────────

    /// Retrieve a token record by ID, returning an error if not found.
    pub fn get_token(env: &Env, token_id: &BytesN<32>) -> Result<RwaToken, &'static str> {
        env.storage()
            .instance()
            .get(&AssetRegistryKey::Token(token_id.clone()))
            .ok_or("Token not found")
    }

    /// Return all registered token IDs in registration order.
    pub fn list_tokens(env: &Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&AssetRegistryKey::TokenIndex)
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Return token IDs registered by a specific issuer.
    pub fn list_tokens_by_issuer(env: &Env, issuer: &Address) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&AssetRegistryKey::IssuerTokens(issuer.clone()))
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Return token IDs belonging to a specific asset class.
    pub fn list_tokens_by_class(env: &Env, class: AssetClass) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&AssetRegistryKey::ClassTokens(class as u8))
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Total number of registered tokens.
    pub fn token_count(env: &Env) -> u32 {
        Self::get_config(env).token_count
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Persist an updated token record.
    fn save_token(env: &Env, token: &RwaToken) {
        env.storage()
            .instance()
            .set(&AssetRegistryKey::Token(token.token_id.clone()), token);
    }

    /// Derive a deterministic token ID.
    pub fn compute_token_id(env: &Env, issuer: &Address, external_id: &Bytes) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, issuer.to_xdr().as_ref()));
        input.append(external_id);
        input.append(&Bytes::from_slice(
            env,
            &env.ledger().timestamp().to_le_bytes(),
        ));
        sha256(&input)
    }
}
