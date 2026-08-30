/// # Privacy Training and Awareness Module
///
/// Comprehensive privacy training and awareness management framework
/// implementing GDPR Article 39(1)(b) requirements with mandatory training
/// programs, role-specific modules, completion tracking, annual refresher
/// automation, and privacy culture metrics.
///
/// ## Regulatory Framework
/// - **GDPR Article 39** — Tasks of the data protection officer
/// - **ISO 27701** — Privacy training and awareness requirements
/// - **NIST SP 800-50** — Building an information technology security awareness and training program

use soroban_sdk::{
    bytes, contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Bytes,
    BytesN, Env, Symbol, Vec,
};

// ── Error Codes ──────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TrainingError {
    /// Training module not found
    ModuleNotFound = 6300,
    /// Training assignment not found
    AssignmentNotFound = 6301,
    /// Employee not enrolled
    EmployeeNotEnrolled = 6302,
    /// Training already completed
    TrainingAlreadyCompleted = 6303,
    /// Refresher not yet due
    RefresherNotYetDue = 6304,
    /// Role not recognized
    RoleNotRecognized = 6305,
    /// Completion threshold not met
    CompletionThresholdNotMet = 6306,
    /// Module not active
    ModuleNotActive = 6307,
    /// Assessment failed
    AssessmentFailed = 6308,
    /// Certification expired
    CertificationExpired = 6309,
    /// Mandatory module missing
    MandatoryModuleMissing = 6310,
    /// Training period expired
    TrainingPeriodExpired = 6311,
}

// ── Data Types ───────────────────────────────────────────────────────────

/// Staff role for training assignment
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum StaffRole {
    AllStaff = 0,
    Management = 1,
    HR = 2,
    IT = 3,
    Legal = 4,
    Finance = 5,
    Marketing = 6,
    Operations = 7,
    DPO = 8,
    Engineering = 9,
    CustomerSupport = 10,
}

/// Training module type
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum ModuleType {
    GDPRBasics = 1,
    DataHandling = 2,
    BreachResponse = 3,
    DSRHandling = 4,
    PrivacyByDesign = 5,
    DataMinimization = 6,
    InternationalTransfers = 7,
    RecordKeeping = 8,
    VendorManagement = 9,
    IncidentReporting = 10,
}

/// Completion status
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[repr(u32)]
pub enum CompletionStatus {
    NotStarted = 0,
    InProgress = 1,
    Completed = 2,
    Failed = 3,
    Expired = 4,
    Overdue = 5,
}

/// Training module
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingModule {
    pub id: BytesN<32>,
    pub module_type: u32,
    pub title: Bytes,
    pub description: Bytes,
    pub content_hash: BytesN<32>,
    pub duration_minutes: u32,
    pub passing_score: u32,
    pub roles_required: Vec<u32>,
    pub is_mandatory: bool,
    pub refresher_months: u32,
    pub version: u32,
    pub is_active: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub module_hash: BytesN<32>,
}

/// Training assignment
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingAssignment {
    pub id: BytesN<32>,
    pub module_id: BytesN<32>,
    pub employee: Address,
    pub role: u32,
    pub status: u32,
    pub assigned_at: u64,
    pub started_at: u64,
    pub completed_at: u64,
    pub score: u32,
    pub attempts: u32,
    pub certificate_hash: BytesN<32>,
    pub next_refresher_due: u64,
    pub assignment_hash: BytesN<32>,
}

/// Training completion record
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionRecord {
    pub id: BytesN<32>,
    pub assignment_id: BytesN<32>,
    pub employee: Address,
    pub module_id: BytesN<32>,
    pub module_title: Bytes,
    pub status: u32,
    pub score: u32,
    pub duration_minutes: u32,
    pub completed_at: u64,
    pub certificate_expires_at: u64,
    pub verifier: Bytes,
    pub record_hash: BytesN<32>,
}

/// Training statistics
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainingStats {
    pub total_modules: u32,
    pub total_assignments: u32,
    pub completed_count: u32,
    pub overdue_count: u32,
    pub completion_rate: u32,
    pub average_score: u32,
    pub last_updated: u64,
}

// ── Storage Keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrainingKey {
    Owner,
    TrainingModule(BytesN<32>),
    ModuleByType(u32),
    Assignment(BytesN<32>),
    AssignmentByEmployee(Address),
    AssignmentByModule(BytesN<32>),
    CompletionRecord(BytesN<32>),
    CompletionByEmployee(Address),
    ModuleCount,
    AssignmentCount,
    CompletionCount,
}

// ── Contract Implementation ──────────────────────────────────────────────

#[contract]
pub struct PrivacyTraining;

#[contractimpl]
impl PrivacyTraining {
    /// Initialize privacy training module
    pub fn initialize(env: Env, owner: Address) {
        owner.require_auth();
        env.storage()
            .instance()
            .set(&TrainingKey::Owner, &owner);
        env.storage()
            .instance()
            .set(&TrainingKey::ModuleCount, &0u32);
        env.storage()
            .instance()
            .set(&TrainingKey::AssignmentCount, &0u32);
        env.storage()
            .instance()
            .set(&TrainingKey::CompletionCount, &0u32);
    }

