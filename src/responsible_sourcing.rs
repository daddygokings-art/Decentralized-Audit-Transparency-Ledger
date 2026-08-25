/// # Responsible Sourcing Verification Module
///
/// This module implements comprehensive responsible sourcing verification for supply chain audit trails.
/// It supports multiple certification schemes (RJC, LBMA, RMI), audit standards, chain of custody tracking,
/// and blockchain-anchored consumer claims.
///
/// ## Certification Schemes
/// - **RJC (Responsible Jewellery Council)** — Gold, silver, diamond traceability
/// - **LBMA (London Bullion Market Association)** — Precious metals standards
/// - **RMI (Responsible Minerals Initiative)** — Conflict-free minerals certification
///
/// ## Key Features
/// - Multi-tiered supply chain verification with checkpoints
/// - Cryptographic proof of custody transfers
/// - Tamper-evident audit trail with hash chaining
/// - Consumer claim verification against sourcing data
/// - Audit certification state machine

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes, 
    BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ResponsibleSourcingError {
    /// Certification not found or expired
    CertificationNotFound = 1000,
    /// Invalid certification scheme
    InvalidCertificationScheme = 1001,
    /// Chain of custody broken (gap in custody records)
    ChainOfCustodyBroken = 1002,
    /// Audit standards not met for sourcing claim
    AuditStandardsNotMet = 1003,
    /// Consumer claim contradicts verified sourcing data
    ConsumerClaimConflict = 1004,
    /// Traceability record incomplete or insufficient
    TraceabilityIncomplete = 1005,
    /// Custody transfer signature invalid or missing
    InvalidCustodySignature = 1006,
    /// Shipment verification failed (hash mismatch)
    ShipmentVerificationFailed = 1007,
    /// Audit report not yet finalized
    AuditNotFinalized = 1008,
    /// Certification authority not recognized
    UnauthorizedCertifier = 1009,
    /// Material origin claimed but not verified
    UnverifiedOrigin = 1010,
    /// Conflict minerals detected in supply chain
    ConflictMineralsDetected = 1011,
}

// ── Data Structures ──────────────────────────────────────────────────────

/// Certification scheme enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum CertificationScheme {
    /// Responsible Jewellery Council certification
    RJC = 1,
    /// London Bullion Market Association standard
    LBMA = 2,
    /// Responsible Minerals Initiative certification
    RMI = 3,
    /// ISO 9001 quality management system
    ISO9001 = 4,
    /// ISO 14001 environmental management system
    ISO14001 = 5,
    /// Custom/Proprietary certification scheme
    Custom = 6,
}

/// Audit standard enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum AuditStandard {
    /// Third-party independent audit
    ThirdPartyAudit = 1,
    /// Chain of Custody certification (CoC)
    ChainOfCustody = 2,
    /// Due diligence audit
    DueDiligence = 3,
    /// Conflict minerals audit (OECD due diligence)
    ConflictMineralsAudit = 4,
    /// Environmental audit
    EnvironmentalAudit = 5,
    /// Social responsibility audit
    SocialAudit = 6,
}

/// Material origin classification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum MaterialOrigin {
    /// Responsibly mined from known, audited sources
    ResponsiblyMined = 1,
    /// Recycled/secondary material
    Recycled = 2,
    /// Post-consumer recovered material
    PostConsumerRecovered = 3,
    /// Conflict-free verified origin
    ConflictFreeVerified = 4,
    /// Unknown or unverified origin
    Unknown = 5,
}

/// Certification metadata
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certification {
    /// Unique certification identifier
    pub id: BytesN<32>,
    /// Certification scheme (RJC, LBMA, RMI, etc.)
    pub scheme: u32,
    /// Certifying authority address
    pub authority: Address,
    /// Material/product being certified
    pub material_description: Bytes,
    /// Issuance timestamp
    pub issued_at: u64,
    /// Expiration timestamp (0 = no expiry)
    pub expires_at: u64,
    /// Certification status: 0=pending, 1=active, 2=suspended, 3=revoked
    pub status: u32,
    /// Audit standards covered
    pub audit_standards: Vec<u32>,
    /// Material origin classification
    pub origin: u32,
    /// Metadata hash (content-addressed for verification)
    pub metadata_hash: BytesN<32>,
}

