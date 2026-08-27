//! NFT Provenance Contract Implementation
//!
//! Full implementation of NFT tracking including creator registration, ownership history,
//! royalty payments, license enforcement, fractionalization, and cross-chain support.

use crate::nft_provenance::*;
use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

/// Contract implementation for NFT provenance
pub struct NFTProvenanceContract;

#[contractimpl]
impl NFTProvenanceTrait for NFTProvenanceContract {
    // ==================== CREATOR MANAGEMENT ====================

    fn register_creator(
        env: Env,
        creator: Address,
        name: Bytes,
        bio: Bytes,
        website: Bytes,
    ) -> Result<(), NFTProvenanceError> {
        // Check if already registered
        if storage_get_creator(&env, &creator).is_some() {
            return Ok(());
        }

        let profile = CreatorProfile {
            creator: creator.clone(),
            name,
            bio,
            website,
            total_nfts: 0u32,
            total_royalties: 0u128,
            registered_at: env.ledger().timestamp(),
            verified: false,
        };

        storage_set_creator(&env, &creator, &profile);

        // Update creator count
        let count = storage_get_creator_count(&env);
        storage_set_creator_count(&env, count + 1);

        Ok(())
    }

    fn get_creator(env: Env, creator: Address) -> Result<CreatorProfile, NFTProvenanceError> {
        storage_get_creator(&env, &creator).ok_or(NFTProvenanceError::CreatorNotFound)
    }

    fn verify_creator(env: Env, creator: Address) -> Result<(), NFTProvenanceError> {
        let mut profile = storage_get_creator(&env, &creator)
            .ok_or(NFTProvenanceError::CreatorNotFound)?;

        profile.verified = true;
        storage_set_creator(&env, &creator, &profile);

        Ok(())
    }

    // ==================== NFT REGISTRATION ====================

    fn mint_nft(
        env: Env,
        creator: Address,
        name: Bytes,
        description: Bytes,
        uri: Bytes,
        token_standard: Symbol,
        chain: Symbol,
    ) -> Result<BytesN<32>, NFTProvenanceError> {
        // Verify creator exists
        let mut creator_profile = storage_get_creator(&env, &creator)
            .ok_or(NFTProvenanceError::CreatorNotFound)?;

        let nft_id = compute_nft_id(&env, &creator);

        let metadata = NFTMetadata {
            nft_id,
            name,
            description,
            uri,
            current_owner: creator.clone(),
            creator: creator.clone(),
            minted_at: env.ledger().timestamp(),
            token_standard,
            chain,
        };

        storage_set_nft_metadata(&env, &nft_id, &metadata);

        // Update creator NFT list
        let mut creator_nfts = storage_get_creator_nft_list(&env, &creator);
        creator_nfts.push_back(nft_id);
        storage_set_creator_nft_list(&env, &creator, &creator_nfts);

        // Update owner NFT list
        let mut owner_nfts = storage_get_owner_nft_list(&env, &creator);
        owner_nfts.push_back(nft_id);
        storage_set_owner_nft_list(&env, &creator, &owner_nfts);

        // Update counters
        creator_profile.total_nfts = creator_profile.total_nfts.saturating_add(1u32);
        storage_set_creator(&env, &creator, &creator_profile);

        let total_count = storage_get_total_nft_count(&env);
        storage_set_total_nft_count(&env, total_count + 1);

        Ok(nft_id)
    }

    fn get_nft(env: Env, nft_id: BytesN<32>) -> Result<NFTMetadata, NFTProvenanceError> {
        storage_get_nft_metadata(&env, &nft_id).ok_or(NFTProvenanceError::NFTNotFound)
    }

    // ==================== OWNERSHIP TRACKING ====================