    // ── Module Management ────────────────────────────────────────────────

    pub fn create_module(
        env: Env,
        caller: Address,
        module_type: u32,
        title: Bytes,
        description: Bytes,
        content_hash: BytesN<32>,
        duration_minutes: u32,
        passing_score: u32,
        roles_required: Vec<u32>,
        is_mandatory: bool,
        refresher_months: u32,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let module_id = env.crypto().sha256(&title.clone()).into();
        let now = env.ledger().timestamp();

        let module = TrainingModule {
            id: module_id.clone(),
            module_type,
            title: title.clone(),
            description,
            content_hash,
            duration_minutes,
            passing_score,
            roles_required,
            is_mandatory,
            refresher_months,
            version: 1,
            is_active: true,
            created_at: now,
            updated_at: now,
            module_hash: env
                .crypto()
                .sha256(&Self::pack_module_data(&env, &title, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TrainingKey::TrainingModule(module_id.clone()), &module);
        env.storage()
            .instance()
            .set(&TrainingKey::ModuleByType(module_type), &module_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&TrainingKey::ModuleCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&TrainingKey::ModuleCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "training"), Symbol::new(&env, "module_created")),
            (module_id.clone(), module_type, title),
        );

        module_id
    }

    pub fn get_module(env: Env, module_id: BytesN<32>) -> TrainingModule {
        env.storage()
            .instance()
            .get(&TrainingKey::TrainingModule(module_id))
            .unwrap_or_else(|| panic_with_error!(&env, TrainingError::ModuleNotFound))
    }

    // ── Assignment Management ────────────────────────────────────────────

    pub fn assign_training(
        env: Env,
        caller: Address,
        module_id: BytesN<32>,
        employee: Address,
        role: u32,
        due_date: u64,
    ) -> BytesN<32> {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let module = Self::get_module(env.clone(), module_id.clone());
        let now = env.ledger().timestamp();

        if !module.is_active {
            panic_with_error!(&env, TrainingError::ModuleNotActive);
        }

        let assignment_id = env.crypto().sha256(&module_id.clone().to_bytes()).into();

        let assignment = TrainingAssignment {
            id: assignment_id.clone(),
            module_id: module_id.clone(),
            employee: employee.clone(),
            role,
            status: CompletionStatus::NotStarted as u32,
            assigned_at: now,
            started_at: 0,
            completed_at: 0,
            score: 0,
            attempts: 0,
            certificate_hash: BytesN::from_array(&env, &[0u8; 32]),
            next_refresher_due: if module.refresher_months > 0 {
                now + (module.refresher_months as u64 * 2592000)
            } else {
                0
            },
            assignment_hash: env
                .crypto()
                .sha256(&Self::pack_assignment_data(&env, &module_id, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TrainingKey::Assignment(assignment_id.clone()), &assignment);
        env.storage()
            .instance()
            .set(&TrainingKey::AssignmentByEmployee(employee.clone()), &assignment_id);
        env.storage()
            .instance()
            .set(&TrainingKey::AssignmentByModule(module_id), &assignment_id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&TrainingKey::AssignmentCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&TrainingKey::AssignmentCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "training"), Symbol::new(&env, "assigned")),
            (assignment_id.clone(), employee, due_date),
        );

        assignment_id
    }

    pub fn get_assignment(env: Env, assignment_id: BytesN<32>) -> TrainingAssignment {
        env.storage()
            .instance()
            .get(&TrainingKey::Assignment(assignment_id))
            .unwrap_or_else(|| panic_with_error!(&env, TrainingError::AssignmentNotFound))
    }

    // ── Completion Tracking ──────────────────────────────────────────────

    pub fn start_training(env: Env, caller: Address, assignment_id: BytesN<32>) -> TrainingAssignment {
        caller.require_auth();

        let mut assignment = Self::get_assignment(env.clone(), assignment_id.clone());

        if assignment.status != CompletionStatus::NotStarted as u32 {
            panic_with_error!(&env, TrainingError::TrainingAlreadyCompleted);
        }

        assignment.status = CompletionStatus::InProgress as u32;
        assignment.started_at = env.ledger().timestamp();
        assignment.attempts += 1;

        env.storage()
            .instance()
            .set(&TrainingKey::Assignment(assignment_id.clone()), &assignment);

        env.events().publish(
            (Symbol::new(&env, "training"), Symbol::new(&env, "started")),
            (assignment_id, assignment.employee),
        );

        assignment
    }

