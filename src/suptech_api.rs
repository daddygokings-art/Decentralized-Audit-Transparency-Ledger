#![no_std]

use crate::suptech_types::{Supervisor, SupervisorRole, ComplianceAlert, AlertStatus, RegulatoryFramework};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Dashboard query for supervisor analytics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardQuery {
    /// Query ID
    pub query_id: BytesN<32>,
    /// Query executor
    pub executor: Address,
    /// Query type (e.g., "transaction_summary", "risk_overview")
    pub query_type: Bytes,
    /// Query parameters (filters, date ranges, etc.)
    pub parameters: Bytes,
    /// Executed at
    pub executed_at: u64,
    /// Query results
    pub results: Bytes,
    /// Query execution time (milliseconds)
    pub execution_time_ms: u32,
}

/// Dashboard view configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardView {
    /// View ID
    pub view_id: BytesN<32>,
    /// Owner/supervisor
    pub owner: Address,
    /// View name
    pub name: Bytes,
    /// Widgets configuration (serialized)
    pub widgets_config: Bytes,
    /// Refresh interval (seconds)
    pub refresh_interval: u64,
    /// Created at
    pub created_at: u64,
    /// Last modified at
    pub updated_at: u64,
}

/// Alert subscription for supervisors.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlertSubscription {
    /// Subscription ID
    pub subscription_id: BytesN<32>,
    /// Subscriber
    pub subscriber: Address,
    /// Alert types to subscribe to (severity levels)
    pub severity_threshold: u8, // 0-10, receive alerts at or above this level
    /// Alert categories (filters)
    pub category_filters: Vec<Bytes>,
    /// Active
    pub is_active: bool,
    /// Created at
    pub created_at: u64,
    /// Alerts received count
    pub alerts_received: u32,
}

/// Supervisor API manager.
pub struct SupervisorAPI;

impl SupervisorAPI {
    /// Register supervisor
    pub fn register_supervisor(
        env: &Env,
        address: Address,
        framework: RegulatoryFramework,
        role: SupervisorRole,
        name: Bytes,
    ) -> Result<Supervisor, &'static str> {
        let supervisor_id = Self::compute_supervisor_id(env, &address);

        Ok(Supervisor {
            supervisor_id,
            address,
            framework: framework as u8,
            role: role as u8,
            subscribed_feeds: Vec::new(env),
            created_at: env.ledger().timestamp(),
            is_active: true,
            name,
        })
    }

    /// Compute supervisor ID
    pub fn compute_supervisor_id(env: &Env, address: &Address) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, address.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, b"SUPERVISOR"));

        env.crypto().sha256(&input)
    }

    /// Check if supervisor has permission for operation
    pub fn check_permission(
        supervisor: &Supervisor,
        operation: &str,
    ) -> Result<(), &'static str> {
        if !supervisor.is_active {
            return Err("Supervisor is not active");
        }

        let role = match supervisor.role {
            r if r == SupervisorRole::Observer as u8 => SupervisorRole::Observer,
            r if r == SupervisorRole::Analyst as u8 => SupervisorRole::Analyst,
            r if r == SupervisorRole::Administrator as u8 => SupervisorRole::Administrator,
            r if r == SupervisorRole::SuperAdministrator as u8 => SupervisorRole::SuperAdministrator,
            _ => return Err("Invalid supervisor role"),
        };

        match operation {
            "read_data" => {
                if !role.can_read() {
                    return Err("Insufficient permissions for read access");
                }
            }
            "query_system" => {
                if !role.can_query() {
                    return Err("Insufficient permissions for queries");
                }
            }
            "manage_rules" => {
                if !role.can_manage_rules() {
                    return Err("Insufficient permissions to manage rules");
                }
            }
            "override_rule" => {
                if !role.can_override() {
                    return Err("Insufficient permissions for overrides");
                }
            }
            _ => return Err("Unknown operation"),
        }

        Ok(())
    }

    /// Execute dashboard query
    pub fn execute_query(
        env: &Env,
        executor: Address,
        query_type: Bytes,
        parameters: Bytes,
    ) -> Result<DashboardQuery, &'static str> {
        if query_type.is_empty() {
            return Err("Query type cannot be empty");
        }

        let query_id = Self::compute_query_id(env, &executor, &query_type);

        Ok(DashboardQuery {
            query_id,
            executor,
            query_type,
            parameters,
            executed_at: env.ledger().timestamp(),
            results: Bytes::new(env),
            execution_time_ms: 0,
        })
    }

    /// Compute query ID
    pub fn compute_query_id(
        env: &Env,
        executor: &Address,
        query_type: &Bytes,
    ) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, executor.to_xdr().as_ref()));
        input.append(query_type);
        input.append(&Bytes::from_slice(env, &env.ledger().timestamp().to_le_bytes()));

        env.crypto().sha256(&input)
    }

    /// Create dashboard view
    pub fn create_dashboard_view(
        env: &Env,
        owner: Address,
        name: Bytes,
        widgets_config: Bytes,
        refresh_interval: u64,
    ) -> Result<DashboardView, &'static str> {
        if name.is_empty() {
            return Err("Dashboard name cannot be empty");
        }

        if refresh_interval < 5 {
            return Err("Refresh interval must be at least 5 seconds");
        }

        let view_id = Self::compute_view_id(env, &owner, &name);

        Ok(DashboardView {
            view_id,
            owner,
            name,
            widgets_config,
            refresh_interval,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        })
    }

    /// Compute dashboard view ID
    pub fn compute_view_id(env: &Env, owner: &Address, name: &Bytes) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, owner.to_xdr().as_ref()));
        input.append(name);

        env.crypto().sha256(&input)
    }

    /// Subscribe to alerts
    pub fn subscribe_to_alerts(
        env: &Env,
        subscriber: Address,
        severity_threshold: u8,
    ) -> Result<AlertSubscription, &'static str> {
        if severity_threshold > 10 {
            return Err("Severity must be 0-10");
        }

        let subscription_id = Self::compute_alert_subscription_id(env, &subscriber);

        Ok(AlertSubscription {
            subscription_id,
            subscriber,
            severity_threshold,
            category_filters: Vec::new(env),
            is_active: true,
            created_at: env.ledger().timestamp(),
            alerts_received: 0,
        })
    }

    /// Compute alert subscription ID
    pub fn compute_alert_subscription_id(env: &Env, subscriber: &Address) -> BytesN<32> {
        
        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, subscriber.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, b"ALERT_SUB"));

        env.crypto().sha256(&input)
    }

    /// Deliver alert to subscriber
    pub fn should_deliver_alert(
        alert: &ComplianceAlert,
        subscription: &AlertSubscription,
    ) -> bool {
        if !subscription.is_active {
            return false;
        }

        // Check severity threshold
        if alert.severity < subscription.severity_threshold {
            return false;
        }

        // Check category filters (if any)
        if !subscription.category_filters.is_empty() {
            // Simple check: alert message should contain at least one filter term
            let has_matching_category = subscription
                .category_filters
                .iter()
                .any(|filter| {
                    // In a real implementation, would do proper text matching
                    true
                });

            if !has_matching_category && !subscription.category_filters.is_empty() {
                return false;
            }
        }

        true
    }

    /// Log alert delivery
    pub fn record_alert_delivery(
        subscription: &mut AlertSubscription,
    ) -> Result<(), &'static str> {
        subscription.alerts_received = subscription.alerts_received.saturating_add(1);
        Ok(())
    }

    /// Deactivate supervisor
    pub fn deactivate_supervisor(supervisor: &mut Supervisor) {
        supervisor.is_active = false;
    }

    /// Reactivate supervisor
    pub fn reactivate_supervisor(supervisor: &mut Supervisor) {
        supervisor.is_active = true;
    }

    /// Get supervision dashboard summary (synthetic)
    pub fn get_dashboard_summary(
        env: &Env,
        total_alerts: u32,
        high_severity_alerts: u32,
        reports_pending: u32,
    ) -> DashboardSummary {
        DashboardSummary {
            timestamp: env.ledger().timestamp(),
            total_alerts,
            high_severity_alerts,
            reports_pending,
            data_feeds_healthy: 95,
            compliance_score: 85,
        }
    }
}