    fn transfer_ownership(
        env: Env,
        nft_id: BytesN<32>,
        new_owner: Address,
        transfer_price: u128,
        transfer_type: u32,
    ) -> Result<BytesN<32>, NFTProvenanceError> {
        let mut metadata = storage_get_nft_metadata(&env, &nft_id)
            .ok_or(NFTProvenanceError::NFTNotFound)?;

        let previous_owner = metadata.current_owner.clone();

        // Create ownership record
        let record_id = compute_ownership_record_id(&env, &nft_id);
        let record = OwnershipRecord {
            record_id,
            nft_id,
            previous_owner: previous_owner.clone(),
            new_owner: new_owner.clone(),
            transfer_price,
            transferred_at: env.ledger().timestamp(),
            transfer_type,
        };

        storage_set_ownership_record(&env, &record_id, &record);

        // Update NFT ownership history
        let mut history = storage_get_nft_ownership_history(&env, &nft_id);
        history.push_back(record_id);
        storage_set_nft_ownership_history(&env, &nft_id, &history);

        // Update NFT metadata
        metadata.current_owner = new_owner.clone();
        storage_set_nft_metadata(&env, &nft_id, &metadata);

        // Update owner NFT lists
        let mut prev_owner_nfts = storage_get_owner_nft_list(&env, &previous_owner);
        // Remove from previous owner (simplified - keep for history)
        
        let mut new_owner_nfts = storage_get_owner_nft_list(&env, &new_owner);
        new_owner_nfts.push_back(nft_id);
        storage_set_owner_nft_list(&env, &new_owner, &new_owner_nfts);

        // Update ownership counts
        let prev_count = storage_get_owner_nft_count(&env, &previous_owner);
        storage_set_owner_nft_count(&env, &previous_owner, prev_count.saturating_sub(1u32));

        let new_count = storage_get_owner_nft_count(&env, &new_owner);
        storage_set_owner_nft_count(&env, &new_owner, new_count + 1);

        Ok(record_id)
    }

    fn get_ownership_history(
        env: Env,
        nft_id: BytesN<32>,
    ) -> Result<u32, NFTProvenanceError> {
        let _ = storage_get_nft_metadata(&env, &nft_id)
            .ok_or(NFTProvenanceError::NFTNotFound)?;

        Ok(storage_get_nft_ownership_history(&env, &nft_id).len() as u32)
    }

    fn get_ownership_record(
        env: Env,
        record_id: BytesN<32>,
    ) -> Result<OwnershipRecord, NFTProvenanceError> {
        storage_get_ownership_record(&env, &record_id)
            .ok_or(NFTProvenanceError::InvalidOwnershipTransfer)
    }

    // ==================== ROYALTY MANAGEMENT ====================

    fn set_royalty_config(
        env: Env,
        nft_id: BytesN<32>,
        royalty_rate_bp: u32,
        royalty_recipient: Address,
    ) -> Result<(), NFTProvenanceError> {
        // Verify NFT exists
        let metadata = storage_get_nft_metadata(&env, &nft_id)
            .ok_or(NFTProvenanceError::NFTNotFound)?;

        // Validate royalty rate
        if !validate_royalty_rate(royalty_rate_bp) {
            return Err(NFTProvenanceError::InvalidRoyaltyRate);
        }

        let config = RoyaltyConfig {
            nft_id,
            creator: metadata.creator.clone(),
            royalty_rate_bp,
            royalty_recipient,
            max_supply: None,
            configured_at: env.ledger().timestamp(),
        };

        storage_set_royalty_config(&env, &nft_id, &config);

        Ok(())
    }

    fn pay_royalty(
        env: Env,
        nft_id: BytesN<32>,
        from: Address,
        sale_price: u128,
        chain: Symbol,
    ) -> Result<BytesN<32>, NFTProvenanceError> {
        let config = storage_get_royalty_config(&env, &nft_id)
            .ok_or(NFTProvenanceError::InvalidRoyaltyRate)?;

        let royalty_amount = calculate_royalty_amount(sale_price, config.royalty_rate_bp);

        let payment_id = compute_royalty_payment_id(&env, &nft_id);

        let payment = RoyaltyPayment {
            payment_id,
            nft_id,
            from: from.clone(),
            to: config.royalty_recipient.clone(),
            amount: royalty_amount,
            sale_price,
            royalty_rate_bp: config.royalty_rate_bp,
            paid_at: env.ledger().timestamp(),
            chain,
        };

        storage_set_royalty_payment(&env, &payment_id, &payment);

        // Update creator total royalties
        let mut creator_profile = storage_get_creator(&env, &config.creator)
            .ok_or(NFTProvenanceError::CreatorNotFound)?;

        creator_profile.total_royalties = creator_profile.total_royalties.saturating_add(royalty_amount);
        storage_set_creator(&env, &config.creator, &creator_profile);

        // Update payment count
        let count = storage_get_royalty_payment_count(&env);
        storage_set_royalty_payment_count(&env, count + 1);

        Ok(payment_id)
    }

