//! NFT Provenance Tracking Module
//!
//! Comprehensive NFT tracking with creator registration, ownership history,
//! royalty payments, license enforcement, fractionalization, and cross-chain support.

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Error codes for NFT provenance operations
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum NFTProvenanceError {
    /// NFT not found
    NFTNotFound = 1,
    /// Creator not registered
    CreatorNotFound = 2,
    /// Invalid royalty rate
    InvalidRoyaltyRate = 3,
    /// Royalty payment failed
    RoyaltyPaymentFailed = 4,
    /// License not found
    LicenseNotFound = 5,
    /// License expired
    LicenseExpired = 6,
    /// Unauthorized license usage
    UnauthorizedLicense = 7,
    /// Fractionalization failed
    FractionalizationFailed = 8,
    /// Insufficient fractions
    InsufficientFractions = 9,
    /// Invalid ownership transfer
    InvalidOwnershipTransfer = 10,
    /// Cross-chain transfer failed
    CrossChainTransferFailed = 11,
}

/// NFT metadata structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NFTMetadata {
    /// NFT ID (unique identifier)
    pub nft_id: BytesN<32>,
    /// NFT name
    pub name: Bytes,
    /// NFT description
    pub description: Bytes,
    /// NFT URI (metadata location)
    pub uri: Bytes,
    /// Current owner
    pub current_owner: Address,
    /// Original creator
    pub creator: Address,
    /// Minting timestamp
    pub minted_at: u64,
    /// Token standard (ERC-721, ERC-1155, etc.)
    pub token_standard: Symbol,
    /// Chain identifier
    pub chain: Symbol,
}

/// Creator profile
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatorProfile {
    /// Creator address
    pub creator: Address,
    /// Creator name/handle
    pub name: Bytes,
    /// Creator bio/description
    pub bio: Bytes,
    /// Creator website/social
    pub website: Bytes,
    /// Total NFTs created
    pub total_nfts: u32,
    /// Total royalties earned (in smallest units)
    pub total_royalties: u128,
    /// Registration timestamp
    pub registered_at: u64,
    /// Creator verification status
    pub verified: bool,
}

/// Ownership history entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipRecord {
    /// Record ID
    pub record_id: BytesN<32>,
    /// NFT ID
    pub nft_id: BytesN<32>,
    /// Previous owner
    pub previous_owner: Address,
    /// New owner
    pub new_owner: Address,
    /// Transfer price (0 if gift)
    pub transfer_price: u128,
    /// Transfer timestamp
    pub transferred_at: u64,
    /// Transfer type: 0=sale, 1=gift, 2=inheritance, 3=other
    pub transfer_type: u32,
}

/// Royalty configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyConfig {
    /// NFT ID
    pub nft_id: BytesN<32>,
    /// Creator address
    pub creator: Address,
    /// Royalty rate in basis points (0-10000)
    pub royalty_rate_bp: u32,
    /// Recipient of royalties (may differ from creator)
    pub royalty_recipient: Address,
    /// Optional maximum supply for royalty calculation
    pub max_supply: Option<u128>,
    /// Configuration timestamp
    pub configured_at: u64,
}

/// Royalty payment record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoyaltyPayment {
    /// Payment ID
    pub payment_id: BytesN<32>,
    /// NFT ID
    pub nft_id: BytesN<32>,
    /// From address (seller/marketplace)
    pub from: Address,
    /// To address (royalty recipient)
    pub to: Address,
    /// Amount paid
    pub amount: u128,
    /// Sale price (for calculation reference)
    pub sale_price: u128,
    /// Calculated royalty rate
    pub royalty_rate_bp: u32,
    /// Payment timestamp
    pub paid_at: u64,
    /// Chain identifier
    pub chain: Symbol,
}

/// NFT license
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NFTLicense {
    /// License ID
    pub license_id: BytesN<32>,
    /// NFT ID
    pub nft_id: BytesN<32>,
    /// License holder
    pub licensee: Address,
    /// License type: 0=personal, 1=commercial, 2=exclusive, 3=derivative
    pub license_type: u32,
    /// Rights granted (bitmask)
    pub rights: u32,
    /// Can sublicense
    pub can_sublicense: bool,
    /// Expiration timestamp
    pub expires_at: u64,
    /// License grant timestamp
    pub granted_at: u64,
    /// License restrictions (hash)
    pub restrictions_hash: BytesN<32>,
}

/// Rights bitmask constants
pub mod Rights {
    pub const VIEW: u32 = 1;          // View/display
    pub const COPY: u32 = 2;          // Make copies
    pub const DISTRIBUTE: u32 = 4;    // Distribute
    pub const MODIFY: u32 = 8;        // Create derivatives
    pub const COMMERCIAL: u32 = 16;   // Commercial use
    pub const SUBLICENSE: u32 = 32;   // Grant sublicenses
}

