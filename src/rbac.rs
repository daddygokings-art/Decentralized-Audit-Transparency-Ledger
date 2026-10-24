//! Role-Based Access Control (RBAC) Module (#686 #689 #688 #687)
//!
//! Replaces legacy single-owner authorization checks with explicit,
//! granular roles, precedence hierarchies, and persistent contract storage.

use soroban_sdk::{contracterror, contracttype, Address, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RbacError {
    UnauthorizedCaller = 401,
    RoleNotFound = 404,
    InsufficientRole = 403,
    CannotRevokeLastAdmin = 409,
    InvalidRoleAssignment = 422,
    /// The submitter DID has been suspended and may not mutate state.
    SubmitterSuspended = 423,
    /// The caller DID is not authorized to perform the requested mutation.
    UnauthorizedSubmitter = 424,
    /// Legacy alias for a suspended DID, surfaced through `check_role`.
    Suspended = 425,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Role {
    /// Read-only access to published audit events and public verification
    Viewer = 1,
    /// Permission to submit and record new audit events and batches
    Submitter = 2,
    /// Permission to inspect audit analytics, compliance stats, and verification proofs
    Auditor = 3,
    /// Full governance permissions: assign roles, configure retention, upgrade policies
    Admin = 4,
}

#[contracttype]
pub enum RbacStorageKey {
    Role(Address),
    AdminCount,
    /// Marks a submitter DID as suspended, blocking further mutations.
    Suspended(Address),
}

pub struct RbacManager;

impl RbacManager {
    /// Assigns or updates the role of a target address. Caller must have Admin role.
    pub fn set_role(
        env: &Env,
        caller: &Address,
        target: &Address,
        role: Role,
    ) -> Result<(), RbacError> {
        caller.require_auth();

        // Caller must be an Admin
        if !Self::has_role_min(env, caller, Role::Admin) {
            return Err(RbacError::InsufficientRole);
        }

        let previous_role = Self::get_role(env, target);
        let key = RbacStorageKey::Role(target.clone());

        // Update admin count tracking
        let mut admin_count: u32 = env
            .storage()
            .persistent()
            .get(&RbacStorageKey::AdminCount)
            .unwrap_or(1);

        if previous_role == Some(Role::Admin) && role != Role::Admin {
            if admin_count <= 1 {
                return Err(RbacError::CannotRevokeLastAdmin);
            }
            admin_count -= 1;
        } else if previous_role != Some(Role::Admin) && role == Role::Admin {
            admin_count += 1;
        }

        env.storage().persistent().set(&key, &role);
        env.storage()
            .persistent()
            .set(&RbacStorageKey::AdminCount, &admin_count);

        Ok(())
    }

    /// Retrieves the current role assigned to an address, if any.
    pub fn get_role(env: &Env, address: &Address) -> Option<Role> {
        let key = RbacStorageKey::Role(address.clone());
        env.storage().persistent().get(&key)
    }

    /// Checks if the address possesses at least the required minimum role level.
    pub fn has_role_min(env: &Env, address: &Address, min_role: Role) -> bool {
        match Self::get_role(env, address) {
            Some(role) => role >= min_role,
            None => false,
        }
    }

    /// Enforces that the caller possesses at least the required minimum role level.
    pub fn require_role_min(
        env: &Env,
        address: &Address,
        min_role: Role,
    ) -> Result<(), RbacError> {
        if Self::has_role_min(env, address, min_role) {
            Ok(())
        } else {
            Err(RbacError::InsufficientRole)
        }
    }

    /// Role check used by granular module gating (issues #860, supply-chain).
    ///
    /// Resolves the resolved role hierarchy for `caller` and rejects the
    /// operation with the stable [`RbacError`] code when the caller is
    /// suspended or does not satisfy `required_role`. Callers of this
    /// function perform their own authentication via `Address::require_auth`.
    pub fn check_role(env: &Env, caller: &Address, required_role: Role) -> Result<(), RbacError> {
        if Self::is_suspended(env, caller) {
            return Err(RbacError::SubmitterSuspended);
        }
        Self::require_role_min(env, caller, required_role)
    }

    /// Bootstraps initial superadmin role during contract initialization.
    pub fn initialize_admin(env: &Env, admin: &Address) {
        let key = RbacStorageKey::Role(admin.clone());
        env.storage().persistent().set(&key, &Role::Admin);
        env.storage()
            .persistent()
            .set(&RbacStorageKey::AdminCount, &1u32);
    }

    /// Alias for initialize_admin
    pub fn init_admin(env: &Env, admin: &Address) {
        Self::initialize_admin(env, admin);
    }

    /// Marks a submitter DID as suspended, revoking its ability to mutate state.
    ///
    /// Caller must have Admin role. Suspension is persistent and takes
    /// precedence over any previously assigned role.
    /// Suspended submitters are rejected by [`Self::require_active_submitter`]
    /// with the stable [`RbacError::SubmitterSuspended`] code.
    pub fn suspend_submitter(
        env: &Env,
        caller: &Address,
        target: &Address,
    ) -> Result<(), RbacError> {
        caller.require_auth();

        if !Self::has_role_min(env, caller, Role::Admin) {
            return Err(RbacError::InsufficientRole);
        }

        let key = RbacStorageKey::Suspended(target.clone());
        env.storage().persistent().set(&key, &true);

        Ok(())
    }

    /// Alias for suspend_submitter
    pub fn suspend(
        env: &Env,
        caller: &Address,
        target: &Address,
    ) -> Result<(), RbacError> {
        Self::suspend_submitter(env, caller, target)
    }

    /// Clears the suspension flag for a submitter DID. Caller must have Admin role.
    pub fn reinstate_submitter(
        env: &Env,
        caller: &Address,
        target: &Address,
    ) -> Result<(), RbacError> {
        caller.require_auth();

        if !Self::has_role_min(env, caller, Role::Admin) {
            return Err(RbacError::InsufficientRole);
        }

        let key = RbacStorageKey::Suspended(target.clone());
        env.storage().persistent().remove(&key);

        Ok(())
    }

    /// Alias for reinstate_submitter
    pub fn unsuspend(
        env: &Env,
        caller: &Address,
        target: &Address,
    ) -> Result<(), RbacError> {
        Self::reinstate_submitter(env, caller, target)
    }

    /// Returns `true` if the given submitter DID is currently suspended.
    pub fn is_suspended(env: &Env, address: &Address) -> bool {
        let key = RbacStorageKey::Suspended(address.clone());
        env.storage().persistent().get(&key).unwrap_or(false)
    }

    /// Enforces role hierarchy precedence and revocation for a caller.
    ///
    /// This is the canonical gate used by state-mutating entrypoints. It
    /// validates the caller's authentication, rejects suspended addresses,
    /// and verifies the caller holds at least `min_role` in the hierarchy
    /// (`Viewer < Submitter < Auditor < Admin`). Revoked roles (no stored
    /// role) fail with [`RbacError::InsufficientRole`], while suspended
    /// submitters fail with [`RbacError::SubmitterSuspended`].
    pub fn require_role_precedence(
        env: &Env,
        caller: &Address,
        min_role: Role,
    ) -> Result<(), RbacError> {
        caller.require_auth();

        if Self::is_suspended(env, caller) {
            return Err(RbacError::SubmitterSuspended);
        }

        match Self::get_role(env, caller) {
            Some(role) if role >= min_role => Ok(()),
            Some(_) => Err(RbacError::InsufficientRole),
            None => Err(RbacError::InsufficientRole),
        }
    }

    /// Builds a deterministic state-diff digest for audit event emission.
    ///
    /// The digest binds the caller, the operation label, and the pre/post
    /// state values into a single 32-byte value so that audit events are
    /// tamper-evident and reproducible off-chain.
    pub fn state_diff_digest(
        env: &Env,
        caller: &Address,
        operation: &soroban_sdk::Symbol,
        previous: i128,
        current: i128,
    ) -> soroban_sdk::BytesN<32> {
        let mut buf = soroban_sdk::Bytes::new(env);
        buf.append(&caller.clone().to_string().into_bytes());
        buf.append(&operation.clone().to_string().into_bytes());
        buf.extend_from_array(&previous.to_be_bytes());
        buf.extend_from_array(&current.to_be_bytes());
        env.crypto().sha256(&buf).into()
    }

    /// Enforces that the caller is authenticated, holds at least `min_role`,
    /// and is not a suspended submitter DID.
    ///
    /// Returns stable RBAC error codes:
    /// - [`RbacError::InsufficientRole`] when the role hierarchy check fails
    /// - [`RbacError::SubmitterSuspended`] when the DID is suspended
    pub fn require_active_role_min(
        env: &Env,
        address: &Address,
        min_role: Role,
    ) -> Result<(), RbacError> {
        address.require_auth();

        if Self::is_suspended(env, address) {
            return Err(RbacError::SubmitterSuspended);
        }

        Self::require_role_min(env, address, min_role)
    }

    /// Enforces that the caller is an authenticated, non-suspended submitter
    /// (or higher) DID, returning [`RbacError::UnauthorizedSubmitter`] when the
    /// caller holds no qualifying role at all.
    pub fn require_active_submitter(env: &Env, address: &Address) -> Result<(), RbacError> {
        address.require_auth();

        if Self::is_suspended(env, address) {
            return Err(RbacError::SubmitterSuspended);
        }

        if Self::get_role(env, address).is_none() {
            return Err(RbacError::UnauthorizedSubmitter);
        }

        Self::require_role_min(env, address, Role::Submitter)
    }
}

pub fn check_role(env: &Env, address: &Address, min_role: Role) -> Result<(), RbacError> {
    RbacManager::require_active_role_min(env, address, min_role)
}

/// Generic permission check that maps a permission Symbol to the appropriate Role.
///
/// This function bridges the gap between string-based permission names used in
/// ESG reporting and the Role enum. It provides a unified interface for all
/// modules that need RBAC gating.
pub fn check_permission(env: &Env, address: &Address, permission: &soroban_sdk::Symbol) -> Result<(), RbacError> {
    use soroban_sdk::Symbol;
    
    // Map permission symbols to minimum required roles
    let min_role = if permission == &Symbol::new(env, "EsgDataWrite") {
        Role::Submitter
    } else if permission == &Symbol::new(env, "EsgReportGenerate") {
        Role::Submitter
    } else if permission == &Symbol::new(env, "EsgFrameworkAlign") {
        Role::Auditor
    } else {
        // Default: require Submitter role for unknown permissions
        Role::Submitter
    };
    
    RbacManager::require_active_role_min(env, address, min_role)
}

/// Call-site friendly RBAC shim mirroring the concise API used by integration
/// test suites: `init_admin`, `get_role`, `set_role`, `has_role_min`,
/// `suspend` / `reinstate`. Thin delegates over [`RbacManager`].
pub struct Rbac;

impl Rbac {
    /// Bootstraps the initial superadmin (`RbacManager::initialize_admin`).
    pub fn init_admin(env: &Env, admin: &Address) {
        RbacManager::initialize_admin(env, admin);
    }

    /// Current role of `address`, if assigned.
    pub fn get_role(env: &Env, address: &Address) -> Option<Role> {
        RbacManager::get_role(env, address)
    }

    /// Assigns/updates `target`'s role; caller must be an Admin.
    pub fn set_role(env: &Env, caller: &Address, target: &Address, role: Role) -> Result<(), RbacError> {
        RbacManager::set_role(env, caller, target, role)
    }

    /// Whether `address` holds at least `min_role`.
    pub fn has_role_min(env: &Env, address: &Address, min_role: Role) -> bool {
        RbacManager::has_role_min(env, address, min_role)
    }

    /// Suspends a submitter DID (Admin required).
    pub fn suspend(env: &Env, caller: &Address, target: &Address) -> Result<(), RbacError> {
        RbacManager::suspend_submitter(env, caller, target)
    }

    /// Clears a suspension flag (Admin required).
    pub fn reinstate(env: &Env, caller: &Address, target: &Address) -> Result<(), RbacError> {
        RbacManager::reinstate_submitter(env, caller, target)
    }
}