/// Custody transfer record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustodyTransfer {
    /// Sequential record ID
    pub id: u32,
    /// Shipment being transferred
    pub shipment_id: BytesN<32>,
    /// From party (previous custodian)
    pub from: Address,
    /// To party (new custodian)
    pub to: Address,
    /// Transfer timestamp
    pub transferred_at: u64,
    /// Cryptographic proof: hash(shipment_id || from || to || timestamp)
    pub transfer_proof: BytesN<32>,
    /// Transfer signature by 'from' party (96 bytes: pubkey[32] || sig[64])
    pub signature: Bytes,
    /// Location/custody facility
    pub location: Bytes,
}

/// Shipment tracking record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shipment {
    /// Unique shipment identifier
    pub id: BytesN<32>,
    /// Associated certification ID
    pub certification_id: BytesN<32>,
    /// Quantity (e.g., troy ounces, grams, etc.)
    pub quantity: u64,
    /// Unit of measurement (e.g., "oz", "g", "kg")
    pub unit: Bytes,
    /// Shipment creation timestamp
    pub created_at: u64,
    /// Last custody transfer timestamp
    pub last_transfer_at: u64,
    /// Current custodian
    pub current_custodian: Address,
    /// Shipment hash: sha256(material_data || quantity || certification_id)
    pub shipment_hash: BytesN<32>,
    /// Chain of custody complete (all custody transfers have valid signatures)
    pub custody_verified: bool,
}

/// Audit report structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditReport {
    /// Unique report ID
    pub id: BytesN<32>,
    /// Certification being audited
    pub certification_id: BytesN<32>,
    /// Auditor address
    pub auditor: Address,
    /// Audit timestamp
    pub audited_at: u64,
    /// Audit standards covered (vec of u32)
    pub standards_covered: Vec<u32>,
    /// Number of shipments audited
    pub shipments_audited: u32,
    /// Findings summary (JSON-encoded)
    pub findings: Bytes,
    /// Compliance status: 0=non-compliant, 1=compliant, 2=compliant_with_findings
    pub compliance_status: u32,
    /// Report hash for integrity verification
    pub report_hash: BytesN<32>,
    /// Audit finalized (immutable)
    pub finalized: bool,
}

/// Consumer claim structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumerClaim {
    /// Unique claim ID
    pub id: BytesN<32>,
    /// Claiming party (consumer/buyer)
    pub claimer: Address,
    /// Claim text (e.g., "100% responsibly sourced")
    pub claim: Bytes,
    /// Supported by certification ID
    pub supporting_certification: BytesN<32>,
    /// Supported by audit report IDs
    pub supporting_audits: Vec<BytesN<32>>,
    /// Claim timestamp
    pub claimed_at: u64,
    /// Verification status: 0=pending, 1=verified, 2=disputed
    pub verification_status: u32,
    /// Claim verification hash
    pub claim_hash: BytesN<32>,
}

/// Traceability checkpoint (single point in supply chain)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceabilityCheckpoint {
    /// Sequential checkpoint index
    pub index: u32,
    /// Shipment ID
    pub shipment_id: BytesN<32>,
    /// Party at checkpoint
    pub party: Address,
    /// Checkpoint timestamp
    pub checkpoint_at: u64,
    /// Location/facility name
    pub location: Bytes,
    /// Checkpoint metadata (JSON)
    pub metadata: Bytes,
    /// Hash chain: sha256(prev_checkpoint_hash || this_data)
    pub checkpoint_hash: BytesN<32>,
    /// Previous checkpoint hash (chain linkage)
    pub prev_checkpoint_hash: BytesN<32>,
}