    fn get_royalty_config(
        env: Env,
        nft_id: BytesN<32>,
    ) -> Result<RoyaltyConfig, NFTProvenanceError> {
        storage_get_royalty_config(&env, &nft_id)
            .ok_or(NFTProvenanceError::InvalidRoyaltyRate)
    }

    fn calculate_royalty(
        env: Env,
        nft_id: BytesN<32>,
        sale_price: u128,
    ) -> Result<u128, NFTProvenanceError> {
        let config = storage_get_royalty_config(&env, &nft_id)
            .ok_or(NFTProvenanceError::InvalidRoyaltyRate)?;

        Ok(calculate_royalty_amount(sale_price, config.royalty_rate_bp))
    }

    // ==================== LICENSE MANAGEMENT ====================

    fn grant_license(
        env: Env,
        nft_id: BytesN<32>,
        licensee: Address,
        license_type: u32,
        rights: u32,
        duration_seconds: u64,
        restrictions_hash: BytesN<32>,
    ) -> Result<BytesN<32>, NFTProvenanceError> {
        // Verify NFT exists
        let _ = storage_get_nft_metadata(&env, &nft_id)
            .ok_or(NFTProvenanceError::NFTNotFound)?;

        let license_id = compute_license_id(&env, &nft_id, &licensee);
        let now = env.ledger().timestamp();

        let license = NFTLicense {
            license_id,
            nft_id,
            licensee,
            license_type,
            rights,
            can_sublicense: (rights & Rights::SUBLICENSE) != 0,
            expires_at: now + duration_seconds,
            granted_at: now,
            restrictions_hash,
        };

        storage_set_license(&env, &license_id, &license);

        // Update license list for NFT
        let mut licenses = storage_get_nft_license_list(&env, &nft_id);
        licenses.push_back(license_id);
        storage_set_nft_license_list(&env, &nft_id, &licenses);

        // Update license count
        let count = storage_get_license_count(&env);
        storage_set_license_count(&env, count + 1);

        Ok(license_id)
    }

    fn revoke_license(env: Env, license_id: BytesN<32>) -> Result<(), NFTProvenanceError> {
        let license = storage_get_license(&env, &license_id)
            .ok_or(NFTProvenanceError::LicenseNotFound)?;

        // Mark as expired by setting expiration to now
        let mut updated_license = license;
        updated_license.expires_at = env.ledger().timestamp();

        storage_set_license(&env, &license_id, &updated_license);

        Ok(())
    }

    fn verify_license(
        env: Env,
        license_id: BytesN<32>,
        required_rights: u32,
    ) -> Result<bool, NFTProvenanceError> {
        let license = storage_get_license(&env, &license_id)
            .ok_or(NFTProvenanceError::LicenseNotFound)?;

        let now = env.ledger().timestamp();

        // Check if expired
        if license.expires_at < now {
            return Err(NFTProvenanceError::LicenseExpired);
        }

        // Check if required rights are granted
        let has_rights = (license.rights & required_rights) == required_rights;

        Ok(has_rights)
    }