/// Fractionalized NFT (F-NFT)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalNFT {
    /// F-NFT ID
    pub fnft_id: BytesN<32>,
    /// Original NFT ID
    pub original_nft_id: BytesN<32>,
    /// Total fractions
    pub total_fractions: u128,
    /// Fraction decimals (e.g., 18 for standard ERC-20)
    pub fraction_decimals: u32,
    /// Fraction token name
    pub fraction_name: Bytes,
    /// Fraction token symbol
    pub fraction_symbol: Bytes,
    /// Fractionalization date
    pub fractionalized_at: u64,
    /// Can be redeemed for original
    pub redeemable: bool,
    /// Redemption price per fraction
    pub redemption_price: u128,
}

/// Fraction ownership
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionOwnership {
    /// Ownership ID
    pub ownership_id: BytesN<32>,
    /// F-NFT ID
    pub fnft_id: BytesN<32>,
    /// Owner address
    pub owner: Address,
    /// Fraction balance
    pub balance: u128,
    /// Acquisition timestamp
    pub acquired_at: u64,
}

/// Cross-chain royalty record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossChainRoyalty {
    /// Record ID
    pub record_id: BytesN<32>,
    /// NFT ID (original chain)
    pub nft_id: BytesN<32>,
    /// Source chain
    pub source_chain: Symbol,
    /// Destination chain
    pub destination_chain: Symbol,
    /// Royalty amount due
    pub amount: u128,
    /// Status: 0=pending, 1=locked, 2=transferred
    pub status: u32,
    /// Created timestamp
    pub created_at: u64,
    /// Completed timestamp
    pub completed_at: u64,
}

/// Storage key enumeration
#[derive(Clone)]
#[contracttype]
pub enum NFTProvenanceDataKey {
    /// NFT ID → NFTMetadata
    NFTMetadata(BytesN<32>),
    /// Creator address → CreatorProfile
    CreatorProfile(Address),
    /// NFT ID → RoyaltyConfig
    RoyaltyConfig(BytesN<32>),
    /// Payment ID → RoyaltyPayment
    RoyaltyPayment(BytesN<32>),
    /// License ID → NFTLicense
    NFTLicense(BytesN<32>),
    /// F-NFT ID → FractionalNFT
    FractionalNFT(BytesN<32>),
    /// Ownership ID → FractionOwnership
    FractionOwnership(BytesN<32>),
    /// Record ID → CrossChainRoyalty
    CrossChainRoyalty(BytesN<32>),
    /// Ownership record ID → OwnershipRecord
    OwnershipRecord(BytesN<32>),
    /// NFT ID → List of ownership records
    NFTOwnershipHistory(BytesN<32>),
    /// Creator → List of NFT IDs
    CreatorNFTList(Address),
    /// Owner → List of NFT IDs
    OwnerNFTList(Address),
    /// NFT ID → List of licenses
    NFTLicenseList(BytesN<32>),
    /// F-NFT ID → List of fraction owners
    FractionOwnerList(BytesN<32>),
    /// Creator NFT counter
    CreatorNFTCount(Address),
    /// Owner NFT counter
    OwnerNFTCount(Address),
    /// Total NFT counter
    TotalNFTCount,
    /// Total creator counter
    CreatorCount,
    /// Total payment counter
    RoyaltyPaymentCount,
    /// Total license counter
    LicenseCount,
    /// Total fractionalization counter
    FractionalizationCount,
}

/// NFT provenance contract trait
pub trait NFTProvenanceTrait {
    // ==================== CREATOR MANAGEMENT ====================
    fn register_creator(
        env: Env,
        creator: Address,
        name: Bytes,
        bio: Bytes,
        website: Bytes,
    ) -> Result<(), NFTProvenanceError>;

    fn get_creator(env: Env, creator: Address) -> Result<CreatorProfile, NFTProvenanceError>;

    fn verify_creator(env: Env, creator: Address) -> Result<(), NFTProvenanceError>;

    // ==================== NFT REGISTRATION ====================
    fn mint_nft(
        env: Env,
        creator: Address,
        name: Bytes,
        description: Bytes,
        uri: Bytes,
        token_standard: Symbol,
        chain: Symbol,
    ) -> Result<BytesN<32>, NFTProvenanceError>;

    fn get_nft(env: Env, nft_id: BytesN<32>) -> Result<NFTMetadata, NFTProvenanceError>;

    // ==================== OWNERSHIP TRACKING ====================
    fn transfer_ownership(
        env: Env,
        nft_id: BytesN<32>,
        new_owner: Address,
        transfer_price: u128,
        transfer_type: u32,
    ) -> Result<BytesN<32>, NFTProvenanceError>;

    fn get_ownership_history(
        env: Env,
        nft_id: BytesN<32>,
    ) -> Result<u32, NFTProvenanceError>;

