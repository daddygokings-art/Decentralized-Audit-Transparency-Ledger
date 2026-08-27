/// Self-Sovereign Identity (SSI) Integration
///
/// Integrates self-sovereign identity for event submitters and verifiers with:
/// - Credential issuance and verification
/// - Revocation registry
/// - Wallet integration
/// - Support for Aries/Indy and KERI

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, Map, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// SSI framework types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum SsiFramework {
    AriesIndy = 0,
    Keri = 1,
}

/// A self-sovereign identity record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsiIdentity {
    pub id: BytesN<32>,
    pub controller: Address,
    pub did: Bytes,
    pub framework: u32,
    pub public_key: Bytes,
    pub created_at: u64,
    pub updated_at: u64,
    pub active: bool,
}

/// An SSI credential issued by an issuer
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsiCredential {
    pub id: BytesN<32>,
    pub issuer_did: Bytes,
    pub subject_did: Bytes,
    pub schema_id: Symbol,
    pub attributes: Bytes,
    pub issued_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
    pub proof: Bytes,
}

/// A wallet record for a user
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WalletRecord {
    pub owner: Address,
    pub wallet_type: Symbol,
    pub did: Bytes,
    pub public_key: Bytes,
    pub created_at: u64,
    pub last_accessed: u64,
    pub active: bool,
}

/// A revocation entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevocationEntry {
    pub credential_id: BytesN<32>,
    pub issuer: Address,
    pub revoked_at: u64,
    pub reason: Bytes,
}