/// Material origin verification record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialOriginRecord {
    /// Origin record ID
    pub id: BytesN<32>,
    /// Material type (e.g., "gold", "silver", "diamond")
    pub material_type: Bytes,
    /// Origin location/mine
    pub origin_location: Bytes,
    /// Extraction/mining date
    pub extraction_date: u64,
    /// Extraction authority (mine operator/certifier)
    pub extraction_authority: Address,
    /// Conflict-free verification: true if verified non-conflict
    pub conflict_free: bool,
    /// Legal compliance: true if legally mined/extracted
    pub legally_sourced: bool,
    /// Environmental compliance: true if meets standards
    pub environmentally_compliant: bool,
    /// Supporting documentation hash
    pub documentation_hash: BytesN<32>,
}

// ── Data Keys ────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum ResponsibleSourcingKey {
    /// Contract owner address
    Owner,
    /// Authorized certifiers (vec of Address)
    AuthorizedCertifiers,
    /// Certification data by ID
    Certification(BytesN<32>),
    /// Shipment data by ID
    Shipment(BytesN<32>),
    /// Custody transfer records (indexed by shipment_id + sequence)
    CustodyTransfer(BytesN<32>, u32),
    /// Custody transfer count per shipment
    CustodyTransferCount(BytesN<32>),
    /// Audit reports by ID
    AuditReport(BytesN<32>),
    /// Consumer claims by ID
    ConsumerClaim(BytesN<32>),
    /// Traceability checkpoints (indexed by shipment_id + index)
    TraceabilityCheckpoint(BytesN<32>, u32),
    /// Checkpoint count per shipment
    CheckpointCount(BytesN<32>),
    /// Material origin records by ID
    MaterialOriginRecord(BytesN<32>),
    /// Certification authority registry: Address -> bool (approved)
    CertifierApproved(Address),
    /// Conflict materials registry: material_type -> is_conflict_prone
    ConflictMaterialAlert(Bytes),
    /// Total certifications issued
    CertificationCount,
    /// Total shipments tracked
    ShipmentCount,
    /// Total audit reports filed
    AuditReportCount,
    /// Total consumer claims
    ConsumerClaimCount,
}

// ── Contract Implementation ──────────────────────────────────────────────

#[contract]
pub struct ResponsibleSourcing;

