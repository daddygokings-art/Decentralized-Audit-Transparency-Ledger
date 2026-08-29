//! Data retention, legal hold, GDPR right-to-erasure, and the immutable operational
//! audit log for AuditLedger.
//!
//! # Design
//!
//! The core `AuditLedger` event log is append-only and hash-chained (see
//! `docs/adr/ADR-001-append-only-log.md`), which makes literal on-chain deletion
//! infeasible — this mirrors the "Right to erasure: Not feasible on-chain" finding in
//! `docs/security/privacy-by-design.md`. This module implements the practical
//! alternative recommended there: **crypto-shredding**. An event eligible for erasure
//! has its `metadata` field redacted in place (reusing `AuditLedger::update_event`'s
//! existing chain-rewiring logic, so the hash chain stays intact), while the original
//! metadata's SHA-256 digest is preserved in an `ErasureRecordData` so auditors can
//! verify a redaction happened rather than undetected tampering.
//!
//! Retention is policy-driven per event `category`, with two overrides:
//! - **Legal hold** (`place_legal_hold`) — blocks erasure indefinitely for a specific
//!   event, e.g. during litigation or an active investigation.
//! - **Compliance exception** (`grant_compliance_exception`) — blocks erasure until an
//!   expiry timestamp, e.g. a statutory record-keeping requirement that outlives a GDPR
//!   erasure request.
//!
//! Every governance action here (policy changes, holds, exceptions, erasure decisions)
//! is gated the same way as the rest of the contract's governance surface — owner or
//! multisig, via `AuditLedger::require_owner_or_multisig` — for consistency with
//! `pause`, `set_event_ttl`, `block_submitter`, etc. `request_erasure` is the one
//! exception: any authenticated address may file a request (e.g. the data subject),
//! but only owner/multisig can approve or deny it via `process_erasure_request`.
//!
//! # Immutable operational audit log
//!
//! `log_operational_action` appends deployment, configuration-change, access-grant, and
//! secret-rotation records as ordinary events under the reserved `operational` category,
//! reusing the exact same append-only, content-addressed, hash-chained storage as
//! business audit events (no new storage engine to trust). Unlike business events,
//! `operational`-category events are *never* eligible for erasure — `request_erasure`
//! and `run_retention_sweep` both refuse to touch them — so the operational trail stays
//! immutable for the lifetime of the contract. Only addresses added via
//! `add_ops_recorder` (or the owner/multisig) may append to it, so CI/CD service
//! accounts can be scoped to this single capability without full governance rights.

use soroban_sdk::{contracttype, panic_with_error, Address, Bytes, BytesN, Env, Symbol, Vec};

use crate::{AuditLedger, AuditLedgerArgs, AuditLedgerClient, ContractError, DataKey};