    fn get_license(env: Env, license_id: BytesN<32>) -> Result<NFTLicense, NFTProvenanceError> {
        storage_get_license(&env, &license_id).ok_or(NFTProvenanceError::LicenseNotFound)
    }

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
    ) -> Result<BytesN<32>, NFTProvenanceError> {
        // Verify NFT exists
        let _ = storage_get_nft_metadata(&env, &nft_id)
            .ok_or(NFTProvenanceError::NFTNotFound)?;

        let fnft_id = compute_fnft_id(&env, &nft_id);

        let fnft = FractionalNFT {
            fnft_id,
            original_nft_id: nft_id,
            total_fractions,
            fraction_decimals,
            fraction_name,
            fraction_symbol,
            fractionalized_at: env.ledger().timestamp(),
            redeemable,
            redemption_price,
        };

        storage_set_fractional_nft(&env, &fnft_id, &fnft);

        // Create initial fraction ownership for original owner
        let metadata = storage_get_nft_metadata(&env, &nft_id)
            .ok_or(NFTProvenanceError::NFTNotFound)?;

        let ownership_id = compute_fraction_ownership_id(&env, &fnft_id, &metadata.current_owner);

        let ownership = FractionOwnership {
            ownership_id,
            fnft_id,
            owner: metadata.current_owner.clone(),
            balance: total_fractions,
            acquired_at: env.ledger().timestamp(),
        };

        storage_set_fraction_ownership(&env, &ownership_id, &ownership);

        // Update fractionalization count
        let count = storage_get_fractionalization_count(&env);
        storage_set_fractionalization_count(&env, count + 1);

        Ok(fnft_id)
    }

    fn transfer_fraction(
        env: Env,
        fnft_id: BytesN<32>,
        from: Address,
        to: Address,
        amount: u128,
    ) -> Result<(), NFTProvenanceError> {
        let fnft = storage_get_fractional_nft(&env, &fnft_id)
            .ok_or(NFTProvenanceError::FractionalizationFailed)?;

        // Get from ownership
        let from_ownership_id = compute_fraction_ownership_id(&env, &fnft_id, &from);
        let mut from_ownership = storage_get_fraction_ownership(&env, &from_ownership_id)
            .ok_or(NFTProvenanceError::InsufficientFractions)?;

        if from_ownership.balance < amount {
            return Err(NFTProvenanceError::InsufficientFractions);
        }

        // Get or create to ownership
        let to_ownership_id = compute_fraction_ownership_id(&env, &fnft_id, &to);
        let mut to_ownership = storage_get_fraction_ownership(&env, &to_ownership_id)
            .unwrap_or_else(|| FractionOwnership {
                ownership_id: to_ownership_id,
                fnft_id,
                owner: to.clone(),
                balance: 0u128,
                acquired_at: env.ledger().timestamp(),
            });

        // Transfer fractions
        from_ownership.balance = from_ownership.balance.saturating_sub(amount);
        to_ownership.balance = to_ownership.balance.saturating_add(amount);

        storage_set_fraction_ownership(&env, &from_ownership_id, &from_ownership);
        storage_set_fraction_ownership(&env, &to_ownership_id, &to_ownership);

        Ok(())
    }

    fn redeem_fractions(
        env: Env,
        fnft_id: BytesN<32>,
        fraction_owner: Address,
        amount: u128,
    ) -> Result<(), NFTProvenanceError> {
        let fnft = storage_get_fractional_nft(&env, &fnft_id)
            .ok_or(NFTProvenanceError::FractionalizationFailed)?;

        if !fnft.redeemable {
            return Err(NFTProvenanceError::FractionalizationFailed);
        }

        // Verify ownership
        let ownership_id = compute_fraction_ownership_id(&env, &fnft_id, &fraction_owner);
        let mut ownership = storage_get_fraction_ownership(&env, &ownership_id)
            .ok_or(NFTProvenanceError::InsufficientFractions)?;

        if ownership.balance < amount {
            return Err(NFTProvenanceError::InsufficientFractions);
        }

        // Deduct fractions
        ownership.balance = ownership.balance.saturating_sub(amount);
        storage_set_fraction_ownership(&env, &ownership_id, &ownership);

        Ok(())
    }

    fn get_fractional_nft(
        env: Env,
        fnft_id: BytesN<32>,
    ) -> Result<FractionalNFT, NFTProvenanceError> {
        storage_get_fractional_nft(&env, &fnft_id)
            .ok_or(NFTProvenanceError::FractionalizationFailed)
    }

    fn get_fraction_balance(
        env: Env,
        fnft_id: BytesN<32>,
        owner: Address,
    ) -> Result<u128, NFTProvenanceError> {
        let ownership_id = compute_fraction_ownership_id(&env, &fnft_id, &owner);
        
        match storage_get_fraction_ownership(&env, &ownership_id) {
            Some(ownership) => Ok(ownership.balance),
            None => Ok(0u128),
        }
    }

    // ==================== CROSS-CHAIN ====================

    fn initiate_cross_chain_royalty(
        env: Env,
        nft_id: BytesN<32>,
        destination_chain: Symbol,
        amount: u128,
    ) -> Result<BytesN<32>, NFTProvenanceError> {
        let metadata = storage_get_nft_metadata(&env, &nft_id)
            .ok_or(NFTProvenanceError::NFTNotFound)?;

        let record_id = compute_cross_chain_royalty_id(&env, &nft_id);

        let record = CrossChainRoyalty {
            record_id,
            nft_id,
            source_chain: metadata.chain,
            destination_chain,
            amount,
            status: 1u32, // locked
            created_at: env.ledger().timestamp(),
            completed_at: 0u64,
        };

        storage_set_cross_chain_royalty(&env, &record_id, &record);

        Ok(record_id)
    }

    fn complete_cross_chain_royalty(
        env: Env,
        record_id: BytesN<32>,
    ) -> Result<(), NFTProvenanceError> {
        let mut record = storage_get_cross_chain_royalty(&env, &record_id)
            .ok_or(NFTProvenanceError::CrossChainTransferFailed)?;

        record.status = 2u32; // transferred
        record.completed_at = env.ledger().timestamp();

        storage_set_cross_chain_royalty(&env, &record_id, &record);

        Ok(())
    }

    fn get_cross_chain_royalty(
        env: Env,
        record_id: BytesN<32>,
    ) -> Result<CrossChainRoyalty, NFTProvenanceError> {
        storage_get_cross_chain_royalty(&env, &record_id)
            .ok_or(NFTProvenanceError::CrossChainTransferFailed)
    }

    // ==================== QUERY FUNCTIONS ====================

    fn total_nft_count(env: Env) -> u32 {
        storage_get_total_nft_count(&env)
    }

    fn creator_nft_count(env: Env, creator: Address) -> u32 {
        storage_get_creator_nft_count(&env, &creator)
    }

    fn owner_nft_count(env: Env, owner: Address) -> u32 {
        storage_get_owner_nft_count(&env, &owner)
    }

    fn total_creator_count(env: Env) -> u32 {
        storage_get_creator_count(&env)
    }

    fn total_royalty_payments(env: Env) -> u32 {
        storage_get_royalty_payment_count(&env)
    }

    fn creator_total_royalties(env: Env, creator: Address) -> u128 {
        match storage_get_creator(&env, &creator) {
            Some(profile) => profile.total_royalties,
            None => 0u128,
        }
    }
}