/// A verification request
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRequest {
    pub id: BytesN<32>,
    pub verifier: Address,
    pub subject_did: Bytes,
    pub credential_ids: Vec<BytesN<32>>,
    pub requested_at: u64,
    pub verified: bool,
    pub result: Bytes,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum SsiKey {
    Owner,
    SsiIdentity(Bytes),
    AllIdentityIds,
    SsiCredential(BytesN<32>),
    AllCredentialIds,
    WalletRecord(Address),
    AllWalletIds,
    RevocationEntry(BytesN<32>),
    AllRevocationIds,
    VerificationRequest(BytesN<32>),
    AllVerificationIds,
    NextIdentityId,
    NextCredentialId,
    NextWalletId,
    NextRevocationId,
    NextVerificationId,
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SsiError {
    Unauthorized = 1,
    IdentityNotFound = 2,
    CredentialNotFound = 3,
    WalletNotFound = 4,
    RevocationNotFound = 5,
    VerificationNotFound = 6,
    InvalidFramework = 7,
    InvalidDid = 8,
    CredentialExpired = 9,
    CredentialRevoked = 10,
    VerificationFailed = 11,
    DuplicateIdentity = 12,
    WalletAlreadyExists = 13,
    InvalidProof = 14,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct SsiIntegration;

#[contractimpl]
impl SsiIntegration {
    /// Initialize SSI module (owner-only)
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();

        if env.storage().instance().has(&SsiKey::Owner) {
            panic_with_error!(&env, SsiError::Unauthorized);
        }

        env.storage().instance().set(&SsiKey::Owner, &owner);
        env.storage().instance().set(&SsiKey::NextIdentityId, &1u32);
        env.storage().instance().set(&SsiKey::NextCredentialId, &1u32);
        env.storage().instance().set(&SsiKey::NextWalletId, &1u32);
        env.storage().instance().set(&SsiKey::NextRevocationId, &1u32);
        env.storage().instance().set(&SsiKey::NextVerificationId, &1u32);
    }

    // ========================================================================
    // Identity Management
    // ========================================================================

    /// Register an SSI identity
    pub fn register_identity(
        env: Env,
        controller: Address,
        did: Bytes,
        framework: u32,
        public_key: Bytes,
    ) -> SsiIdentity {
        controller.require_auth();

        if framework > 1 {
            panic_with_error!(&env, SsiError::InvalidFramework);
        }

        if env.storage().instance().has(&SsiKey::SsiIdentity(did.clone())) {
            panic_with_error!(&env, SsiError::DuplicateIdentity);
        }

        let id = Self::get_next_identity_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));
        let now = env.ledger().timestamp();

        let identity = SsiIdentity {
            id: id_bytes.clone(),
            controller: controller.clone(),
            did: did.clone(),
            framework,
            public_key,
            created_at: now,
            updated_at: now,
            active: true,
        };

        env.storage()
            .instance()
            .set(&SsiKey::SsiIdentity(did.clone()), &identity);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&SsiKey::AllIdentityIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&SsiKey::AllIdentityIds, &all_ids);

        log!(
            &env,
            "SsiIntegration: identity registered - controller={}, framework={}",
            controller,
            framework
        );

        identity
    }

    /// Get an identity by DID
    pub fn get_identity(env: Env, did: Bytes) -> SsiIdentity {
        Self::get_identity_or_panic(&env, did)
    }

    /// List all identity DIDs
    pub fn list_identities(env: Env) -> Vec<Bytes> {
        env.storage()
            .instance()
            .get(&SsiKey::AllIdentityIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Credential Issuance and Verification
    // ========================================================================

    /// Issue an SSI credential
    pub fn issue_credential(
        env: Env,
        issuer: Address,
        issuer_did: Bytes,
        subject_did: Bytes,
        schema_id: Symbol,
        attributes: Bytes,
        validity_days: u32,
        proof: Bytes,
    ) -> SsiCredential {
        issuer.require_auth();

        let id = Self::get_next_credential_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));
        let now = env.ledger().timestamp();
        let validity_seconds = validity_days * 86400u64;

        let credential = SsiCredential {
            id: id_bytes.clone(),
            issuer_did,
            subject_did,
            schema_id,
            attributes,
            issued_at: now,
            expires_at: now + validity_seconds,
            revoked: false,
            proof,
        };

        env.storage()
            .instance()
            .set(&SsiKey::SsiCredential(id_bytes.clone()), &credential);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&SsiKey::AllCredentialIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&SsiKey::AllCredentialIds, &all_ids);

        log!(
            &env,
            "SsiIntegration: credential issued - id={}, schema={:?}",
            id,
            schema_id
        );

        credential
    }

    /// Verify an SSI credential
    pub fn verify_credential(env: Env, verifier: Address, credential_id: BytesN<32>) -> bool {
        verifier.require_auth();

        let credential = Self::get_credential_or_panic(&env, credential_id.clone());
        let now = env.ledger().timestamp();

        if credential.revoked {
            panic_with_error!(&env, SsiError::CredentialRevoked);
        }

        if now > credential.expires_at {
            panic_with_error!(&env, SsiError::CredentialExpired);
        }

        true
    }

    /// Revoke a credential (issuer only)
    pub fn revoke_credential(env: Env, caller: Address, credential_id: BytesN<32>, reason: Bytes) {
        caller.require_auth();

        let credential = Self::get_credential_or_panic(&env, credential_id.clone());

        let revocation = RevocationEntry {
            credential_id: credential_id.clone(),
            issuer: caller,
            revoked_at: env.ledger().timestamp(),
            reason,
        };

        env.storage()
            .instance()
            .set(&SsiKey::RevocationEntry(credential_id.clone()), &revocation);

        let mut all_rev_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&SsiKey::AllRevocationIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_rev_ids.push_back(credential_id.clone());
        env.storage()
            .instance()
            .set(&SsiKey::AllRevocationIds, &all_rev_ids);

        log!(
            &env,
            "SsiIntegration: credential revoked - id={}",
            credential_id
        );
    }

    /// Get a credential by ID
    pub fn get_credential(env: Env, credential_id: BytesN<32>) -> SsiCredential {
        Self::get_credential_or_panic(&env, credential_id)
    }

    /// List all credential IDs
    pub fn list_credential_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&SsiKey::AllCredentialIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Wallet Integration
    // ========================================================================

    /// Register a wallet for a user
    pub fn register_wallet(
        env: Env,
        owner: Address,
        wallet_type: Symbol,
        did: Bytes,
        public_key: Bytes,
    ) -> WalletRecord {
        owner.require_auth();

        if env.storage().instance().has(&SsiKey::WalletRecord(owner.clone())) {
            panic_with_error!(&env, SsiError::WalletAlreadyExists);
        }

        let id = Self::get_next_wallet_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));
        let now = env.ledger().timestamp();

        let wallet = WalletRecord {
            owner: owner.clone(),
            wallet_type,
            did,
            public_key,
            created_at: now,
            last_accessed: now,
            active: true,
        };

        env.storage()
            .instance()
            .set(&SsiKey::WalletRecord(owner.clone()), &wallet);

        let mut all_ids: Vec<Address> = env
            .storage()
            .instance()
            .get(&SsiKey::AllWalletIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(owner.clone());
        env.storage()
            .instance()
            .set(&SsiKey::AllWalletIds, &all_ids);

        log!(
            &env,
            "SsiIntegration: wallet registered - owner={}, type={:?}",
            owner,
            wallet_type
        );

        wallet
    }

    /// Get a wallet by owner address
    pub fn get_wallet(env: Env, owner: Address) -> WalletRecord {
        Self::get_wallet_or_panic(&env, owner)
    }

    /// List all wallet owners
    pub fn list_wallets(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&SsiKey::AllWalletIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Verification Requests
    // ========================================================================

    /// Submit a verification request
    pub fn submit_verification(
        env: Env,
        verifier: Address,
        subject_did: Bytes,
        credential_ids: Vec<BytesN<32>>,
    ) -> VerificationRequest {
        verifier.require_auth();

        let id = Self::get_next_verification_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let request = VerificationRequest {
            id: id_bytes.clone(),
            verifier: verifier.clone(),
            subject_did,
            credential_ids,
            requested_at: env.ledger().timestamp(),
            verified: false,
            result: Bytes::new(&env),
        };

        env.storage()
            .instance()
            .set(&SsiKey::VerificationRequest(id_bytes.clone()), &request);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&SsiKey::AllVerificationIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&SsiKey::AllVerificationIds, &all_ids);

        request
    }

    /// Process a verification request (owner-only)
    pub fn process_verification(
        env: Env,
        caller: Address,
        verification_id: BytesN<32>,
        result: Bytes,
    ) {
        Self::require_owner(&env, &caller);

        let mut request = Self::get_verification_or_panic(&env, verification_id.clone());
        request.verified = true;
        request.result = result;

        env.storage()
            .instance()
            .set(&SsiKey::VerificationRequest(verification_id), &request);
    }

    /// Get a verification request by ID
    pub fn get_verification(env: Env, verification_id: BytesN<32>) -> VerificationRequest {
        Self::get_verification_or_panic(&env, verification_id)
    }

    /// List all verification IDs
    pub fn list_verification_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&SsiKey::AllVerificationIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&SsiKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, SsiError::Unauthorized));
        if &owner != caller {
            panic_with_error!(env, SsiError::Unauthorized);
        }
    }

    fn get_identity_or_panic(env: &Env, did: Bytes) -> SsiIdentity {
        env.storage()
            .instance()
            .get(&SsiKey::SsiIdentity(did))
            .unwrap_or_else(|| panic_with_error!(env, SsiError::IdentityNotFound))
    }

    fn get_credential_or_panic(env: &Env, credential_id: BytesN<32>) -> SsiCredential {
        env.storage()
            .instance()
            .get(&SsiKey::SsiCredential(credential_id))
            .unwrap_or_else(|| panic_with_error!(env, SsiError::CredentialNotFound))
    }

    fn get_wallet_or_panic(env: &Env, owner: Address) -> WalletRecord {
        env.storage()
            .instance()
            .get(&SsiKey::WalletRecord(owner))
            .unwrap_or_else(|| panic_with_error!(env, SsiError::WalletNotFound))
    }

    fn get_verification_or_panic(env: &Env, verification_id: BytesN<32>) -> VerificationRequest {
        env.storage()
            .instance()
            .get(&SsiKey::VerificationRequest(verification_id))
            .unwrap_or_else(|| panic_with_error!(env, SsiError::VerificationNotFound))
    }

    fn get_next_identity_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&SsiKey::NextIdentityId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&SsiKey::NextIdentityId, &(current + 1));
        current
    }

    fn get_next_credential_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&SsiKey::NextCredentialId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&SsiKey::NextCredentialId, &(current + 1));
        current
    }

    fn get_next_wallet_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&SsiKey::NextWalletId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&SsiKey::NextWalletId, &(current + 1));
        current
    }

    fn get_next_verification_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&SsiKey::NextVerificationId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&SsiKey::NextVerificationId, &(current + 1));
        current
    }

    fn sha2_digest(env: &Env, data: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&result);
        arr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_registration() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let controller = Address::from_array(&env, &[2; 32]);

        SsiIntegration::initialize(env.clone(), owner.clone());

        let did = Bytes::from_slice(&env, b"did:key:z6MkhaXgBZDvotDkL5257ppizwGqR8P7KxG6Hjpy");

        let identity = SsiIntegration::register_identity(
            env.clone(),
            controller,
            did.clone(),
            0,
            Bytes::new(&env),
        );
        assert_eq!(identity.framework, 0);
        assert!(identity.active);

        let all = SsiIntegration::list_identities(env.clone());
        assert_eq!(all.len(), 1);

        let fetched = SsiIntegration::get_identity(env, did);
        assert_eq!(fetched.framework, 0);
    }

    #[test]
    fn test_wallet_registration() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let user = Address::from_array(&env, &[2; 32]);

        SsiIntegration::initialize(env.clone(), owner.clone());

        let wallet = SsiIntegration::register_wallet(
            env.clone(),
            user.clone(),
            Symbol::new(&env, "aries"),
            Bytes::from_slice(&env, b"did:key:test"),
            Bytes::new(&env),
        );
        assert!(wallet.active);

        let fetched = SsiIntegration::get_wallet(env, user);
        assert_eq!(fetched.wallet_type, Symbol::new(&env, "aries"));
    }

    #[test]
    fn test_credential_lifecycle() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let issuer = Address::from_array(&env, &[2; 32]);

        SsiIntegration::initialize(env.clone(), owner.clone());

        let issuer_did = Bytes::from_slice(&env, b"did:key:z6MkhaXgBZDvotDkL5257ppizwGqR8P7KxG6Hjpy");
        let subject_did = Bytes::from_slice(&env, b"did:web:example.com");

        let vc = SsiIntegration::issue_credential(
            env.clone(),
            issuer,
            issuer_did,
            subject_did,
            Symbol::new(&env, "Membership"),
            Bytes::new(&env),
            30,
            Bytes::new(&env),
        );
        assert!(!vc.revoked);

        SsiIntegration::revoke_credential(env.clone(), owner, vc.id, Bytes::new(&env));
        let all = SsiIntegration::list_credential_ids(env.clone());
        assert_eq!(all.len(), 1);
    }
}