/// Reserved category for operational audit log entries (deployments, configuration
/// changes, access grants, secret rotations). Events in this category are never
/// eligible for erasure.
fn operational_category(env: &Env) -> Symbol {
    Symbol::new(env, "operational")
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionPolicyData {
    pub retention_days: u32,
    pub legal_basis: Symbol,
    pub set_by: Address,
    pub set_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalHoldRecord {
    pub active: bool,
    pub reason: Bytes,
    pub placed_by: Address,
    pub placed_at: u64,
    pub released_by: Option<Address>,
    /// Timestamp the hold was released, or 0 while still active.
    pub released_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComplianceExceptionRecord {
    pub reason: Bytes,
    pub granted_by: Address,
    pub granted_at: u64,
    /// 0 means the exception never expires.
    pub expires_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErasureStatus {
    Pending,
    Fulfilled,
    Denied,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErasureRequestData {
    pub requester: Address,
    pub event_index: u32,
    pub reason: Symbol,
    pub requested_at: u64,
    pub status: ErasureStatus,
    pub decided_by: Option<Address>,
    /// Timestamp of the decision, or 0 while still pending.
    pub decided_at: u64,
    pub justification: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErasureRecordData {
    pub erased: bool,
    pub erased_at: u64,
    pub erased_by: Address,
    /// SHA-256 of the original (pre-redaction) metadata, so auditors can verify a
    /// redaction happened rather than undetected tampering.
    pub original_metadata_hash: BytesN<32>,
    pub reason: Symbol,
}

/// Result of a bounded, read-only retention compliance scan (see `verify_retention_compliance`).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionComplianceReport {
    pub checked: u32,
    pub compliant: u32,
    pub overdue_blocked_hold: u32,
    pub overdue_blocked_exception: u32,
    pub overdue_erasable: u32,
    pub already_erased: u32,
}

#[soroban_sdk::contractimpl]
impl AuditLedger {
    // ═══════════════════════════════════════════════════════════════════════
    // Retention policy
    // ═══════════════════════════════════════════════════════════════════════

    /// Set (or replace) the retention policy for a category of events. `retention_days`
    /// is the number of days after which events in this category become eligible for
    /// erasure via `run_retention_sweep` or an approved `process_erasure_request`,
    /// unless they are under an active legal hold or compliance exception.
    ///
    /// Owner/multisig-only.
    pub fn set_retention_policy(env: Env, caller: Address, category: Symbol, retention_days: u32, legal_basis: Symbol) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if retention_days == 0 {
            panic_with_error!(&env, ContractError::InvalidRetentionPeriod);
        }
        let policy = RetentionPolicyData {
            retention_days,
            legal_basis: legal_basis.clone(),
            set_by: caller.clone(),
            set_at: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&DataKey::RetentionPolicy(category.clone()), &policy);
        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "retention_policy_set")),
            (caller, category, retention_days, legal_basis),
        );
    }

    /// Return the retention policy configured for `category`, if any.
    pub fn get_retention_policy(env: Env, category: Symbol) -> Option<RetentionPolicyData> {
        env.storage().instance().get(&DataKey::RetentionPolicy(category))
    }

    /// Set the global default retention period (in days) applied to categories without
    /// their own explicit policy. `days == 0` disables the default (no automatic retention).
    /// Owner/multisig-only.
    pub fn set_default_retention_days(env: Env, caller: Address, days: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage().instance().set(&DataKey::DefaultRetentionDays, &days);
        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "default_retention_set")),
            (caller, days),
        );
    }

    /// Return the global default retention period in days, or 0 if unset.
    pub fn get_default_retention_days(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::DefaultRetentionDays).unwrap_or(0)
    }

    fn retention_days_for(env: &Env, category: &Symbol) -> u32 {
        if let Some(policy) = env
            .storage()
            .instance()
            .get::<_, RetentionPolicyData>(&DataKey::RetentionPolicy(category.clone()))
        {
            return policy.retention_days;
        }
        env.storage().instance().get(&DataKey::DefaultRetentionDays).unwrap_or(0)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Legal hold
    // ═══════════════════════════════════════════════════════════════════════

    /// Place a legal hold on a specific event, preventing erasure or TTL cleanup while
    /// active (e.g. during litigation or an active investigation). Owner/multisig-only.
    pub fn place_legal_hold(env: Env, caller: Address, event_index: u32, reason: Bytes) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if reason.len() == 0 {
            panic_with_error!(&env, ContractError::EmptyComplianceReason);
        }
        let total = Self::total_events(env.clone());
        if event_index >= total {
            panic_with_error!(&env, ContractError::EventDoesNotExist);
        }
        let hold = LegalHoldRecord {
            active: true,
            reason,
            placed_by: caller.clone(),
            placed_at: env.ledger().timestamp(),
            released_by: None,
            released_at: 0,
        };
        env.storage().instance().set(&DataKey::LegalHold(event_index), &hold);
        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "legal_hold_placed")),
            (caller, event_index),
        );
    }

    /// Release a legal hold on an event, allowing it to become eligible for erasure or
    /// cleanup again. Owner/multisig-only. The hold record is retained (with
    /// `active = false`) for audit history rather than deleted.
    pub fn release_legal_hold(env: Env, caller: Address, event_index: u32, justification: Bytes) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let mut hold: LegalHoldRecord = env
            .storage()
            .instance()
            .get(&DataKey::LegalHold(event_index))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::LegalHoldNotFound));
        if !hold.active {
            panic_with_error!(&env, ContractError::LegalHoldNotFound);
        }
        hold.active = false;
        hold.released_by = Some(caller.clone());
        hold.released_at = env.ledger().timestamp();
        env.storage().instance().set(&DataKey::LegalHold(event_index), &hold);
        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "legal_hold_released")),
            (caller, event_index, justification.len()),
        );
    }

    /// Returns true if `event_index` is currently under an active legal hold.
    pub fn is_under_legal_hold(env: Env, event_index: u32) -> bool {
        env.storage()
            .instance()
            .get::<_, LegalHoldRecord>(&DataKey::LegalHold(event_index))
            .map(|h| h.active)
            .unwrap_or(false)
    }

    /// Return the legal hold record for `event_index`, if one has ever been placed.
    pub fn get_legal_hold(env: Env, event_index: u32) -> Option<LegalHoldRecord> {
        env.storage().instance().get(&DataKey::LegalHold(event_index))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Compliance exceptions
    // ═══════════════════════════════════════════════════════════════════════

    /// Grant a compliance exception overriding retention/erasure for an event (e.g. a
    /// statutory record-keeping requirement that outlives a GDPR erasure request).
    /// `expires_at == 0` means the exception never expires. Owner/multisig-only.
    pub fn grant_compliance_exception(env: Env, caller: Address, event_index: u32, reason: Bytes, expires_at: u64) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        if reason.len() == 0 {
            panic_with_error!(&env, ContractError::EmptyComplianceReason);
        }
        let total = Self::total_events(env.clone());
        if event_index >= total {
            panic_with_error!(&env, ContractError::EventDoesNotExist);
        }
        let exception = ComplianceExceptionRecord {
            reason,
            granted_by: caller.clone(),
            granted_at: env.ledger().timestamp(),
            expires_at,
        };
        env.storage()
            .instance()
            .set(&DataKey::ComplianceException(event_index), &exception);
        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "exception_granted")),
            (caller, event_index, expires_at),
        );
    }

    /// Revoke a compliance exception ahead of its expiry. Owner/multisig-only.
    pub fn revoke_compliance_exception(env: Env, caller: Address, event_index: u32) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        env.storage()
            .instance()
            .remove(&DataKey::ComplianceException(event_index));
        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "exception_revoked")),
            (caller, event_index),
        );
    }

    /// Returns true if `event_index` has a currently active (unexpired) compliance exception.
    pub fn has_active_compliance_exception(env: Env, event_index: u32) -> bool {
        if let Some(exception) = env
            .storage()
            .instance()
            .get::<_, ComplianceExceptionRecord>(&DataKey::ComplianceException(event_index))
        {
            exception.expires_at == 0 || exception.expires_at > env.ledger().timestamp()
        } else {
            false
        }
    }

    /// Return the compliance exception record for `event_index`, if any.
    pub fn get_compliance_exception(env: Env, event_index: u32) -> Option<ComplianceExceptionRecord> {
        env.storage().instance().get(&DataKey::ComplianceException(event_index))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // GDPR right to erasure
    // ═══════════════════════════════════════════════════════════════════════

    /// File a GDPR right-to-erasure request against an event's metadata. Any
    /// authenticated address may file a request (e.g. the data subject or their
    /// representative); it is recorded immutably and must be reviewed via
    /// `process_erasure_request` before anything is redacted. Returns the new request ID.
    pub fn request_erasure(env: Env, requester: Address, event_index: u32, reason: Symbol) -> u32 {
        Self::require_initialized(&env);
        requester.require_auth();

        let event = Self::get_event_by_order(env.clone(), event_index);
        if event.category == operational_category(&env) {
            panic_with_error!(&env, ContractError::OperationalEventNotErasable);
        }
        if Self::is_event_erased(env.clone(), event_index) {
            panic_with_error!(&env, ContractError::EventAlreadyErased);
        }

        let request_id: u32 = env.storage().instance().get(&DataKey::ErasureRequestCount).unwrap_or(0);
        let request = ErasureRequestData {
            requester: requester.clone(),
            event_index,
            reason: reason.clone(),
            requested_at: env.ledger().timestamp(),
            status: ErasureStatus::Pending,
            decided_by: None,
            decided_at: 0,
            justification: Bytes::new(&env),
        };
        env.storage().instance().set(&DataKey::ErasureRequest(request_id), &request);
        env.storage()
            .instance()
            .set(&DataKey::ErasureRequestCount, &(request_id + 1));
        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "erasure_requested")),
            (requester, event_index, request_id, reason),
        );
        request_id
    }

    /// Return an erasure request by ID, if any.
    pub fn get_erasure_request(env: Env, request_id: u32) -> Option<ErasureRequestData> {
        env.storage().instance().get(&DataKey::ErasureRequest(request_id))
    }

    /// Review a pending erasure request. Owner/multisig-only. On approval — and only if
    /// no legal hold or active compliance exception blocks it — the event's metadata is
    /// redacted (crypto-shredded) in place via `update_event`, which recomputes the
    /// content-addressed ID/hash and re-chains every later event, so the tamper-evident
    /// chain stays intact. The original metadata's SHA-256 is preserved in the erasure
    /// record so auditors can verify redaction happened rather than undetected tampering.
    pub fn process_erasure_request(env: Env, caller: Address, request_id: u32, approve: bool, justification: Bytes) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let mut request: ErasureRequestData = env
            .storage()
            .instance()
            .get(&DataKey::ErasureRequest(request_id))
            .unwrap_or_else(|| panic_with_error!(&env, ContractError::ErasureRequestNotFound));

        if request.status != ErasureStatus::Pending {
            panic_with_error!(&env, ContractError::ErasureRequestAlreadyDecided);
        }

        let event_index = request.event_index;

        if approve {
            let event = Self::get_event_by_order(env.clone(), event_index);
            if event.category == operational_category(&env) {
                panic_with_error!(&env, ContractError::OperationalEventNotErasable);
            }
            if Self::is_under_legal_hold(env.clone(), event_index) {
                panic_with_error!(&env, ContractError::EventOnLegalHold);
            }
            if Self::has_active_compliance_exception(env.clone(), event_index) {
                panic_with_error!(&env, ContractError::ComplianceExceptionActive);
            }

            let original_hash: BytesN<32> = env.crypto().sha256(&event.metadata).into();

            Self::update_event(env.clone(), caller.clone(), event_index, Bytes::new(&env));

            let erasure_record = ErasureRecordData {
                erased: true,
                erased_at: env.ledger().timestamp(),
                erased_by: caller.clone(),
                original_metadata_hash: original_hash,
                reason: request.reason.clone(),
            };
            env.storage()
                .instance()
                .set(&DataKey::ErasureRecord(event_index), &erasure_record);

            request.status = ErasureStatus::Fulfilled;
        } else {
            request.status = ErasureStatus::Denied;
        }

        request.decided_by = Some(caller.clone());
        request.decided_at = env.ledger().timestamp();
        request.justification = justification;
        env.storage().instance().set(&DataKey::ErasureRequest(request_id), &request);

        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "erasure_decided")),
            (caller, request_id, event_index, approve),
        );
    }

    /// Return the erasure record for `event_index`, if its metadata has been redacted.
    pub fn get_erasure_record(env: Env, event_index: u32) -> Option<ErasureRecordData> {
        env.storage().instance().get(&DataKey::ErasureRecord(event_index))
    }

    /// Returns true if `event_index`'s metadata has already been erased (redacted).
    pub fn is_event_erased(env: Env, event_index: u32) -> bool {
        env.storage()
            .instance()
            .get::<_, ErasureRecordData>(&DataKey::ErasureRecord(event_index))
            .map(|r| r.erased)
            .unwrap_or(false)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Automated retention verification & sweep
    // ═══════════════════════════════════════════════════════════════════════

    /// Verify retention compliance for a bounded batch of events without mutating any
    /// state. For each event, classifies it as: compliant (within its retention window,
    /// operational, or already erased), overdue but protected by a legal hold, overdue
    /// but protected by a compliance exception, or overdue and eligible for erasure.
    pub fn verify_retention_compliance(env: Env, start_index: u32, batch_size: u32) -> RetentionComplianceReport {
        Self::require_initialized(&env);
        let total = Self::total_events(env.clone());
        let end = if start_index.saturating_add(batch_size) < total {
            start_index + batch_size
        } else {
            total
        };

        let mut report = RetentionComplianceReport {
            checked: 0,
            compliant: 0,
            overdue_blocked_hold: 0,
            overdue_blocked_exception: 0,
            overdue_erasable: 0,
            already_erased: 0,
        };

        let now = env.ledger().timestamp();
        let op_category = operational_category(&env);
        for i in start_index..end {
            let event = Self::get_event_by_order(env.clone(), i);
            report.checked += 1;

            if event.category == op_category {
                report.compliant += 1;
                continue;
            }
            if Self::is_event_erased(env.clone(), i) {
                report.already_erased += 1;
                continue;
            }

            let retention_days = Self::retention_days_for(&env, &event.category);
            if retention_days == 0 {
                report.compliant += 1;
                continue;
            }
            let retention_seconds = (retention_days as u64).saturating_mul(86_400);
            let age = now.saturating_sub(event.timestamp);
            if age <= retention_seconds {
                report.compliant += 1;
            } else if Self::is_under_legal_hold(env.clone(), i) {
                report.overdue_blocked_hold += 1;
            } else if Self::has_active_compliance_exception(env.clone(), i) {
                report.overdue_blocked_exception += 1;
            } else {
                report.overdue_erasable += 1;
            }
        }

        report
    }

    /// Run an automated retention sweep over a bounded batch, redacting metadata for any
    /// event that is past its retention period and not protected by a legal hold or
    /// compliance exception (and never touching `operational`-category entries).
    /// Owner/multisig-only. Returns the number of events erased in this run.
    pub fn run_retention_sweep(env: Env, caller: Address, start_index: u32, batch_size: u32) -> u32 {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);

        let total = Self::total_events(env.clone());
        let end = if start_index.saturating_add(batch_size) < total {
            start_index + batch_size
        } else {
            total
        };

        let now = env.ledger().timestamp();
        let op_category = operational_category(&env);
        let mut erased_count: u32 = 0;

        for i in start_index..end {
            let event = Self::get_event_by_order(env.clone(), i);
            if event.category == op_category {
                continue;
            }
            if Self::is_event_erased(env.clone(), i) {
                continue;
            }
            let retention_days = Self::retention_days_for(&env, &event.category);
            if retention_days == 0 {
                continue;
            }
            let retention_seconds = (retention_days as u64).saturating_mul(86_400);
            if now.saturating_sub(event.timestamp) <= retention_seconds {
                continue;
            }
            if Self::is_under_legal_hold(env.clone(), i) {
                continue;
            }
            if Self::has_active_compliance_exception(env.clone(), i) {
                continue;
            }

            let original_hash: BytesN<32> = env.crypto().sha256(&event.metadata).into();
            Self::update_event(env.clone(), caller.clone(), i, Bytes::new(&env));
            env.storage().instance().set(
                &DataKey::ErasureRecord(i),
                &ErasureRecordData {
                    erased: true,
                    erased_at: now,
                    erased_by: caller.clone(),
                    original_metadata_hash: original_hash,
                    reason: Symbol::new(&env, "retention_expired"),
                },
            );
            erased_count += 1;
        }

        env.events().publish(
            (Symbol::new(&env, "compliance"), Symbol::new(&env, "retention_sweep")),
            (caller, start_index, end, erased_count),
        );

        erased_count
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Immutable operational audit log
    // ═══════════════════════════════════════════════════════════════════════

    fn is_addr_ops_recorder(env: &Env, addr: &Address) -> bool {
        if Self::is_addr_owner(env, addr) {
            return true;
        }
        let recorders: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::OpsAuditRecorders)
            .unwrap_or_else(|| Vec::new(env));
        for i in 0..recorders.len() {
            if &recorders.get(i).unwrap() == addr {
                return true;
            }
        }
        false
    }

    /// Authorize an address (e.g. a CI/CD service account) to append operational audit
    /// records via `log_operational_action`. Owner/multisig-only.
    pub fn add_ops_recorder(env: Env, caller: Address, recorder: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let mut recorders: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::OpsAuditRecorders)
            .unwrap_or_else(|| Vec::new(&env));
        for i in 0..recorders.len() {
            if recorders.get(i).unwrap() == recorder {
                return;
            }
        }
        recorders.push_back(recorder.clone());
        env.storage().instance().set(&DataKey::OpsAuditRecorders, &recorders);
        env.events()
            .publish((Symbol::new(&env, "ops_recorder_added"),), (recorder, caller));
    }

    /// Revoke an address's authorization to append operational audit records. Owner/multisig-only.
    pub fn remove_ops_recorder(env: Env, caller: Address, recorder: Address) {
        Self::require_initialized(&env);
        caller.require_auth();
        Self::require_owner_or_multisig(&env, &caller);
        let recorders: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::OpsAuditRecorders)
            .unwrap_or_else(|| Vec::new(&env));
        let mut new_list: Vec<Address> = Vec::new(&env);
        for i in 0..recorders.len() {
            let r = recorders.get(i).unwrap();
            if r != recorder {
                new_list.push_back(r);
            }
        }
        env.storage().instance().set(&DataKey::OpsAuditRecorders, &new_list);
        env.events()
            .publish((Symbol::new(&env, "ops_recorder_removed"),), (recorder, caller));
    }

    /// Returns true if `addr` is authorized to append operational audit records
    /// (either the owner/multisig, or added via `add_ops_recorder`).
    pub fn is_ops_recorder(env: Env, addr: Address) -> bool {
        Self::is_addr_ops_recorder(&env, &addr)
    }

    /// Append an immutable operational audit record: a deployment, configuration change,
    /// access grant, or secret rotation. Reuses the same append-only, hash-chained event
    /// log as business audit events (category = `operational`), so entries are
    /// content-addressed, chain-linked to the previous event, and can be verified with
    /// the same tooling as `get_event`/`total_events` — but are never eligible for GDPR
    /// erasure or retention-driven cleanup (see `request_erasure`, `run_retention_sweep`).
    ///
    /// `action_type` should be a reserved operational symbol, e.g. `ops_deploy`,
    /// `ops_config`, `ops_access`, or `ops_secret`, so callers can page through the trail
    /// by type via the existing `get_events_by_type`.
    pub fn log_operational_action(env: Env, caller: Address, action_type: Symbol, details: Bytes) -> BytesN<32> {
        Self::require_initialized(&env);
        if !Self::is_addr_ops_recorder(&env, &caller) {
            panic_with_error!(&env, ContractError::UnauthorizedOpsRecorder);
        }
        let category = operational_category(&env);
        Self::log_event_with_hierarchy(env, caller, action_type, details, Some(category), None, true)
    }
}