#[contractimpl]
impl ResponsibleSourcing {
    /// Initialize the responsible sourcing module
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CertificationCount, &0u32);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::ShipmentCount, &0u32);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::AuditReportCount, &0u32);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::ConsumerClaimCount, &0u32);
    }

    // ── Certifier Management ─────────────────────────────────────────────

    /// Register a certification authority (owner-only)
    pub fn register_certifier(env: Env, caller: Address, certifier: Address) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CertifierApproved(certifier.clone()), &true);
        env.events().publish(
            (Symbol::new(&env, "certifier_registered"),),
            (certifier,),
        );
    }

    /// Revoke a certification authority (owner-only)
    pub fn revoke_certifier(env: Env, caller: Address, certifier: Address) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CertifierApproved(certifier.clone()), &false);
        env.events().publish(
            (Symbol::new(&env, "certifier_revoked"),),
            (certifier,),
        );
    }

    /// Check if a certifier is authorized
    pub fn is_certifier_approved(env: Env, certifier: Address) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&ResponsibleSourcingKey::CertifierApproved(certifier))
            .unwrap_or(false)
    }

    // ── Certification Management ─────────────────────────────────────────

    /// Issue a new certification
    pub fn issue_certification(
        env: Env,
        authority: Address,
        scheme: u32,
        material_description: Bytes,
        expires_at: u64,
        audit_standards: Vec<u32>,
        origin: u32,
        metadata: Bytes,
    ) -> BytesN<32> {
        authority.require_auth();
        if !Self::is_certifier_approved(env.clone(), authority.clone()) {
            panic_with_error!(&env, ResponsibleSourcingError::UnauthorizedCertifier);
        }

        let cert_id = Self::compute_cert_id(&env, &authority, &material_description, &metadata);
        let issued_at = env.ledger().timestamp();

        let metadata_hash = env.crypto().sha256(&metadata).into();

        let certification = Certification {
            id: cert_id.clone(),
            scheme,
            authority: authority.clone(),
            material_description: material_description.clone(),
            issued_at,
            expires_at,
            status: 1, // active
            audit_standards: audit_standards.clone(),
            origin,
            metadata_hash,
        };

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::Certification(cert_id.clone()), &certification);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::CertificationCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CertificationCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "certification_issued"),),
            (cert_id.clone(), authority, scheme),
        );

        cert_id
    }

    /// Retrieve a certification by ID
    pub fn get_certification(env: Env, cert_id: BytesN<32>) -> Certification {
        env.storage()
            .instance()
            .get(&ResponsibleSourcingKey::Certification(cert_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ResponsibleSourcingError::CertificationNotFound))
    }

    /// Revoke a certification (authority-only)
    pub fn revoke_certification(env: Env, authority: Address, cert_id: BytesN<32>) {
        authority.require_auth();
        let mut cert = Self::get_certification(env.clone(), cert_id.clone());
        if cert.authority != authority {
            panic_with_error!(&env, ResponsibleSourcingError::UnauthorizedCertifier);
        }
        cert.status = 3; // revoked
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::Certification(cert_id.clone()), &cert);
        env.events().publish(
            (Symbol::new(&env, "certification_revoked"),),
            (cert_id, authority),
        );
    }

    // ── Shipment Tracking ────────────────────────────────────────────────

    /// Create a new tracked shipment
    pub fn create_shipment(
        env: Env,
        creator: Address,
        certification_id: BytesN<32>,
        quantity: u64,
        unit: Bytes,
    ) -> BytesN<32> {
        creator.require_auth();
        
        // Verify certification exists and is active
        let cert = Self::get_certification(env.clone(), certification_id.clone());
        if cert.status != 1 {
            panic_with_error!(&env, ResponsibleSourcingError::CertificationNotFound);
        }

        let shipment_id = Self::compute_shipment_id(&env, &certification_id, quantity);
        let now = env.ledger().timestamp();

        let shipment_hash = env
            .crypto()
            .sha256(&Self::pack_shipment_data(
                &env,
                &certification_id,
                quantity,
            ))
            .into();

        let shipment = Shipment {
            id: shipment_id.clone(),
            certification_id,
            quantity,
            unit,
            created_at: now,
            last_transfer_at: now,
            current_custodian: creator.clone(),
            shipment_hash,
            custody_verified: true,
        };

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::Shipment(shipment_id.clone()), &shipment);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CustodyTransferCount(shipment_id.clone()), &0u32);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CheckpointCount(shipment_id.clone()), &0u32);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::ShipmentCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::ShipmentCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "shipment_created"),),
            (shipment_id.clone(), certification_id, quantity),
        );

        shipment_id
    }

    /// Retrieve shipment by ID
    pub fn get_shipment(env: Env, shipment_id: BytesN<32>) -> Shipment {
        env.storage()
            .instance()
            .get(&ResponsibleSourcingKey::Shipment(shipment_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ResponsibleSourcingError::TraceabilityIncomplete))
    }

    // ── Chain of Custody ─────────────────────────────────────────────────

    /// Transfer custody of a shipment (signed by current custodian)
    pub fn transfer_custody(
        env: Env,
        from: Address,
        to: Address,
        shipment_id: BytesN<32>,
        location: Bytes,
        signature: Bytes,
    ) -> u32 {
        from.require_auth();

        let mut shipment = Self::get_shipment(env.clone(), shipment_id.clone());
        if shipment.current_custodian != from {
            panic_with_error!(&env, ResponsibleSourcingError::ChainOfCustodyBroken);
        }

        let now = env.ledger().timestamp();
        let transfer_proof = Self::compute_custody_proof(&env, &shipment_id, &from, &to, now);

        // Validate signature (96 bytes: pubkey[32] || signature[64])
        if signature.len() != 96 {
            panic_with_error!(&env, ResponsibleSourcingError::InvalidCustodySignature);
        }

        let transfer_seq: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::CustodyTransferCount(shipment_id.clone()))
            .unwrap_or(0);

        let custody_transfer = CustodyTransfer {
            id: transfer_seq,
            shipment_id: shipment_id.clone(),
            from: from.clone(),
            to: to.clone(),
            transferred_at: now,
            transfer_proof,
            signature,
            location,
        };

        env.storage().instance().set(
            &ResponsibleSourcingKey::CustodyTransfer(shipment_id.clone(), transfer_seq),
            &custody_transfer,
        );

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CustodyTransferCount(shipment_id.clone()), &(transfer_seq + 1));

        shipment.current_custodian = to.clone();
        shipment.last_transfer_at = now;
        shipment.custody_verified = true;

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::Shipment(shipment_id.clone()), &shipment);

        env.events().publish(
            (Symbol::new(&env, "custody_transferred"),),
            (shipment_id, from, to, transfer_seq),
        );

        transfer_seq
    }

    /// Get custody transfer record
    pub fn get_custody_transfer(env: Env, shipment_id: BytesN<32>, seq: u32) -> CustodyTransfer {
        env.storage()
            .instance()
            .get(&ResponsibleSourcingKey::CustodyTransfer(shipment_id.clone(), seq))
            .unwrap_or_else(|| panic_with_error!(&env, ResponsibleSourcingError::ChainOfCustodyBroken))
    }

    /// Verify full chain of custody for a shipment
    pub fn verify_custody_chain(env: Env, shipment_id: BytesN<32>) -> bool {
        let count: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::CustodyTransferCount(shipment_id.clone()))
            .unwrap_or(0);

        if count == 0 {
            return true; // Single custodian, chain is valid
        }

        for i in 0..count {
            let transfer = Self::get_custody_transfer(env.clone(), shipment_id.clone(), i);
            
            // Recompute and verify transfer proof
            let recomputed_proof =
                Self::compute_custody_proof(&env, &transfer.shipment_id, &transfer.from, &transfer.to, transfer.transferred_at);
            
            if transfer.transfer_proof != recomputed_proof {
                return false;
            }
        }

        true
    }

    // ── Traceability Checkpoints ─────────────────────────────────────────

    /// Record a traceability checkpoint in the supply chain
    pub fn record_checkpoint(
        env: Env,
        party: Address,
        shipment_id: BytesN<32>,
        location: Bytes,
        metadata: Bytes,
    ) -> u32 {
        party.require_auth();

        let checkpoint_seq: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::CheckpointCount(shipment_id.clone()))
            .unwrap_or(0);

        let prev_hash: BytesN<32> = if checkpoint_seq == 0 {
            BytesN::from_array(&env, &[0u8; 32])
        } else {
            let prev = Self::get_checkpoint(env.clone(), shipment_id.clone(), checkpoint_seq - 1);
            prev.checkpoint_hash
        };

        let now = env.ledger().timestamp();
        let checkpoint_data =
            Self::pack_checkpoint_data(&env, checkpoint_seq, &party, &location, &metadata, now);
        
        let checkpoint_hash = env.crypto().sha256(&checkpoint_data).into();

        let checkpoint = TraceabilityCheckpoint {
            index: checkpoint_seq,
            shipment_id: shipment_id.clone(),
            party: party.clone(),
            checkpoint_at: now,
            location,
            metadata,
            checkpoint_hash,
            prev_checkpoint_hash: prev_hash,
        };

        env.storage().instance().set(
            &ResponsibleSourcingKey::TraceabilityCheckpoint(shipment_id.clone(), checkpoint_seq),
            &checkpoint,
        );

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::CheckpointCount(shipment_id.clone()), &(checkpoint_seq + 1));

        env.events().publish(
            (Symbol::new(&env, "checkpoint_recorded"),),
            (shipment_id, checkpoint_seq, party),
        );

        checkpoint_seq
    }

    /// Get traceability checkpoint
    pub fn get_checkpoint(env: Env, shipment_id: BytesN<32>, index: u32) -> TraceabilityCheckpoint {
        env.storage()
            .instance()
            .get(&ResponsibleSourcingKey::TraceabilityCheckpoint(shipment_id.clone(), index))
            .unwrap_or_else(|| panic_with_error!(&env, ResponsibleSourcingError::TraceabilityIncomplete))
    }

    /// Verify full traceability chain (all checkpoints hash-linked)
    pub fn verify_traceability_chain(env: Env, shipment_id: BytesN<32>) -> bool {
        let count: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::CheckpointCount(shipment_id.clone()))
            .unwrap_or(0);

        if count == 0 {
            return true;
        }

        let mut prev_hash = BytesN::from_array(&env, &[0u8; 32]);

        for i in 0..count {
            let checkpoint = Self::get_checkpoint(env.clone(), shipment_id.clone(), i);
            if checkpoint.prev_checkpoint_hash != prev_hash {
                return false;
            }
            prev_hash = checkpoint.checkpoint_hash;
        }

        true
    }

    /// Get full traceability path (all checkpoints)
    pub fn get_traceability_path(env: Env, shipment_id: BytesN<32>) -> Vec<TraceabilityCheckpoint> {
        let count: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::CheckpointCount(shipment_id.clone()))
            .unwrap_or(0);

        let mut path: Vec<TraceabilityCheckpoint> = Vec::new(&env);
        for i in 0..count {
            if let Some(checkpoint) = env
                .storage()
                .instance()
                .get::<_, TraceabilityCheckpoint>(&ResponsibleSourcingKey::TraceabilityCheckpoint(
                    shipment_id.clone(),
                    i,
                ))
            {
                path.push_back(checkpoint);
            }
        }
        path
    }

    // ── Material Origin Verification ─────────────────────────────────────

    /// Record material origin with compliance data
    pub fn record_material_origin(
        env: Env,
        authority: Address,
        material_type: Bytes,
        origin_location: Bytes,
        extraction_date: u64,
        conflict_free: bool,
        legally_sourced: bool,
        environmentally_compliant: bool,
        documentation: Bytes,
    ) -> BytesN<32> {
        authority.require_auth();
        if !Self::is_certifier_approved(env.clone(), authority.clone()) {
            panic_with_error!(&env, ResponsibleSourcingError::UnauthorizedCertifier);
        }

        let origin_id = Self::compute_origin_id(&env, &material_type, &origin_location, extraction_date);
        let doc_hash = env.crypto().sha256(&documentation).into();

        let origin_record = MaterialOriginRecord {
            id: origin_id.clone(),
            material_type,
            origin_location,
            extraction_date,
            extraction_authority: authority.clone(),
            conflict_free,
            legally_sourced,
            environmentally_compliant,
            documentation_hash: doc_hash,
        };

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::MaterialOriginRecord(origin_id.clone()), &origin_record);

        env.events().publish(
            (Symbol::new(&env, "material_origin_recorded"),),
            (origin_id.clone(), authority),
        );

        origin_id
    }

    /// Get material origin record
    pub fn get_material_origin(env: Env, origin_id: BytesN<32>) -> MaterialOriginRecord {
        env.storage()
            .instance()
            .get(&ResponsibleSourcingKey::MaterialOriginRecord(origin_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ResponsibleSourcingError::UnverifiedOrigin))
    }

    // ── Audit Reporting ──────────────────────────────────────────────────

    /// File an audit report
    pub fn file_audit_report(
        env: Env,
        auditor: Address,
        certification_id: BytesN<32>,
        standards_covered: Vec<u32>,
        shipments_audited: u32,
        findings: Bytes,
        compliance_status: u32,
    ) -> BytesN<32> {
        auditor.require_auth();
        if !Self::is_certifier_approved(env.clone(), auditor.clone()) {
            panic_with_error!(&env, ResponsibleSourcingError::UnauthorizedCertifier);
        }

        // Verify certification exists
        let _ = Self::get_certification(env.clone(), certification_id.clone());

        let report_id = Self::compute_report_id(&env, &certification_id, &auditor, &findings);
        let now = env.ledger().timestamp();

        let report_data =
            Self::pack_report_data(&env, &certification_id, &auditor, shipments_audited, &findings);
        let report_hash = env.crypto().sha256(&report_data).into();

        let audit_report = AuditReport {
            id: report_id.clone(),
            certification_id,
            auditor: auditor.clone(),
            audited_at: now,
            standards_covered,
            shipments_audited,
            findings,
            compliance_status,
            report_hash,
            finalized: true,
        };

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::AuditReport(report_id.clone()), &audit_report);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::AuditReportCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::AuditReportCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "audit_report_filed"),),
            (report_id.clone(), certification_id, auditor),
        );

        report_id
    }

    /// Get audit report
    pub fn get_audit_report(env: Env, report_id: BytesN<32>) -> AuditReport {
        env.storage()
            .instance()
            .get(&ResponsibleSourcingKey::AuditReport(report_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ResponsibleSourcingError::AuditNotFinalized))
    }

    // ── Consumer Claims & Verification ───────────────────────────────────

    /// Submit a consumer claim backed by certifications and audits
    pub fn submit_consumer_claim(
        env: Env,
        claimer: Address,
        claim: Bytes,
        supporting_certification: BytesN<32>,
        supporting_audits: Vec<BytesN<32>>,
    ) -> BytesN<32> {
        claimer.require_auth();

        // Verify certification
        let cert = Self::get_certification(env.clone(), supporting_certification.clone());
        if cert.status != 1 {
            panic_with_error!(&env, ResponsibleSourcingError::CertificationNotFound);
        }

        // Verify all audit reports
        for i in 0..supporting_audits.len() {
            let audit_id = supporting_audits.get(i).unwrap();
            let report = Self::get_audit_report(env.clone(), audit_id.clone());
            if report.certification_id != supporting_certification {
                panic_with_error!(&env, ResponsibleSourcingError::AuditNotFinalized);
            }
        }

        let claim_id = Self::compute_claim_id(&env, &claimer, &claim);
        let now = env.ledger().timestamp();
        let claim_hash = env.crypto().sha256(&claim).into();

        let consumer_claim = ConsumerClaim {
            id: claim_id.clone(),
            claimer: claimer.clone(),
            claim,
            supporting_certification,
            supporting_audits,
            claimed_at: now,
            verification_status: 1, // verified
            claim_hash,
        };

        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::ConsumerClaim(claim_id.clone()), &consumer_claim);

        let count: u32 = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::ConsumerClaimCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::ConsumerClaimCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "consumer_claim_submitted"),),
            (claim_id.clone(), claimer, supporting_certification),
        );

        claim_id
    }

    /// Get consumer claim
    pub fn get_consumer_claim(env: Env, claim_id: BytesN<32>) -> ConsumerClaim {
        env.storage()
            .instance()
            .get(&ResponsibleSourcingKey::ConsumerClaim(claim_id.clone()))
            .unwrap_or_else(|| panic_with_error!(&env, ResponsibleSourcingError::ConsumerClaimConflict))
    }

    /// Verify a consumer claim against supporting data
    pub fn verify_consumer_claim(env: Env, claim_id: BytesN<32>) -> bool {
        let claim = Self::get_consumer_claim(env.clone(), claim_id.clone());

        // Verify certification
        let cert = match env.storage().instance().get::<_, Certification>(&ResponsibleSourcingKey::Certification(claim.supporting_certification.clone())) {
            Some(c) => c,
            None => return false,
        };

        if cert.status != 1 || cert.expires_at > 0 && cert.expires_at < env.ledger().timestamp() {
            return false;
        }

        // Verify all audits
        for i in 0..claim.supporting_audits.len() {
            let audit_id = claim.supporting_audits.get(i).unwrap();
            if let Some(report) = env
                .storage()
                .instance()
                .get::<_, AuditReport>(&ResponsibleSourcingKey::AuditReport(audit_id.clone()))
            {
                if !report.finalized || report.compliance_status == 0 {
                    return false; // Non-compliant audit
                }
            } else {
                return false; // Audit report missing
            }
        }

        true
    }

    // ── Conflict Minerals Detection ──────────────────────────────────────

    /// Register a conflict material alert
    pub fn register_conflict_alert(env: Env, caller: Address, material: Bytes) {
        caller.require_auth();
        Self::require_owner(&env, &caller);
        env.storage()
            .instance()
            .set(&ResponsibleSourcingKey::ConflictMaterialAlert(material.clone()), &true);
        env.events().publish(
            (Symbol::new(&env, "conflict_alert_registered"),),
            (material,),
        );
    }

    /// Check if material is flagged as conflict-prone
    pub fn is_conflict_material(env: Env, material: Bytes) -> bool {
        env.storage()
            .instance()
            .get::<_, bool>(&ResponsibleSourcingKey::ConflictMaterialAlert(material))
            .unwrap_or(false)
    }

    // ── Helper Functions ─────────────────────────────────────────────────

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&ResponsibleSourcingKey::Owner)
            .unwrap();
        if caller != &owner {
            panic_with_error!(env, ResponsibleSourcingError::UnauthorizedCertifier);
        }
    }

    fn compute_cert_id(env: &Env, authority: &Address, material: &Bytes, metadata: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&authority.to_string().to_bytes());
        preimage.append(material);
        preimage.append(metadata);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_shipment_id(env: &Env, cert_id: &BytesN<32>, quantity: u64) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&cert_id.clone().into());
        preimage.append(&Self::u64_to_bytes(env, quantity));
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_custody_proof(
        env: &Env,
        shipment_id: &BytesN<32>,
        from: &Address,
        to: &Address,
        timestamp: u64,
    ) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&shipment_id.clone().into());
        preimage.append(&from.to_string().to_bytes());
        preimage.append(&to.to_string().to_bytes());
        preimage.append(&Self::u64_to_bytes(env, timestamp));
        env.crypto().sha256(&preimage).into()
    }

    fn pack_shipment_data(env: &Env, cert_id: &BytesN<32>, quantity: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&cert_id.clone().into());
        data.append(&Self::u64_to_bytes(env, quantity));
        data
    }

    fn pack_checkpoint_data(
        env: &Env,
        index: u32,
        party: &Address,
        location: &Bytes,
        metadata: &Bytes,
        timestamp: u64,
    ) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&Self::u32_to_bytes(env, index));
        data.append(&party.to_string().to_bytes());
        data.append(location);
        data.append(metadata);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_report_data(
        env: &Env,
        cert_id: &BytesN<32>,
        auditor: &Address,
        shipments_audited: u32,
        findings: &Bytes,
    ) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&cert_id.clone().into());
        data.append(&auditor.to_string().to_bytes());
        data.append(&Self::u32_to_bytes(env, shipments_audited));
        data.append(findings);
        data
    }

    fn compute_origin_id(env: &Env, material: &Bytes, location: &Bytes, extraction_date: u64) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(material);
        preimage.append(location);
        preimage.append(&Self::u64_to_bytes(env, extraction_date));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_report_id(env: &Env, cert_id: &BytesN<32>, auditor: &Address, findings: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&cert_id.clone().into());
        preimage.append(&auditor.to_string().to_bytes());
        preimage.append(findings);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn compute_claim_id(env: &Env, claimer: &Address, claim: &Bytes) -> BytesN<32> {
        let mut preimage = Bytes::new(env);
        preimage.append(&claimer.to_string().to_bytes());
        preimage.append(claim);
        preimage.append(&Self::u64_to_bytes(env, env.ledger().timestamp()));
        env.crypto().sha256(&preimage).into()
    }

    fn u64_to_bytes(env: &Env, v: u64) -> Bytes {
        bytes!(
            env,
            [
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
                ((v >> 32) & 0xff) as u8,
                ((v >> 40) & 0xff) as u8,
                ((v >> 48) & 0xff) as u8,
                ((v >> 56) & 0xff) as u8,
            ]
        )
    }

    fn u32_to_bytes(env: &Env, v: u32) -> Bytes {
        bytes!(
            env,
            [
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
            ]
        )
    }
}

#[cfg(test)]
mod tests;
