//! Comprehensive tests for NFT provenance system
//!
//! Tests cover creator registration, NFT minting, ownership tracking,
//! royalty payments, licenses, fractionalization, and cross-chain support.

#[cfg(test)]
mod nft_provenance_tests {
    use crate::nft_provenance::*;
    use crate::nft_provenance_impl::*;
    use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, Symbol, Vec};

    fn setup_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    // ==================== CREATOR MANAGEMENT TESTS ====================

    #[test]
    fn test_register_creator() {
        let env = setup_env();
        let creator = Address::generate(&env);

        let result = NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Digital artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        );

        assert!(result.is_ok());

        let profile = NFTProvenanceContract::get_creator(env, creator)
            .expect("Failed to get creator");
        assert_eq!(profile.verified, false);
    }

    #[test]
    fn test_verify_creator() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Digital artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        NFTProvenanceContract::verify_creator(env.clone(), creator.clone())
            .expect("Failed to verify");

        let profile = NFTProvenanceContract::get_creator(env, creator)
            .expect("Failed to get creator");
        assert_eq!(profile.verified, true);
    }

    // ==================== NFT MINTING TESTS ====================

    #[test]
    fn test_mint_nft() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register creator");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Artwork #1"),
            Bytes::from_slice(&env, b"Beautiful digital art"),
            Bytes::from_slice(&env, b"https://ipfs.io/..."),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint NFT");

        assert_ne!(nft_id.to_array(), [0u8; 32]);

        let nft = NFTProvenanceContract::get_nft(env, nft_id)
            .expect("Failed to get NFT");
        assert_eq!(nft.creator, creator);
    }

    #[test]
    fn test_nft_count() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT 1"),
            Bytes::from_slice(&env, b"Description"),
            Bytes::from_slice(&env, b"uri1"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint 1");

        NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT 2"),
            Bytes::from_slice(&env, b"Description"),
            Bytes::from_slice(&env, b"uri2"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint 2");

        let total = NFTProvenanceContract::total_nft_count(env.clone());
        assert_eq!(total, 2u32);

        let creator_count = NFTProvenanceContract::creator_nft_count(env, creator);
        assert_eq!(creator_count, 2u32);
    }

    // ==================== OWNERSHIP TRACKING TESTS ====================

    #[test]
    fn test_transfer_ownership() {
        let env = setup_env();
        let creator = Address::generate(&env);
        let buyer = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let record_id = NFTProvenanceContract::transfer_ownership(
            env.clone(),
            nft_id,
            buyer.clone(),
            1000u128, // sale price
            0u32,     // sale
        ).expect("Failed to transfer");

        let nft = NFTProvenanceContract::get_nft(env, nft_id)
            .expect("Failed to get NFT");
        assert_eq!(nft.current_owner, buyer);
    }

    #[test]
    fn test_ownership_history() {
        let env = setup_env();
        let creator = Address::generate(&env);
        let buyer = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        NFTProvenanceContract::transfer_ownership(
            env.clone(),
            nft_id,
            buyer,
            1000u128,
            0u32,
        ).expect("Failed to transfer");

        let history_count = NFTProvenanceContract::get_ownership_history(env, nft_id)
            .expect("Failed to get history");
        assert_eq!(history_count, 1u32);
    }

    // ==================== ROYALTY MANAGEMENT TESTS ====================

    #[test]
    fn test_set_royalty_config() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let result = NFTProvenanceContract::set_royalty_config(
            env.clone(),
            nft_id,
            500u32, // 5%
            creator.clone(),
        );

        assert!(result.is_ok());

        let config = NFTProvenanceContract::get_royalty_config(env, nft_id)
            .expect("Failed to get config");
        assert_eq!(config.royalty_rate_bp, 500u32);
    }

    #[test]
    fn test_calculate_royalty() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        NFTProvenanceContract::set_royalty_config(
            env.clone(),
            nft_id,
            500u32, // 5%
            creator.clone(),
        ).expect("Failed to set config");

        let royalty = NFTProvenanceContract::calculate_royalty(
            env,
            nft_id,
            10000u128, // $100 sale
        ).expect("Failed to calculate");

        assert_eq!(royalty, 500u128); // 5% of 10000
    }

    #[test]
    fn test_pay_royalty() {
        let env = setup_env();
        let creator = Address::generate(&env);
        let seller = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        NFTProvenanceContract::set_royalty_config(
            env.clone(),
            nft_id,
            500u32,
            creator.clone(),
        ).expect("Failed to set config");

        let payment_id = NFTProvenanceContract::pay_royalty(
            env.clone(),
            nft_id,
            seller,
            10000u128,
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to pay royalty");

        assert_ne!(payment_id.to_array(), [0u8; 32]);

        let total_royalties = NFTProvenanceContract::creator_total_royalties(env, creator);
        assert_eq!(total_royalties, 500u128);
    }

    // ==================== LICENSE MANAGEMENT TESTS ====================

    #[test]
    fn test_grant_license() {
        let env = setup_env();
        let creator = Address::generate(&env);
        let licensee = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let license_id = NFTProvenanceContract::grant_license(
            env.clone(),
            nft_id,
            licensee.clone(),
            1u32, // commercial
            Rights::VIEW | Rights::COPY,
            86400u64, // 1 day
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to grant license");

        assert_ne!(license_id.to_array(), [0u8; 32]);

        let license = NFTProvenanceContract::get_license(env, license_id)
            .expect("Failed to get license");
        assert_eq!(license.licensee, licensee);
    }

    #[test]
    fn test_verify_license_rights() {
        let env = setup_env();
        let creator = Address::generate(&env);
        let licensee = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let license_id = NFTProvenanceContract::grant_license(
            env.clone(),
            nft_id,
            licensee,
            1u32,
            Rights::VIEW | Rights::COPY,
            86400u64,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to grant license");

        let has_rights = NFTProvenanceContract::verify_license(
            env,
            license_id,
            Rights::VIEW,
        ).expect("Failed to verify license");

        assert!(has_rights);
    }

    // ==================== FRACTIONALIZATION TESTS ====================

    #[test]
    fn test_fractionalize_nft() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let fnft_id = NFTProvenanceContract::fractionalize_nft(
            env.clone(),
            nft_id,
            1_000_000u128, // 1 million fractions
            18u32,
            Bytes::from_slice(&env, b"Fractional NFT"),
            Bytes::from_slice(&env, b"fNFT"),
            true,
            100u128,
        ).expect("Failed to fractionalize");

        assert_ne!(fnft_id.to_array(), [0u8; 32]);

        let fnft = NFTProvenanceContract::get_fractional_nft(env, fnft_id)
            .expect("Failed to get F-NFT");
        assert_eq!(fnft.total_fractions, 1_000_000u128);
    }

    #[test]
    fn test_transfer_fractions() {
        let env = setup_env();
        let creator = Address::generate(&env);
        let buyer = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let fnft_id = NFTProvenanceContract::fractionalize_nft(
            env.clone(),
            nft_id,
            1_000_000u128,
            18u32,
            Bytes::from_slice(&env, b"Fractional NFT"),
            Bytes::from_slice(&env, b"fNFT"),
            true,
            100u128,
        ).expect("Failed to fractionalize");

        NFTProvenanceContract::transfer_fraction(
            env.clone(),
            fnft_id,
            creator.clone(),
            buyer.clone(),
            100_000u128, // transfer 10%
        ).expect("Failed to transfer fractions");

        let buyer_balance = NFTProvenanceContract::get_fraction_balance(env, fnft_id, buyer)
            .expect("Failed to get balance");
        assert_eq!(buyer_balance, 100_000u128);
    }

    // ==================== CROSS-CHAIN TESTS ====================

    #[test]
    fn test_cross_chain_royalty() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let record_id = NFTProvenanceContract::initiate_cross_chain_royalty(
            env.clone(),
            nft_id,
            Symbol::new(&env, "polygon"),
            500u128, // 5 USD
        ).expect("Failed to initiate");

        assert_ne!(record_id.to_array(), [0u8; 32]);

        let record = NFTProvenanceContract::get_cross_chain_royalty(env, record_id)
            .expect("Failed to get record");
        assert_eq!(record.status, 1u32); // locked
    }

    #[test]
    fn test_complete_cross_chain_royalty() {
        let env = setup_env();
        let creator = Address::generate(&env);

        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"NFT"),
            Bytes::from_slice(&env, b"Art"),
            Bytes::from_slice(&env, b"uri"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        let record_id = NFTProvenanceContract::initiate_cross_chain_royalty(
            env.clone(),
            nft_id,
            Symbol::new(&env, "polygon"),
            500u128,
        ).expect("Failed to initiate");

        NFTProvenanceContract::complete_cross_chain_royalty(env.clone(), record_id)
            .expect("Failed to complete");

        let record = NFTProvenanceContract::get_cross_chain_royalty(env, record_id)
            .expect("Failed to get record");
        assert_eq!(record.status, 2u32); // transferred
    }

    // ==================== FULL WORKFLOW TEST ====================

    #[test]
    fn test_full_nft_provenance_workflow() {
        let env = setup_env();
        let creator = Address::generate(&env);
        let buyer1 = Address::generate(&env);
        let buyer2 = Address::generate(&env);

        // 1. Register creator
        NFTProvenanceContract::register_creator(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Alice"),
            Bytes::from_slice(&env, b"Digital Artist"),
            Bytes::from_slice(&env, b"https://alice.art"),
        ).expect("Failed to register");

        // 2. Mint NFT
        let nft_id = NFTProvenanceContract::mint_nft(
            env.clone(),
            creator.clone(),
            Bytes::from_slice(&env, b"Masterpiece"),
            Bytes::from_slice(&env, b"Beautiful artwork"),
            Bytes::from_slice(&env, b"https://ipfs.io/QmXXX"),
            Symbol::new(&env, "ERC-721"),
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to mint");

        // 3. Set royalties
        NFTProvenanceContract::set_royalty_config(
            env.clone(),
            nft_id,
            1000u32, // 10%
            creator.clone(),
        ).expect("Failed to set royalties");

        // 4. Transfer to first buyer
        NFTProvenanceContract::transfer_ownership(
            env.clone(),
            nft_id,
            buyer1.clone(),
            10000u128, // $100 sale
            0u32,
        ).expect("Failed to transfer to buyer1");

        // 5. Pay royalties
        NFTProvenanceContract::pay_royalty(
            env.clone(),
            nft_id,
            buyer1.clone(),
            10000u128,
            Symbol::new(&env, "ethereum"),
        ).expect("Failed to pay royalty");

        // 6. Transfer to second buyer
        NFTProvenanceContract::transfer_ownership(
            env.clone(),
            nft_id,
            buyer2.clone(),
            15000u128, // $150 sale
            0u32,
        ).expect("Failed to transfer to buyer2");

        // 7. Grant license
        let _license_id = NFTProvenanceContract::grant_license(
            env.clone(),
            nft_id,
            buyer1.clone(),
            0u32, // personal use
            Rights::VIEW,
            86400u64,
            BytesN::<32>::from_array(&[1u8; 32]),
        ).expect("Failed to grant license");

        // 8. Verify final state
        assert_eq!(NFTProvenanceContract::total_nft_count(env.clone()), 1u32);
        let final_nft = NFTProvenanceContract::get_nft(env.clone(), nft_id)
            .expect("Failed to get final NFT");
        assert_eq!(final_nft.current_owner, buyer2);
    }
}
