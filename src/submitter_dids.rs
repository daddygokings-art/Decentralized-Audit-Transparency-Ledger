/// Contract Event Decentralized Identifiers (DIDs) for Submitters
///
/// Implements DIDs for event submitters with:
/// - DIDComm messaging
/// - Verifiable presentations
/// - DID resolution
/// - Support for did:key, did:web, and did:stellar methods

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol,
    Vec, Map, panic_with_error, log,
};

// ============================================================================
// Data Structures
// ============================================================================

/// Supported DID methods
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum DidMethod {
    DidKey = 0,
    DidWeb = 1,
    DidStellar = 2,
}

/// A decentralized identifier record for a submitter
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DidRecord {
    pub did: Bytes,
    pub method: u32,
    pub submitter: Address,
    pub document: Bytes,
    pub created_at: u64,
    pub updated_at: u64,
    pub verified: bool,
}

/// A verifiable presentation for DID authentication
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiablePresentation {
    pub id: BytesN<32>,
    pub holder_did: Bytes,
    pub verifier: Address,
    pub credentials: Vec<BytesN<32>>,
    pub proof: Bytes,
    pub created_at: u64,
    pub verified: bool,
}

/// A DIDComm message
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DidCommMessage {
    pub id: BytesN<32>,
    pub from_did: Bytes,
    pub to_did: Bytes,
    pub message_type: Symbol,
    pub body: Bytes,
    pub sent_at: u64,
    pub delivered: bool,
}

/// A verifiable credential issued to a DID
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiableCredential {
    pub id: BytesN<32>,
    pub issuer_did: Bytes,
    pub subject_did: Bytes,
    pub credential_type: Symbol,
    pub attributes: Bytes,
    pub issued_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
}

// ============================================================================
// Storage Keys
// ============================================================================

#[contracttype]
pub enum DidKey {
    Owner,
    DidRecord(Bytes),
    AllDidIds,
    VerifiablePresentation(BytesN<32>),
    AllPresentationIds,
    DidCommMessage(BytesN<32>),
    AllMessageIds,
    VerifiableCredential(BytesN<32>),
    AllCredentialIds,
    NextDidId,
    NextPresentationId,
    NextMessageId,
    NextCredentialId,
    ResolverCache(Bytes),
}

// ============================================================================
// Contract Errors
// ============================================================================

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DidError {
    Unauthorized = 1,
    DidNotFound = 2,
    InvalidDidMethod = 3,
    InvalidDid = 4,
    PresentationNotFound = 5,
    MessageNotFound = 6,
    CredentialNotFound = 7,
    CredentialExpired = 8,
    CredentialRevoked = 9,
    VerificationFailed = 10,
    DuplicateDid = 11,
    ResolverError = 12,
}

// ============================================================================
// Contract Implementation
// ============================================================================

#[contract]
pub struct SubmitterDids;

#[contractimpl]
impl SubmitterDids {
    /// Initialize DID module (owner-only)
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();

        if env.storage().instance().has(&DidKey::Owner) {
            panic_with_error!(&env, DidError::Unauthorized);
        }

