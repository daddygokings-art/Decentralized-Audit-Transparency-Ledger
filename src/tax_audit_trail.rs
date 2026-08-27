//! Tax Audit Trail and Documentation
//!
//! Manages tax audit events, compliance documentation, and decision trails

use soroban_sdk::{contracttype, Env, Address, Symbol, Bytes, Vec, BytesN};
use crate::tax::TaxAuditEvent;

/// Tax Audit Log Entry
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxAuditLogEntry {
    /// Entry ID
    pub id: BytesN<32>,
    /// Reference to original event
    pub event_id: BytesN<32>,
    /// Event type (vat_determined, dst_calculated, etc.)
    pub event_type: Symbol,
    /// Entity involved
    pub entity: Address,
    /// Actor (who made decision)
    pub actor: Address,
    /// Action taken
    pub action: Symbol,
    /// Timestamp
    pub timestamp: u64,
    /// Details about the decision
    pub details: Bytes,
    /// Supporting documentation
    pub documentation: Vec<BytesN<32>>,
    /// Audit trail version (for immutability proof)
    pub version: u32,
}

/// Tax Documentation Record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxDocumentation {
    /// Document ID
    pub id: BytesN<32>,
    /// Related tax event
    pub related_event: BytesN<32>,
    /// Document type
    pub document_type: Symbol, // "vat_return", "transfer_pricing_doc", etc.
    /// Document content hash
    pub content_hash: BytesN<32>,
    /// Effective date
    pub effective_date: u64,
    /// Expiry date (retention period)
    pub expiry_date: u64,
    /// Filing status
    pub filing_status: u32, // 0=draft, 1=submitted, 2=filed, 3=accepted
    /// Jurisdiction
    pub jurisdiction: Symbol,
    /// Authority reference (if filed)
    pub authority_reference: Option<Bytes>,
}

/// Tax Compliance Event
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxComplianceEvent {
    /// Event ID
    pub id: BytesN<32>,
    /// Entity being checked
    pub entity: Address,
    /// Event timestamp
    pub timestamp: u64,
    /// Event type (filing, calculation, audit, etc.)
    pub event_type: Symbol,
    /// Compliance requirement
    pub requirement: Symbol,
    /// Status (compliant, non-compliant, pending)
    pub status: u32,
    /// Due date
    pub due_date: u64,
    /// Actual completion date (if applicable)
    pub completion_date: Option<u64>,
    /// Notes
    pub notes: Bytes,
}

/// Tax Determination Decision
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxDeterminationDecision {
    /// Decision ID
    pub id: BytesN<32>,
    /// Transaction being decided
    pub transaction_id: BytesN<32>,
    /// Tax type (VAT, DST, etc.)
    pub tax_type: Symbol,
    /// Jurisdiction
    pub jurisdiction: Symbol,
    /// Decision (rate, amount, applicability)
    pub decision: Bytes,
    /// Decision basis/reasoning
    pub basis: Bytes,
    /// Supporting rules/regulations
    pub regulations: Vec<Bytes>,
    /// Decision date
    pub decision_date: u64,
    /// Authority making decision
    pub decision_authority: Address,
    /// Appeal period end date
    pub appeal_end_date: u64,
    /// Status (provisional, final, appealed)
    pub status: u32,
}

/// Tax Exemption Record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaxExemptionRecord {
    /// Record ID
    pub id: BytesN<32>,
    /// Entity claiming exemption
    pub entity: Address,
    /// Exemption type
    pub exemption_type: Symbol,
    /// Jurisdiction
    pub jurisdiction: Symbol,
    /// Effective date
    pub effective_date: u64,
    /// Expiry date
    pub expiry_date: u64,
    /// Exemption amount (if applicable)
    pub amount: u64,
    /// Conditions
    pub conditions: Bytes,
    /// Certification/authority approval
    pub certification: Option<BytesN<32>>,
    /// Status (active, expired, revoked)
    pub status: u32,
}