/// Dashboard summary for supervisors.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardSummary {
    /// Current timestamp
    pub timestamp: u64,
    /// Total active alerts
    pub total_alerts: u32,
    /// High severity alerts (7+)
    pub high_severity_alerts: u32,
    /// Reports awaiting validation
    pub reports_pending: u32,
    /// Data feed health percentage (0-100)
    pub data_feeds_healthy: u32,
    /// Overall compliance score (0-100)
    pub compliance_score: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervisor_registration() {
        let env = soroban_sdk::Env::default();
        let address = soroban_sdk::Address::generate(&env);
        let name = Bytes::from_slice(&env, b"Test Supervisor");

        let supervisor = SupervisorAPI::register_supervisor(
            &env,
            address.clone(),
            RegulatoryFramework::FSB,
            SupervisorRole::Analyst,
            name,
        )
        .unwrap();

        assert!(supervisor.is_active);
        assert_eq!(supervisor.framework, RegulatoryFramework::FSB as u8);
        assert_eq!(supervisor.role, SupervisorRole::Analyst as u8);
    }

    #[test]
    fn test_permission_checking() {
        let env = soroban_sdk::Env::default();
        let address = soroban_sdk::Address::generate(&env);
        let name = Bytes::from_slice(&env, b"Test");

        let supervisor = SupervisorAPI::register_supervisor(
            &env,
            address,
            RegulatoryFramework::BIS,
            SupervisorRole::Observer,
            name,
        )
        .unwrap();

        assert!(SupervisorAPI::check_permission(&supervisor, "read_data").is_ok());
        assert!(SupervisorAPI::check_permission(&supervisor, "query_system").is_err());
        assert!(SupervisorAPI::check_permission(&supervisor, "manage_rules").is_err());
    }

    #[test]
    fn test_dashboard_view_creation() {
        let env = soroban_sdk::Env::default();
        let owner = soroban_sdk::Address::generate(&env);
        let name = Bytes::from_slice(&env, b"Dashboard");
        let config = Bytes::from_slice(&env, b"{}");

        let view = SupervisorAPI::create_dashboard_view(
            &env,
            owner,
            name,
            config,
            60,
        )
        .unwrap();

        assert_eq!(view.refresh_interval, 60);
    }

    #[test]
    fn test_alert_subscription() {
        let env = soroban_sdk::Env::default();
        let subscriber = soroban_sdk::Address::generate(&env);

        let sub =
            SupervisorAPI::subscribe_to_alerts(&env, subscriber, 5).unwrap();
        assert!(sub.is_active);
        assert_eq!(sub.severity_threshold, 5);
    }
}