        env.storage().instance().set(&DidKey::Owner, &owner);
        env.storage().instance().set(&DidKey::NextDidId, &1u32);
        env.storage().instance().set(&DidKey::NextPresentationId, &1u32);
        env.storage().instance().set(&DidKey::NextMessageId, &1u32);
        env.storage().instance().set(&DidKey::NextCredentialId, &1u32);
    }

    // ========================================================================
    // DID Management
    // ========================================================================

    /// Register a DID for a submitter
    pub fn register_did(
        env: Env,
        submitter: Address,
        did: Bytes,
        method: u32,
        document: Bytes,
    ) -> DidRecord {
        submitter.require_auth();

        if method > 2 {
            panic_with_error!(&env, DidError::InvalidDidMethod);
        }

        if env.storage().instance().has(&DidKey::DidRecord(did.clone())) {
            panic_with_error!(&env, DidError::DuplicateDid);
        }

        let now = env.ledger().timestamp();

        let record = DidRecord {
            did: did.clone(),
            method,
            submitter: submitter.clone(),
            document,
            created_at: now,
            updated_at: now,
            verified: false,
        };

        env.storage()
            .instance()
            .set(&DidKey::DidRecord(did.clone()), &record);

        let mut all_ids: Vec<Bytes> = env
            .storage()
            .instance()
            .get(&DidKey::AllDidIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(did.clone());
        env.storage()
            .instance()
            .set(&DidKey::AllDidIds, &all_ids);

        log!(
            &env,
            "SubmitterDids: DID registered - submitter={}, method={}",
            submitter,
            method
        );

        record
    }

    /// Verify a DID (owner-only)
    pub fn verify_did(env: Env, caller: Address, did: Bytes) {
        Self::require_owner(&env, &caller);

        let mut record = Self::get_did_or_panic(&env, did.clone());
        record.verified = true;
        record.updated_at = env.ledger().timestamp();

        env.storage().instance().set(&DidKey::DidRecord(did), &record);
    }

    /// Resolve a DID to its document
    pub fn resolve_did(env: Env, did: Bytes) -> DidRecord {
        Self::get_did_or_panic(&env, did)
    }

    /// Get all registered DID strings
    pub fn list_dids(env: Env) -> Vec<Bytes> {
        env.storage()
            .instance()
            .get(&DidKey::AllDidIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // DIDComm Messaging
    // ========================================================================

    /// Send a DIDComm message
    pub fn send_message(
        env: Env,
        sender: Address,
        from_did: Bytes,
        to_did: Bytes,
        message_type: Symbol,
        body: Bytes,
    ) -> DidCommMessage {
        sender.require_auth();

        let id = Self::get_next_message_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let message = DidCommMessage {
            id: id_bytes.clone(),
            from_did,
            to_did,
            message_type,
            body,
            sent_at: env.ledger().timestamp(),
            delivered: false,
        };

        env.storage()
            .instance()
            .set(&DidKey::DidCommMessage(id_bytes.clone()), &message);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&DidKey::AllMessageIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&DidKey::AllMessageIds, &all_ids);

        message
    }

    /// Mark a DIDComm message as delivered
    pub fn mark_delivered(env: Env, caller: Address, message_id: BytesN<32>) {
        Self::require_owner(&env, &caller);

        let mut message = Self::get_message_or_panic(&env, message_id.clone());
        message.delivered = true;

        env.storage()
            .instance()
            .set(&DidKey::DidCommMessage(message_id), &message);
    }

    /// Get a DIDComm message by ID
    pub fn get_message(env: Env, message_id: BytesN<32>) -> DidCommMessage {
        Self::get_message_or_panic(&env, message_id)
    }

    /// List all message IDs
    pub fn list_message_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&DidKey::AllMessageIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Verifiable Presentations
    // ========================================================================

    /// Create a verifiable presentation
    pub fn create_presentation(
        env: Env,
        holder: Address,
        holder_did: Bytes,
        credentials: Vec<BytesN<32>>,
        proof: Bytes,
    ) -> VerifiablePresentation {
        holder.require_auth();

        let id = Self::get_next_presentation_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));

        let presentation = VerifiablePresentation {
            id: id_bytes.clone(),
            holder_did,
            verifier: holder.clone(),
            credentials,
            proof,
            created_at: env.ledger().timestamp(),
            verified: false,
        };

        env.storage()
            .instance()
            .set(&DidKey::VerifiablePresentation(id_bytes.clone()), &presentation);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&DidKey::AllPresentationIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&DidKey::AllPresentationIds, &all_ids);

        presentation
    }

    /// Verify a presentation (owner-only)
    pub fn verify_presentation(env: Env, caller: Address, presentation_id: BytesN<32>) {
        Self::require_owner(&env, &caller);

        let mut presentation = Self::get_presentation_or_panic(&env, presentation_id.clone());
        presentation.verified = true;

        env.storage()
            .instance()
            .set(&DidKey::VerifiablePresentation(presentation_id), &presentation);
    }

    /// Get a presentation by ID
    pub fn get_presentation(env: Env, presentation_id: BytesN<32>) -> VerifiablePresentation {
        Self::get_presentation_or_panic(&env, presentation_id)
    }

    /// List all presentation IDs
    pub fn list_presentation_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&DidKey::AllPresentationIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Verifiable Credentials
    // ========================================================================

    /// Issue a verifiable credential
    pub fn issue_credential(
        env: Env,
        issuer: Address,
        issuer_did: Bytes,
        subject_did: Bytes,
        credential_type: Symbol,
        attributes: Bytes,
        validity_days: u32,
    ) -> VerifiableCredential {
        issuer.require_auth();

        let id = Self::get_next_credential_id(&env);
        let id_bytes = BytesN::from_array(&env, &sha2_digest(&env, &id.to_le_bytes()));
        let now = env.ledger().timestamp();
        let validity_seconds = validity_days * 86400u64;

        let credential = VerifiableCredential {
            id: id_bytes.clone(),
            issuer_did,
            subject_did,
            credential_type,
            attributes,
            issued_at: now,
            expires_at: now + validity_seconds,
            revoked: false,
        };

        env.storage()
            .instance()
            .set(&DidKey::VerifiableCredential(id_bytes.clone()), &credential);

        let mut all_ids: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&DidKey::AllCredentialIds)
            .unwrap_or_else(|| Vec::new(&env));
        all_ids.push_back(id_bytes.clone());
        env.storage()
            .instance()
            .set(&DidKey::AllCredentialIds, &all_ids);

        log!(
            &env,
            "SubmitterDids: credential issued - id={}, type={:?}",
            id,
            credential_type
        );

        credential
    }

    /// Revoke a credential (owner-only)
    pub fn revoke_credential(env: Env, caller: Address, credential_id: BytesN<32>) {
        Self::require_owner(&env, &caller);

        let mut credential = Self::get_credential_or_panic(&env, credential_id.clone());
        credential.revoked = true;

        env.storage()
            .instance()
            .set(&DidKey::VerifiableCredential(credential_id), &credential);
    }

    /// Get a credential by ID
    pub fn get_credential(env: Env, credential_id: BytesN<32>) -> VerifiableCredential {
        Self::get_credential_or_panic(&env, credential_id)
    }

    /// List all credential IDs
    pub fn list_credential_ids(env: Env) -> Vec<BytesN<32>> {
        env.storage()
            .instance()
            .get(&DidKey::AllCredentialIds)
            .unwrap_or_else(|| Vec::new(&env))
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&DidKey::Owner)
            .unwrap_or_else(|| panic_with_error!(env, DidError::Unauthorized));
        if &owner != caller {
            panic_with_error!(env, DidError::Unauthorized);
        }
    }

    fn get_did_or_panic(env: &Env, did: Bytes) -> DidRecord {
        env.storage()
            .instance()
            .get(&DidKey::DidRecord(did))
            .unwrap_or_else(|| panic_with_error!(env, DidError::DidNotFound))
    }

    fn get_message_or_panic(env: &Env, message_id: BytesN<32>) -> DidCommMessage {
        env.storage()
            .instance()
            .get(&DidKey::DidCommMessage(message_id))
            .unwrap_or_else(|| panic_with_error!(env, DidError::MessageNotFound))
    }

    fn get_presentation_or_panic(env: &Env, presentation_id: BytesN<32>) -> VerifiablePresentation {
        env.storage()
            .instance()
            .get(&DidKey::VerifiablePresentation(presentation_id))
            .unwrap_or_else(|| panic_with_error!(env, DidError::PresentationNotFound))
    }

    fn get_credential_or_panic(env: &Env, credential_id: BytesN<32>) -> VerifiableCredential {
        env.storage()
            .instance()
            .get(&DidKey::VerifiableCredential(credential_id))
            .unwrap_or_else(|| panic_with_error!(env, DidError::CredentialNotFound))
    }

    fn get_next_did_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&DidKey::NextDidId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&DidKey::NextDidId, &(current + 1));
        current
    }

    fn get_next_presentation_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&DidKey::NextPresentationId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&DidKey::NextPresentationId, &(current + 1));
        current
    }

    fn get_next_message_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&DidKey::NextMessageId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&DidKey::NextMessageId, &(current + 1));
        current
    }

    fn get_next_credential_id(env: &Env) -> u32 {
        let current = env
            .storage()
            .instance()
            .get::<_, u32>(&DidKey::NextCredentialId)
            .unwrap_or(1);
        env.storage()
            .instance()
            .set(&DidKey::NextCredentialId, &(current + 1));
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
    fn test_did_registration() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let submitter = Address::from_array(&env, &[2; 32]);

        SubmitterDids::initialize(env.clone(), owner.clone());

        let did = Bytes::from_slice(&env, b"did:key:z6MkhaXgBZDvotDkL5257ppizwGqR8P7KxG6Hjpy");
        let doc = Bytes::from_slice(&env, b"{}");

        let record = SubmitterDids::register_did(
            env.clone(),
            submitter,
            did.clone(),
            0,
            doc,
        );
        assert_eq!(record.method, 0);
        assert!(!record.verified);

        let all = SubmitterDids::list_dids(env.clone());
        assert_eq!(all.len(), 1);

        SubmitterDids::verify_did(env.clone(), owner, did);
        let verified = SubmitterDids::resolve_did(env, did);
        assert!(verified.verified);
    }

    #[test]
    fn test_didcomm_message() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let sender = Address::from_array(&env, &[2; 32]);

        SubmitterDids::initialize(env.clone(), owner.clone());

        let from_did = Bytes::from_slice(&env, b"did:key:z6MkhaXgBZDvotDkL5257ppizwGqR8P7KxG6Hjpy");
        let to_did = Bytes::from_slice(&env, b"did:web:example.com");

        let msg = SubmitterDids::send_message(
            env.clone(),
            sender,
            from_did,
            to_did,
            Symbol::new(&env, "message"),
            Bytes::new(&env),
        );
        assert!(!msg.delivered);
    }

    #[test]
    fn test_credential_lifecycle() {
        let env = Env::default();
        let owner = Address::from_array(&env, &[1; 32]);
        let issuer = Address::from_array(&env, &[2; 32]);

        SubmitterDids::initialize(env.clone(), owner.clone());

        let issuer_did = Bytes::from_slice(&env, b"did:key:z6MkhaXgBZDvotDkL5257ppizwGqR8P7KxG6Hjpy");
        let subject_did = Bytes::from_slice(&env, b"did:web:example.com");

        let vc = SubmitterDids::issue_credential(
            env.clone(),
            issuer,
            issuer_did,
            subject_did,
            Symbol::new(&env, "VerifiableCredential"),
            Bytes::new(&env),
            30,
        );
        assert!(!vc.revoked);

        SubmitterDids::revoke_credential(env.clone(), owner, vc.id);
        let revoked = SubmitterDids::get_credential(env, vc.id);
        assert!(revoked.revoked);
    }
}