/// Helper for tax audit trail management
pub struct TaxAuditTrailHelper;

impl TaxAuditTrailHelper {
    /// Record a VAT determination decision
    pub fn record_vat_determination(
        env: &Env,
        transaction_id: BytesN<32>,
        jurisdiction: Symbol,
        rate: u32,
        amount: u64,
        exemption: bool,
        actor: Address,
    ) -> TaxAuditLogEntry {
        let mut details = Bytes::new(env);
        details = details.try_extend_from_slice(&rate.to_le_bytes()).unwrap();

        TaxAuditLogEntry {
            id: BytesN::from_array([0u8; 32]),
            event_id: transaction_id,
            event_type: Symbol::new(env, "vat_determined"),
            entity: actor.clone(),
            actor,
            action: Symbol::new(env, if exemption { "exempt" } else { "standard_rate" }),
            timestamp: env.ledger().timestamp(),
            details,
            documentation: Vec::new(env),
            version: 1,
        }
    }

    /// Record a DST calculation
    pub fn record_dst_calculation(
        env: &Env,
        transaction_id: BytesN<32>,
        jurisdiction: Symbol,
        applicable: bool,
        rate: u32,
        amount: u64,
        actor: Address,
    ) -> TaxAuditLogEntry {
        TaxAuditLogEntry {
            id: BytesN::from_array([0u8; 32]),
            event_id: transaction_id,
            event_type: Symbol::new(env, "dst_calculated"),
            entity: actor.clone(),
            actor,
            action: Symbol::new(env, if applicable { "applicable" } else { "not_applicable" }),
            timestamp: env.ledger().timestamp(),
            details: Bytes::new(env),
            documentation: Vec::new(env),
            version: 1,
        }
    }

    /// Record a crypto transaction
    pub fn record_crypto_transaction(
        env: &Env,
        transaction_id: BytesN<32>,
        holder: Address,
        transaction_type: Symbol,
        gain_loss: i64,
        reportable: bool,
        actor: Address,
    ) -> TaxAuditLogEntry {
        TaxAuditLogEntry {
            id: BytesN::from_array([0u8; 32]),
            event_id: transaction_id,
            event_type: Symbol::new(env, "crypto_transaction"),
            entity: holder,
            actor,
            action: Symbol::new(env, if reportable { "reportable" } else { "non_reportable" }),
            timestamp: env.ledger().timestamp(),
            details: Bytes::new(env),
            documentation: Vec::new(env),
            version: 1,
        }
    }

    /// Record a transfer pricing analysis
    pub fn record_transfer_pricing(
        env: &Env,
        transaction_id: BytesN<32>,
        jurisdiction: Symbol,
        defensible: bool,
        variance: i64,
        actor: Address,
    ) -> TaxAuditLogEntry {
        TaxAuditLogEntry {
            id: BytesN::from_array([0u8; 32]),
            event_id: transaction_id,
            event_type: Symbol::new(env, "transfer_pricing"),
            entity: actor.clone(),
            actor,
            action: Symbol::new(env, if defensible { "defensible" } else { "adjustment_needed" }),
            timestamp: env.ledger().timestamp(),
            details: Bytes::new(env),
            documentation: Vec::new(env),
            version: 1,
        }
    }

    /// Record CbCR filing
    pub fn record_cbcr_filing(
        env: &Env,
        report_id: BytesN<32>,
        entity: Address,
        fiscal_year: u32,
        actor: Address,
    ) -> TaxAuditLogEntry {
        TaxAuditLogEntry {
            id: BytesN::from_array([0u8; 32]),
            event_id: report_id,
            event_type: Symbol::new(env, "cbcr_filed"),
            entity,
            actor,
            action: Symbol::new(env, "filed"),
            timestamp: env.ledger().timestamp(),
            details: Bytes::new(env),
            documentation: Vec::new(env),
            version: 1,
        }
    }

