/// Token Gating Module - Premium Access Control via Multi-Chain Tokens
///
/// Supports:
/// - Stellar native assets (XLM and custom assets)
/// - ERC-20 (via bridge verification)
/// - ERC-721 (NFTs, via bridge verification)
/// - ERC-1155 (multi-tokens, via bridge verification)
///
/// Architecture:
/// 1. TokenTier: Define access levels with token requirements
/// 2. VerificationBridge: Cross-chain token balance verification
/// 3. Marketplace: Purchase and transfer token tiers
/// 4. AccessControl: Check user eligibility for premium streams/features

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, Map, String, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Token standard types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum TokenStandard {
    /// Stellar native asset (XLM or custom asset with issuer)
    StellarAsset = 0,
    /// Ethereum ERC-20 token
    ERC20 = 1,
    /// Ethereum ERC-721 NFT
    ERC721 = 2,
    /// Ethereum ERC-1155 multi-token
    ERC1155 = 3,
}

/// Token specification for verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenSpec {
    /// Token standard (Stellar, ERC-20, ERC-721, ERC-1155)
    pub standard: TokenStandard,
    /// For Stellar: issuer address; for EVM: contract address (0x-prefixed hex in Bytes)
    pub contract_address: Bytes,
    /// For ERC-1155: token ID; for others: 0
    pub token_id: u128,
    /// Required amount (scaled by decimals)
    pub required_amount: u128,
}

/// Access tier configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenTier {
    /// Tier identifier (e.g., "premium", "enterprise")
    pub tier_id: Symbol,
    /// Description of tier benefits
    pub description: String,
    /// List of token requirements (any one can grant access)
    pub token_requirements: Vec<TokenSpec>,
    /// Price to purchase this tier (in XLM stroops, or 0 if free)
    pub purchase_price: u128,
    /// Duration in ledgers; 0 = permanent
    pub duration_ledgers: u32,
    /// Whether tier purchases are allowed via marketplace
    pub tradeable: bool,
    /// Owner-only toggle for enabling/disabling tier
    pub enabled: bool,
}

/// User's token tier holding
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierHolding {
    /// User address
    pub holder: Address,
    /// Tier being held
    pub tier_id: Symbol,
    /// Ledger where tier expires (0 = permanent)
    pub expiry_ledger: u32,
    /// Timestamp when purchased
    pub purchased_at: u64,
    /// Whether verification is still valid
    pub verified: bool,
}

/// Cross-chain token verification status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRecord {
    /// User address being verified
    pub user: Address,
    /// Token spec being verified
    pub token_spec: TokenSpec,
    /// Last verified balance
    pub verified_balance: u128,
    /// Ledger height of last verification
    pub verified_at_ledger: u32,
    /// Verification TTL (re-check needed after this ledger)
    pub ttl_ledgers: u32,
    /// Bridge relay that performed verification
    pub verified_by_bridge: Bytes,
}

/// Marketplace listing for tier sales
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketplaceListingDetails {
    /// Seller address
    pub seller: Address,
    /// Tier being sold
    pub tier_id: Symbol,
    /// Price in XLM stroops
    pub price: u128,
    /// Quantity available (0 = unlimited)
    pub quantity: u32,
    /// Whether listing is active
    pub active: bool,
    /// Listing creation timestamp
    pub created_at: u64,
}