    pub fn complete_training(
        env: Env,
        caller: Address,
        assignment_id: BytesN<32>,
        score: u32,
        certificate_hash: BytesN<32>,
    ) -> CompletionRecord {
        caller.require_auth();

        let mut assignment = Self::get_assignment(env.clone(), assignment_id.clone());
        let module = Self::get_module(env.clone(), assignment.module_id.clone());
        let now = env.ledger().timestamp();

        if score < module.passing_score {
            panic_with_error!(&env, TrainingError::AssessmentFailed);
        }

        assignment.status = CompletionStatus::Completed as u32;
        assignment.completed_at = now;
        assignment.score = score;
        assignment.certificate_hash = certificate_hash.clone();

        env.storage()
            .instance()
            .set(&TrainingKey::Assignment(assignment_id.clone()), &assignment);

        let completion = CompletionRecord {
            id: env.crypto().sha256(&certificate_hash).into(),
            assignment_id: assignment_id.clone(),
            employee: assignment.employee.clone(),
            module_id: assignment.module_id.clone(),
            module_title: module.title.clone(),
            status: CompletionStatus::Completed as u32,
            score,
            duration_minutes: module.duration_minutes,
            completed_at: now,
            certificate_expires_at: if module.refresher_months > 0 {
                now + (module.refresher_months as u64 * 2592000)
            } else {
                0
            },
            verifier: Bytes::new(&env),
            record_hash: env
                .crypto()
                .sha256(&Self::pack_completion_data(&env, &assignment_id, now))
                .into(),
        };

        env.storage()
            .instance()
            .set(&TrainingKey::CompletionRecord(completion.id.clone()), &completion);
        env.storage()
            .instance()
            .set(&TrainingKey::CompletionByEmployee(assignment.employee.clone()), &completion.id);

        let count: u32 = env
            .storage()
            .instance()
            .get(&TrainingKey::CompletionCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&TrainingKey::CompletionCount, &(count + 1));

        env.events().publish(
            (Symbol::new(&env, "training"), Symbol::new(&env, "completed")),
            (completion.id.clone(), assignment.employee, score),
        );

        completion
    }

    // ── Refresher Management ─────────────────────────────────────────────

    pub fn schedule_refresher(
        env: Env,
        caller: Address,
        assignment_id: BytesN<32>,
        new_due_date: u64,
    ) -> TrainingAssignment {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut assignment = Self::get_assignment(env.clone(), assignment_id.clone());

        if assignment.status != CompletionStatus::Completed as u32 {
            panic_with_error!(&env, TrainingError::TrainingAlreadyCompleted);
        }

        let now = env.ledger().timestamp();
        assignment.next_refresher_due = new_due_date;
        assignment.status = CompletionStatus::NotStarted as u32;
        assignment.completed_at = 0;
        assignment.score = 0;
        assignment.assignment_hash = env
            .crypto()
            .sha256(&Self::pack_refresher_data(&env, &assignment_id, now))
            .into();

        env.storage()
            .instance()
            .set(&TrainingKey::Assignment(assignment_id.clone()), &assignment);

        env.events().publish(
            (Symbol::new(&env, "training"), Symbol::new(&env, "refresher_scheduled")),
            (assignment_id, new_due_date),
        );

        assignment
    }

    pub fn get_completion(env: Env, completion_id: BytesN<32>) -> CompletionRecord {
        env.storage()
            .instance()
            .get(&TrainingKey::CompletionRecord(completion_id))
            .unwrap_or_else(|| panic_with_error!(&env, TrainingError::AssignmentNotFound))
    }

    // ── Statistics ───────────────────────────────────────────────────────

    pub fn get_training_stats(env: Env) -> TrainingStats {
        let total_modules: u32 = env
            .storage()
            .instance()
            .get(&TrainingKey::ModuleCount)
            .unwrap_or(0);
        let total_assignments: u32 = env
            .storage()
            .instance()
            .get(&TrainingKey::AssignmentCount)
            .unwrap_or(0);
        let completed_count: u32 = env
            .storage()
            .instance()
            .get(&TrainingKey::CompletionCount)
            .unwrap_or(0);

        let completion_rate = if total_assignments > 0 {
            (completed_count * 100) / total_assignments
        } else {
            0
        };

        TrainingStats {
            total_modules,
            total_assignments,
            completed_count,
            overdue_count: 0,
            completion_rate,
            average_score: 0,
            last_updated: env.ledger().timestamp(),
        }
    }

    // ── Private Helpers ──────────────────────────────────────────────────

    fn require_owner(env: &Env, caller: &Address) {
        let owner: Address = env
            .storage()
            .instance()
            .get(&TrainingKey::Owner)
            .unwrap();
        if caller != &owner {
            panic_with_error!(env, TrainingError::RoleNotRecognized);
        }
    }

    fn pack_module_data(env: &Env, title: &Bytes, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(title);
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_assignment_data(env: &Env, module_id: &BytesN<32>, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&module_id.clone().into());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_completion_data(env: &Env, assignment_id: &BytesN<32>, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&assignment_id.clone().into());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
    }

    fn pack_refresher_data(env: &Env, assignment_id: &BytesN<32>, timestamp: u64) -> Bytes {
        let mut data = Bytes::new(env);
        data.append(&assignment_id.clone().into());
        data.append(&Self::u64_to_bytes(env, timestamp));
        data
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
}

#[cfg(test)]
mod tests;