    fn get_ownership_record(
        env: Env,
        record_id: BytesN<32>,
    ) -> Result<OwnershipRecord, NFTProvenanceError>;

    // ==================== ROYALTY MANAGEMENT ====================
    fn set_royalty_config(
        env: Env,
        nft_id: BytesN<32>,
        royalty_rate_bp: u32,
        royalty_recipient: Address,
    ) -> Result<(), NFTProvenanceError>;

    fn pay_royalty(
        env: Env,
        nft_id: BytesN<32>,
        from: Address,
        sale_price: u128,
        chain: Symbol,
    ) -> Result<BytesN<32>, NFTProvenanceError>;

    fn get_royalty_config(
        env: Env,
        nft_id: BytesN<32>,
    ) -> Result<RoyaltyConfig, NFTProvenanceError>;

    fn calculate_royalty(
        env: Env,
        nft_id: BytesN<32>,
        sale_price: u128,
    ) -> Result<u128, NFTProvenanceError>;

    // ==================== LICENSE MANAGEMENT ====================
    fn grant_license(
        env: Env,
        nft_id: BytesN<32>,
        licensee: Address,
        license_type: u32,
        rights: u32,
        duration_seconds: u64,
        restrictions_hash: BytesN<32>,
    ) -> Result<BytesN<32>, NFTProvenanceError>;

    fn revoke_license(env: Env, license_id: BytesN<32>) -> Result<(), NFTProvenanceError>;

    fn verify_license(
        env: Env,
        license_id: BytesN<32>,
        required_rights: u32,
    ) -> Result<bool, NFTProvenanceError>;

    fn get_license(env: Env, license_id: BytesN<32>) -> Result<NFTLicense, NFTProvenanceError>;

    // ==================== FRACTIONALIZATION ====================
    fn fractionalize_nft(
        env: Env,
        nft_id: BytesN<32>,
        total_fractions: u128,
        fraction_decimals: u32,
        fraction_name: Bytes,
        fraction_symbol: Bytes,
        redeemable: bool,
        redemption_price: u128,
    ) -> Result<BytesN<32>, NFTProvenanceError>;

    fn transfer_fraction(
        env: Env,
        fnft_id: BytesN<32>,
        from: Address,
        to: Address,
        amount: u128,
    ) -> Result<(), NFTProvenanceError>;

    fn redeem_fractions(
        env: Env,
        fnft_id: BytesN<32>,
        fraction_owner: Address,
        amount: u128,
    ) -> Result<(), NFTProvenanceError>;

    fn get_fractional_nft(
        env: Env,
        fnft_id: BytesN<32>,
    ) -> Result<FractionalNFT, NFTProvenanceError>;

    fn get_fraction_balance(
        env: Env,
        fnft_id: BytesN<32>,
        owner: Address,
    ) -> Result<u128, NFTProvenanceError>;

    // ==================== CROSS-CHAIN ====================
    fn initiate_cross_chain_royalty(
        env: Env,
        nft_id: BytesN<32>,
        destination_chain: Symbol,
        amount: u128,
    ) -> Result<BytesN<32>, NFTProvenanceError>;

    fn complete_cross_chain_royalty(
        env: Env,
        record_id: BytesN<32>,
    ) -> Result<(), NFTProvenanceError>;

    fn get_cross_chain_royalty(
        env: Env,
        record_id: BytesN<32>,
    ) -> Result<CrossChainRoyalty, NFTProvenanceError>;

    // ==================== QUERY FUNCTIONS ====================
    fn total_nft_count(env: Env) -> u32;

    fn creator_nft_count(env: Env, creator: Address) -> u32;

    fn owner_nft_count(env: Env, owner: Address) -> u32;

    fn total_creator_count(env: Env) -> u32;

    fn total_royalty_payments(env: Env) -> u32;

    fn creator_total_royalties(env: Env, creator: Address) -> u128;
}

/// Helper function to compute hash from data
pub fn compute_hash(data: &Bytes) -> BytesN<32> {
    let env = Env::new();
    env.crypto().sha256(data)
}

/// Helper to validate royalty rate (0-10000 basis points = 0-100%)
pub fn validate_royalty_rate(rate_bp: u32) -> bool {
    rate_bp <= 10000u32
}

/// Helper to calculate royalty amount
pub fn calculate_royalty_amount(sale_price: u128, rate_bp: u32) -> u128 {
    (sale_price * rate_bp as u128) / 10000u128
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_royalty_calculation() {
        let sale_price = 1000u128;
        let rate_bp = 500u32; // 5%
        let royalty = calculate_royalty_amount(sale_price, rate_bp);
        assert_eq!(royalty, 50u128);
    }

    #[test]
    fn test_royalty_validation() {
        assert!(validate_royalty_rate(0u32));
        assert!(validate_royalty_rate(5000u32));
        assert!(validate_royalty_rate(10000u32));
        assert!(!validate_royalty_rate(10001u32));
    }
}