// ==================== STORAGE HELPERS ====================

// Creator storage helpers
fn storage_get_creator(env: &Env, creator: &Address) -> Option<CreatorProfile> {
    let key = NFTProvenanceDataKey::CreatorProfile(creator.clone());
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_creator(env: &Env, creator: &Address, profile: &CreatorProfile) {
    let key = NFTProvenanceDataKey::CreatorProfile(creator.clone());
    env.storage().persistent().set(&key, profile);
}

// NFT metadata helpers
fn storage_get_nft_metadata(env: &Env, nft_id: &BytesN<32>) -> Option<NFTMetadata> {
    let key = NFTProvenanceDataKey::NFTMetadata(*nft_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_nft_metadata(env: &Env, nft_id: &BytesN<32>, metadata: &NFTMetadata) {
    let key = NFTProvenanceDataKey::NFTMetadata(*nft_id);
    env.storage().persistent().set(&key, metadata);
}

// Ownership history helpers
fn storage_get_ownership_record(env: &Env, record_id: &BytesN<32>) -> Option<OwnershipRecord> {
    let key = NFTProvenanceDataKey::OwnershipRecord(*record_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_ownership_record(env: &Env, record_id: &BytesN<32>, record: &OwnershipRecord) {
    let key = NFTProvenanceDataKey::OwnershipRecord(*record_id);
    env.storage().persistent().set(&key, record);
}

fn storage_get_nft_ownership_history(env: &Env, nft_id: &BytesN<32>) -> Vec<BytesN<32>> {
    let key = NFTProvenanceDataKey::NFTOwnershipHistory(*nft_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_nft_ownership_history(env: &Env, nft_id: &BytesN<32>, history: &Vec<BytesN<32>>) {
    let key = NFTProvenanceDataKey::NFTOwnershipHistory(*nft_id);
    env.storage().persistent().set(&key, history);
}

// Royalty helpers
fn storage_get_royalty_config(env: &Env, nft_id: &BytesN<32>) -> Option<RoyaltyConfig> {
    let key = NFTProvenanceDataKey::RoyaltyConfig(*nft_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_royalty_config(env: &Env, nft_id: &BytesN<32>, config: &RoyaltyConfig) {
    let key = NFTProvenanceDataKey::RoyaltyConfig(*nft_id);
    env.storage().persistent().set(&key, config);
}

fn storage_get_royalty_payment(env: &Env, payment_id: &BytesN<32>) -> Option<RoyaltyPayment> {
    let key = NFTProvenanceDataKey::RoyaltyPayment(*payment_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_royalty_payment(env: &Env, payment_id: &BytesN<32>, payment: &RoyaltyPayment) {
    let key = NFTProvenanceDataKey::RoyaltyPayment(*payment_id);
    env.storage().persistent().set(&key, payment);
}

// License helpers
fn storage_get_license(env: &Env, license_id: &BytesN<32>) -> Option<NFTLicense> {
    let key = NFTProvenanceDataKey::NFTLicense(*license_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_license(env: &Env, license_id: &BytesN<32>, license: &NFTLicense) {
    let key = NFTProvenanceDataKey::NFTLicense(*license_id);
    env.storage().persistent().set(&key, license);
}

fn storage_get_nft_license_list(env: &Env, nft_id: &BytesN<32>) -> Vec<BytesN<32>> {
    let key = NFTProvenanceDataKey::NFTLicenseList(*nft_id);
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_nft_license_list(env: &Env, nft_id: &BytesN<32>, list: &Vec<BytesN<32>>) {
    let key = NFTProvenanceDataKey::NFTLicenseList(*nft_id);
    env.storage().persistent().set(&key, list);
}

// Fractionalization helpers
fn storage_get_fractional_nft(env: &Env, fnft_id: &BytesN<32>) -> Option<FractionalNFT> {
    let key = NFTProvenanceDataKey::FractionalNFT(*fnft_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_fractional_nft(env: &Env, fnft_id: &BytesN<32>, fnft: &FractionalNFT) {
    let key = NFTProvenanceDataKey::FractionalNFT(*fnft_id);
    env.storage().persistent().set(&key, fnft);
}

fn storage_get_fraction_ownership(env: &Env, ownership_id: &BytesN<32>) -> Option<FractionOwnership> {
    let key = NFTProvenanceDataKey::FractionOwnership(*ownership_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_fraction_ownership(env: &Env, ownership_id: &BytesN<32>, ownership: &FractionOwnership) {
    let key = NFTProvenanceDataKey::FractionOwnership(*ownership_id);
    env.storage().persistent().set(&key, ownership);
}

// Cross-chain helpers
fn storage_get_cross_chain_royalty(env: &Env, record_id: &BytesN<32>) -> Option<CrossChainRoyalty> {
    let key = NFTProvenanceDataKey::CrossChainRoyalty(*record_id);
    env.storage().persistent().get(&key).unwrap_or(None)
}

fn storage_set_cross_chain_royalty(env: &Env, record_id: &BytesN<32>, record: &CrossChainRoyalty) {
    let key = NFTProvenanceDataKey::CrossChainRoyalty(*record_id);
    env.storage().persistent().set(&key, record);
}

// NFT list helpers
fn storage_get_creator_nft_list(env: &Env, creator: &Address) -> Vec<BytesN<32>> {
    let key = NFTProvenanceDataKey::CreatorNFTList(creator.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_creator_nft_list(env: &Env, creator: &Address, list: &Vec<BytesN<32>>) {
    let key = NFTProvenanceDataKey::CreatorNFTList(creator.clone());
    env.storage().persistent().set(&key, list);
}

fn storage_get_owner_nft_list(env: &Env, owner: &Address) -> Vec<BytesN<32>> {
    let key = NFTProvenanceDataKey::OwnerNFTList(owner.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(Vec::new(env)))
        .unwrap_or_else(|| Vec::new(env))
}

fn storage_set_owner_nft_list(env: &Env, owner: &Address, list: &Vec<BytesN<32>>) {
    let key = NFTProvenanceDataKey::OwnerNFTList(owner.clone());
    env.storage().persistent().set(&key, list);
}

// Counter helpers
fn storage_get_creator_nft_count(env: &Env, creator: &Address) -> u32 {
    let key = NFTProvenanceDataKey::CreatorNFTCount(creator.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_creator_nft_count(env: &Env, creator: &Address, count: u32) {
    let key = NFTProvenanceDataKey::CreatorNFTCount(creator.clone());
    env.storage().persistent().set(&key, &count);
}

fn storage_get_owner_nft_count(env: &Env, owner: &Address) -> u32 {
    let key = NFTProvenanceDataKey::OwnerNFTCount(owner.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_owner_nft_count(env: &Env, owner: &Address, count: u32) {
    let key = NFTProvenanceDataKey::OwnerNFTCount(owner.clone());
    env.storage().persistent().set(&key, &count);
}

fn storage_get_total_nft_count(env: &Env) -> u32 {
    let key = NFTProvenanceDataKey::TotalNFTCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_total_nft_count(env: &Env, count: u32) {
    let key = NFTProvenanceDataKey::TotalNFTCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_creator_count(env: &Env) -> u32 {
    let key = NFTProvenanceDataKey::CreatorCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_creator_count(env: &Env, count: u32) {
    let key = NFTProvenanceDataKey::CreatorCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_royalty_payment_count(env: &Env) -> u32 {
    let key = NFTProvenanceDataKey::RoyaltyPaymentCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_royalty_payment_count(env: &Env, count: u32) {
    let key = NFTProvenanceDataKey::RoyaltyPaymentCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_license_count(env: &Env) -> u32 {
    let key = NFTProvenanceDataKey::LicenseCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_license_count(env: &Env, count: u32) {
    let key = NFTProvenanceDataKey::LicenseCount;
    env.storage().persistent().set(&key, &count);
}

fn storage_get_fractionalization_count(env: &Env) -> u32 {
    let key = NFTProvenanceDataKey::FractionalizationCount;
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(Some(0u32))
        .unwrap_or(0u32)
}

fn storage_set_fractionalization_count(env: &Env, count: u32) {
    let key = NFTProvenanceDataKey::FractionalizationCount;
    env.storage().persistent().set(&key, &count);
}

// ==================== ID GENERATION ====================

fn compute_nft_id(env: &Env, creator: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let addr_bytes = creator.to_string().as_bytes();
    let mut data = [0u8; 40];
    if addr_bytes.len() <= 32 {
        data[0..addr_bytes.len()].copy_from_slice(addr_bytes);
    }
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_ownership_record_id(env: &Env, nft_id: &BytesN<32>) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&nft_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_royalty_payment_id(env: &Env, nft_id: &BytesN<32>) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&nft_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_license_id(env: &Env, nft_id: &BytesN<32>, licensee: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&nft_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_fnft_id(env: &Env, nft_id: &BytesN<32>) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&nft_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_fraction_ownership_id(env: &Env, fnft_id: &BytesN<32>, owner: &Address) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&fnft_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}

fn compute_cross_chain_royalty_id(env: &Env, nft_id: &BytesN<32>) -> BytesN<32> {
    let nonce = env.ledger().timestamp();
    let mut data = [0u8; 40];
    data[0..32].copy_from_slice(&nft_id.to_array());
    data[32..40].copy_from_slice(&nonce.to_le_bytes());
    env.crypto().sha256(&Bytes::from_slice(env, &data))
}