    /// Create documentation record for audit
    pub fn create_documentation(
        env: &Env,
        event_id: BytesN<32>,
        document_type: Symbol,
        content_hash: BytesN<32>,
        jurisdiction: Symbol,
        retention_years: u32,
    ) -> TaxDocumentation {
        let now = env.ledger().timestamp();
        // retention in seconds: years * 365.25 * 24 * 3600
        let retention_seconds = (retention_years as u64) * 31_557_600;

        TaxDocumentation {
            id: BytesN::from_array([0u8; 32]),
            related_event: event_id,
            document_type,
            content_hash,
            effective_date: now,
            expiry_date: now + retention_seconds,
            filing_status: 0, // draft
            jurisdiction,
            authority_reference: None,
        }
    }

    /// Create tax compliance event
    pub fn create_compliance_event(
        env: &Env,
        entity: Address,
        event_type: Symbol,
        requirement: Symbol,
        due_date: u64,
    ) -> TaxComplianceEvent {
        TaxComplianceEvent {
            id: BytesN::from_array([0u8; 32]),
            entity,
            timestamp: env.ledger().timestamp(),
            event_type,
            requirement,
            status: 0, // pending
            due_date,
            completion_date: None,
            notes: Bytes::new(env),
        }
    }

    /// Create tax determination decision
    pub fn create_determination_decision(
        env: &Env,
        transaction_id: BytesN<32>,
        tax_type: Symbol,
        jurisdiction: Symbol,
        decision: Bytes,
        basis: Bytes,
        authority: Address,
    ) -> TaxDeterminationDecision {
        let now = env.ledger().timestamp();
        // Appeal period: typically 30 days
        let appeal_end = now + (30 * 24 * 3600);

        TaxDeterminationDecision {
            id: BytesN::from_array([0u8; 32]),
            transaction_id,
            tax_type,
            jurisdiction,
            decision,
            basis,
            regulations: Vec::new(env),
            decision_date: now,
            decision_authority: authority,
            appeal_end_date: appeal_end,
            status: 1, // final
        }
    }

    /// Create tax exemption record
    pub fn create_exemption_record(
        env: &Env,
        entity: Address,
        exemption_type: Symbol,
        jurisdiction: Symbol,
        amount: u64,
        expiry_years: u32,
    ) -> TaxExemptionRecord {
        let now = env.ledger().timestamp();
        let expiry_seconds = (expiry_years as u64) * 31_557_600;

        TaxExemptionRecord {
            id: BytesN::from_array([0u8; 32]),
            entity,
            exemption_type,
            jurisdiction,
            effective_date: now,
            expiry_date: now + expiry_seconds,
            amount,
            conditions: Bytes::new(env),
            certification: None,
            status: 0, // active
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vat_determination_logging() {
        let env = Env::default();
        let actor = Address::random(&env);
        
        let log = TaxAuditTrailHelper::record_vat_determination(
            &env,
            BytesN::from_array([0u8; 32]),
            Symbol::new(&env, "EU"),
            2000,
            200,
            false,
            actor.clone(),
        );

        assert_eq!(log.event_type.to_string(), "vat_determined");
        assert_eq!(log.version, 1);
    }

    #[test]
    fn test_documentation_creation() {
        let env = Env::default();
        let doc = TaxAuditTrailHelper::create_documentation(
            &env,
            BytesN::from_array([0u8; 32]),
            Symbol::new(&env, "vat_return"),
            BytesN::from_array([1u8; 32]),
            Symbol::new(&env, "EU"),
            6,
        );

        assert_eq!(doc.filing_status, 0);
        assert!(doc.expiry_date > doc.effective_date);
    }

    #[test]
    fn test_exemption_record() {
        let env = Env::default();
        let entity = Address::random(&env);
        
        let exemption = TaxAuditTrailHelper::create_exemption_record(
            &env,
            entity.clone(),
            Symbol::new(&env, "healthcare"),
            Symbol::new(&env, "EU"),
            0,
            10,
        );

        assert_eq!(exemption.entity, entity);
        assert_eq!(exemption.status, 0);
    }
}