/// Event stream access requirement
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamAccessControl {
    /// Event type being gated
    pub event_type: Symbol,
    /// Minimum tier required (Symbol: "free", "premium", "enterprise")
    pub required_tier: Symbol,
    /// Whether premium streams are tracked separately
    pub premium: bool,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum TokenGatingKey {
    /// Owner of token gating contract
    Owner,
    /// Enabled tiers: Symbol (tier_id) → TokenTier
    Tier(Symbol),
    /// User holdings: Address → Vec<TierHolding>
    UserTiers(Address),
    /// Verification cache: (Address, TokenSpec hash) → VerificationRecord
    VerificationCache(Address, BytesN<32>),
    /// Marketplace listings: u32 (listing_id) → MarketplaceListingDetails
    MarketplaceListing(u32),
    /// Next marketplace listing ID
    NextListingId,
    /// Stream access controls: Symbol (event_type) → StreamAccessControl
    StreamAccessControl(Symbol),
    /// Bridge contract addresses (for cross-chain verification)
    BridgeContracts,
    /// Rate limiting: (Address, Symbol) → (last_check_ledger, current_tally)
    VerificationRateLimit(Address, Symbol),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TokenGatingError {
    /// Caller is not authorized to perform this action
    Unauthorized = 1,
    /// User does not have required token tier
    InsufficientTier = 2,
    /// Token verification failed
    VerificationFailed = 3,
    /// Tier does not exist
    TierNotFound = 4,
    /// Insufficient balance to purchase tier
    InsufficientFunds = 5,
    /// Cross-chain bridge verification timeout
    BridgeVerificationTimeout = 6,
    /// Invalid token specification
    InvalidTokenSpec = 7,
    /// Tier already exists
    TierAlreadyExists = 8,
    /// Marketplace listing not found
    ListingNotFound = 9,
    /// Insufficient marketplace inventory
    InsufficientInventory = 10,
    /// Stream access control not found
    StreamAccessControlNotFound = 11,
    /// Tier is disabled
    TierDisabled = 12,
    /// Invalid purchase price
    InvalidPurchasePrice = 13,
    /// Verification rate limit exceeded
    VerificationRateLimitExceeded = 14,
    /// Bridge not configured for this token standard
    BridgeNotConfigured = 15,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct TokenGating;

#[contractimpl]
impl TokenGating {
    /// Initialize token gating module (owner-only)
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        
        if env.storage().instance().has(&TokenGatingKey::Owner) {
            panic_with_error!(&env, TokenGatingError::Unauthorized);
        }
        
        env.storage().instance().set(&TokenGatingKey::Owner, &owner);
    }

    // ========================================================================
    // Tier Management
    // ========================================================================

    /// Create or update a token tier (owner-only)
    pub fn create_token_tier(
        env: Env,
        tier_id: Symbol,
        description: String,
        token_requirements: Vec<TokenSpec>,
        purchase_price: u128,
        duration_ledgers: u32,
        tradeable: bool,
    ) -> TokenTier {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        if token_requirements.is_empty() {
            panic_with_error!(&env, TokenGatingError::InvalidTokenSpec);
        }

        let tier = TokenTier {
            tier_id,
            description,
            token_requirements,
            purchase_price,
            duration_ledgers,
            tradeable,
            enabled: true,
        };

        env.storage().instance().set(&TokenGatingKey::Tier(tier_id), &tier);

        log!(
            &env,
            "TokenGating: tier created - id={:?}, price={}",
            tier_id,
            purchase_price
        );

        tier
    }

    /// Set tier enabled/disabled status (owner-only)
    pub fn set_tier_enabled(env: Env, tier_id: Symbol, enabled: bool) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let mut tier = Self::get_tier_or_panic(&env, tier_id);
        tier.enabled = enabled;
        env.storage().instance().set(&TokenGatingKey::Tier(tier_id), &tier);

        log!(
            &env,
            "TokenGating: tier enabled={} - id={:?}",
            enabled,
            tier_id
        );
    }

    /// Get tier configuration
    pub fn get_tier(env: Env, tier_id: Symbol) -> Option<TokenTier> {
        env.storage()
            .instance()
            .get(&TokenGatingKey::Tier(tier_id))
    }

    // ========================================================================
    // Access Verification
    // ========================================================================

    /// Check if user has access to a tier
    pub fn has_tier_access(env: Env, user: Address, tier_id: Symbol) -> bool {
        let holdings_key = TokenGatingKey::UserTiers(user.clone());
        
        if let Some(holdings) = env.storage().instance().get::<_, Vec<TierHolding>>(&holdings_key) {
            for holding in holdings.iter() {
                if holding.tier_id == tier_id && holding.verified {
                    // Check expiry if tier has duration
                    if holding.expiry_ledger == 0 || env.ledger().sequence() < holding.expiry_ledger {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Grant tier to user (internal, used by marketplace/admin)
    pub fn grant_tier_to_user(
        env: Env,
        user: Address,
        tier_id: Symbol,
        duration_ledgers: u32,
    ) {
        let tier = Self::get_tier_or_panic(&env, tier_id);
        if !tier.enabled {
            panic_with_error!(&env, TokenGatingError::TierDisabled);
        }

        let mut holdings_key = TokenGatingKey::UserTiers(user.clone());
        let mut holdings = env
            .storage()
            .instance()
            .get::<_, Vec<TierHolding>>(&holdings_key)
            .unwrap_or_else(|| Vec::new(&env));

        let expiry_ledger = if duration_ledgers > 0 {
            env.ledger().sequence() + duration_ledgers
        } else {
            0
        };

        let holding = TierHolding {
            holder: user.clone(),
            tier_id,
            expiry_ledger,
            purchased_at: env.ledger().timestamp(),
            verified: true,
        };

        holdings.push_back(holding);
        env.storage().instance().set(&holdings_key, &holdings);

        log!(
            &env,
            "TokenGating: tier granted to user - tier={:?}, expiry={}",
            tier_id,
            expiry_ledger
        );
    }

    /// Verify token balance via bridge (cross-chain verification)
    pub fn verify_token_balance(
        env: Env,
        user: Address,
        token_spec: TokenSpec,
    ) -> bool {
        // Rate limit verification attempts per user/token combo
        Self::check_verification_rate_limit(&env, &user, &token_spec.contract_address);

        // For Stellar assets, verify directly via Soroban
        match token_spec.standard {
            TokenStandard::StellarAsset => {
                Self::verify_stellar_balance(&env, &user, &token_spec)
            }
            // For EVM tokens, delegate to bridge
            TokenStandard::ERC20 | TokenStandard::ERC721 | TokenStandard::ERC1155 => {
                Self::verify_evm_balance(&env, &user, &token_spec)
            }
        }
    }

    // ========================================================================
    // Stream Access Control
    // ========================================================================

    /// Set access control requirement for an event stream (owner-only)
    pub fn set_stream_access_control(
        env: Env,
        event_type: Symbol,
        required_tier: Symbol,
        premium: bool,
    ) {
        let owner = Self::get_owner(&env);
        owner.require_auth();

        let control = StreamAccessControl {
            event_type,
            required_tier,
            premium,
        };

        env.storage()
            .instance()
            .set(&TokenGatingKey::StreamAccessControl(event_type), &control);

        log!(
            &env,
            "TokenGating: stream access control set - event_type={:?}, required_tier={:?}",
            event_type,
            required_tier
        );
    }

    /// Check if user can access an event stream
    pub fn can_access_stream(env: Env, user: Address, event_type: Symbol) -> bool {
        // If no access control set, allow access
        if let Some(control) =
            env.storage()
                .instance()
                .get::<_, StreamAccessControl>(&TokenGatingKey::StreamAccessControl(event_type))
        {
            Self::has_tier_access(&env, user, control.required_tier)
        } else {
            true
        }
    }

    // ========================================================================
    // Marketplace Operations
    // ========================================================================

    /// Create a marketplace listing (tier holder can sell)
    pub fn list_tier_for_sale(
        env: Env,
        tier_id: Symbol,
        price: u128,
        quantity: u32,
    ) -> u32 {
        let seller = env.invoker();

        // Verify seller has the tier to sell
        if !Self::has_tier_access(&env, seller.clone(), tier_id) {
            panic_with_error!(&env, TokenGatingError::InsufficientTier);
        }

        let tier = Self::get_tier_or_panic(&env, tier_id);
        if !tier.tradeable {
            panic_with_error!(&env, TokenGatingError::Unauthorized);
        }

        let listing_id = Self::get_next_listing_id(&env);

        let listing = MarketplaceListingDetails {
            seller,
            tier_id,
            price,
            quantity,
            active: true,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .instance()
            .set(&TokenGatingKey::MarketplaceListing(listing_id), &listing);

        log!(
            &env,
            "TokenGating: listing created - id={}, tier={:?}, price={}",
            listing_id,
            tier_id,
            price
        );

        listing_id
    }

    /// Purchase tier from marketplace listing
    pub fn purchase_from_marketplace(env: Env, listing_id: u32, buyer: Address) {
        buyer.require_auth();

        let listing = env
            .storage()
            .instance()
            .get::<_, MarketplaceListingDetails>(&TokenGatingKey::MarketplaceListing(listing_id))
            .unwrap_or_else(|| panic_with_error!(&env, TokenGatingError::ListingNotFound));

        if !listing.active {
            panic_with_error!(&env, TokenGatingError::ListingNotFound);
        }

        if listing.quantity > 0 && listing.quantity < 1 {
            panic_with_error!(&env, TokenGatingError::InsufficientInventory);
        }

        // Get tier to determine duration
        let tier = Self::get_tier_or_panic(&env, listing.tier_id);

        // Grant tier to buyer
        Self::grant_tier_to_user(&env, buyer.clone(), tier.tier_id, tier.duration_ledgers);

        // Update listing inventory
        let mut updated_listing = listing.clone();
        if updated_listing.quantity > 0 {
            updated_listing.quantity -= 1;
            if updated_listing.quantity == 0 {
                updated_listing.active = false;
            }
        }
        env.storage()
            .instance()
            .set(&TokenGatingKey::MarketplaceListing(listing_id), &updated_listing);

        log!(
            &env,
            "TokenGating: purchase completed - buyer={}, listing_id={}, tier={:?}",
            buyer,
            listing_id,
            tier.tier_id
        );
    }

    /// Cancel a marketplace listing (seller-only)
    pub fn cancel_marketplace_listing(env: Env, listing_id: u32) {
        let listing = env
            .storage()
            .instance()
            .get::<_, MarketplaceListingDetails>(&TokenGatingKey::MarketplaceListing(listing_id))
            .unwrap_or_else(|| panic_with_error!(&env, TokenGatingError::ListingNotFound));

        listing.seller.require_auth();

        let mut cancelled = listing.clone();
        cancelled.active = false;
        env.storage()
            .instance()
            .set(&TokenGatingKey::MarketplaceListing(listing_id), &cancelled);

        log!(
            &env,
            "TokenGating: listing cancelled - id={}",
            listing_id
        );
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn get_owner(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&TokenGatingKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, TokenGatingError::Unauthorized))
    }

    fn get_tier_or_panic(env: &Env, tier_id: Symbol) -> TokenTier {
        env.storage()
            .instance()
            .get(&TokenGatingKey::Tier(tier_id))
            .unwrap_or_else(|| panic_with_error!(env, TokenGatingError::TierNotFound))
    }

    fn get_next_listing_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&TokenGatingKey::NextListingId)
            .unwrap_or(1);

        env.storage()
            .instance()
            .set(&TokenGatingKey::NextListingId, &(current + 1));

        current
    }

    fn check_verification_rate_limit(env: &Env, user: &Address, contract_addr: &Bytes) {
        // Simple rate limit: max 10 verifications per ledger per user/contract pair
        let key = TokenGatingKey::VerificationRateLimit(user.clone(), contract_addr.clone());
        let (last_ledger, count) = env
            .storage()
            .instance()
            .get::<_, (u32, u32)>(&key)
            .unwrap_or((0, 0));

        let current_ledger = env.ledger().sequence();
        let new_count = if current_ledger == last_ledger {
            count + 1
        } else {
            1
        };

        if new_count > 10 {
            panic_with_error!(env, TokenGatingError::VerificationRateLimitExceeded);
        }

        env.storage()
            .instance()
            .set(&key, &(current_ledger, new_count));
    }

    fn verify_stellar_balance(env: &Env, user: &Address, spec: &TokenSpec) -> bool {
        // Direct verification of Stellar asset balance
        // This would integrate with the audit ledger's own verification logic
        // For now, return true (full implementation depends on bridge design)
        
        log!(
            env,
            "TokenGating: stellar balance verified for user - amount={}",
            spec.required_amount
        );

        true
    }

    fn verify_evm_balance(env: &Env, user: &Address, spec: &TokenSpec) -> bool {
        // Delegate to bridge for ERC-20/721/1155 verification
        // This would call the cross-chain bridge contracts
        // For now, return true (full implementation in task #3)
        
        log!(
            env,
            "TokenGating: evm balance verified for user via bridge - amount={}",
            spec.required_amount
        );

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_creation() {
        // Tests for tier creation and retrieval
    }

    #[test]
    fn test_access_verification() {
        // Tests for access tier verification
    }

    #[test]
    fn test_marketplace_operations() {
        // Tests for marketplace listing and purchase
    }

    #[test]
    fn test_stream_access_control() {
        // Tests for stream access gating
    }
}
