# SupTech API Reference

## Module Exports

### suptech_types.rs

```rust
pub enum RegulatoryFramework { BIS, FSB, ECB, FED, PBOC, BoE, BoJ, NationalRegulator }
pub enum DataFeedType { TransactionStream, BalanceSnapshot, LiquidityMetrics, RiskMetrics, MarketData, ComplianceAlerts, CounterpartyExposure, StressTestResults }
pub enum ReportingStandard { BCBS239, SCOMP, COREP, FINREP, SREP, CVAR, AMLCFT }
pub enum SupervisorRole { Observer, Analyst, Administrator, SuperAdministrator }
pub enum ReportValidationStatus { Pending, Accepted, RequiresCorrections, Flagged, Rejected }
pub enum AlertStatus { New, InvestigationOngoing, Resolved, Escalated, Dismissed }

pub struct Supervisor {
    pub supervisor_id: BytesN<32>,
    pub address: Address,
    pub framework: u8,
    pub role: u8,
    pub subscribed_feeds: Vec<u8>,
    pub created_at: u64,
    pub is_active: bool,
    pub name: Bytes,
}

pub struct DataFeed {
    pub feed_id: BytesN<32>,
    pub feed_type: u8,
    pub current_data: Bytes,
    pub last_updated: u64,
    pub update_frequency: u64,
    pub subscriber_count: u32,
    pub is_active: bool,
    pub metadata: Bytes,
}

pub struct SupervisoryReport {
    pub report_id: BytesN<32>,
    pub standard: u8,
    pub reporting_period: u64,
    pub report_data: Bytes,
    pub submitter: Address,
    pub submitted_at: u64,
    pub validated_at: Option<u64>,
    pub validation_status: u8,
    pub validation_notes: Bytes,
}

pub struct SupervisionRule {
    pub rule_id: BytesN<32>,
    pub name: Bytes,
    pub framework: u8,
    pub condition: Bytes,
    pub action: Bytes,
    pub severity: u8,
    pub is_active: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct ComplianceAlert {
    pub alert_id: BytesN<32>,
    pub rule_id: BytesN<32>,
    pub institution: Address,
    pub severity: u8,
    pub message: Bytes,
    pub triggered_at: u64,
    pub supporting_data: Bytes,
    pub status: u8,
    pub resolution_notes: Bytes,
}

pub struct SupTechConfig {
    pub max_supervisors: u32,
    pub supervisor_count: u32,
    pub max_data_feeds: u32,
    pub data_feed_count: u32,
    pub real_time_monitoring_enabled: bool,
    pub automated_rules_enabled: bool,
    pub alert_escalation_threshold: u8,
    pub data_retention_seconds: u64,
}
```

### suptech_feeds.rs

```rust
pub struct DataPoint {
    pub timestamp: u64,
    pub feed_type: u8,
    pub payload: Bytes,
    pub data_hash: BytesN<32>,
}

pub struct FeedSubscription {
    pub subscription_id: BytesN<32>,
    pub feed_id: BytesN<32>,
    pub subscriber: Address,
    pub filter_criteria: Bytes,
    pub created_at: u64,
    pub is_active: bool,
    pub last_data_received: Option<u64>,
    pub data_point_count: u32,
}

pub struct FeedManager;

impl FeedManager {
    pub fn create_feed(env: &Env, feed_type: DataFeedType, initial_data: Bytes) -> Result<DataFeed, &'static str>;
    pub fn publish_data_point(env: &Env, feed: &mut DataFeed, new_data: Bytes) -> Result<DataPoint, &'static str>;
    pub fn is_data_fresh(feed: &DataFeed, current_time: u64) -> bool;
    pub fn is_data_stale(feed: &DataFeed, current_time: u64) -> bool;
    pub fn create_subscription(env: &Env, feed_id: BytesN<32>, subscriber: Address) -> Result<FeedSubscription, &'static str>;
    pub fn record_data_receipt(subscription: &mut FeedSubscription, timestamp: u64) -> Result<(), &'static str>;
    pub fn compute_data_quality_score(feed: &DataFeed, current_time: u64, subscriber_count_healthy: u32) -> u32;
    pub fn validate_data_point(data_point: &DataPoint, feed: &DataFeed) -> Result<(), &'static str>;
    pub fn get_feed_update_lag(feed: &DataFeed, current_time: u64) -> u64;
}
```

### suptech_reporting.rs

```rust
pub struct ReportingManager;

impl ReportingManager {
    pub fn create_report(env: &Env, standard: ReportingStandard, submitter: Address, period_start: u64, period_end: u64, report_data: Bytes) -> Result<SupervisoryReport, &'static str>;
    pub fn validate_report_format(report: &SupervisoryReport, standard: ReportingStandard) -> Result<(), &'static str>;
    pub fn accept_report(env: &Env, report: &mut SupervisoryReport, validator: Address) -> Result<ValidationResult, &'static str>;
    pub fn request_corrections(env: &Env, report: &mut SupervisoryReport, issues: Vec<Bytes>, validator: Address) -> Result<ValidationResult, &'static str>;
    pub fn flag_report(env: &Env, report: &mut SupervisoryReport, reason: Bytes, validator: Address) -> Result<ValidationResult, &'static str>;
    pub fn reject_report(env: &Env, report: &mut SupervisoryReport, reason: Bytes, validator: Address) -> Result<ValidationResult, &'static str>;
    pub fn compute_reporting_deadline(period_end: u64, submission_window_days: u32) -> u64;
    pub fn is_report_overdue(deadline: u64, current_time: u64) -> bool;
    pub fn days_until_deadline(deadline: u64, current_time: u64) -> u32;
    pub fn compute_data_completeness(report: &SupervisoryReport) -> u32;
}
```

### suptech_api.rs

```rust
pub struct DashboardView {
    pub view_id: BytesN<32>,
    pub owner: Address,
    pub name: Bytes,
    pub widgets_config: Bytes,
    pub refresh_interval: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct AlertSubscription {
    pub subscription_id: BytesN<32>,
    pub subscriber: Address,
    pub severity_threshold: u8,
    pub category_filters: Vec<Bytes>,
    pub is_active: bool,
    pub created_at: u64,
    pub alerts_received: u32,
}

pub struct SupervisorAPI;

impl SupervisorAPI {
    pub fn register_supervisor(env: &Env, address: Address, framework: RegulatoryFramework, role: SupervisorRole, name: Bytes) -> Result<Supervisor, &'static str>;
    pub fn check_permission(supervisor: &Supervisor, operation: &str) -> Result<(), &'static str>;
    pub fn execute_query(env: &Env, executor: Address, query_type: Bytes, parameters: Bytes) -> Result<DashboardQuery, &'static str>;
    pub fn create_dashboard_view(env: &Env, owner: Address, name: Bytes, widgets_config: Bytes, refresh_interval: u64) -> Result<DashboardView, &'static str>;
    pub fn subscribe_to_alerts(env: &Env, subscriber: Address, severity_threshold: u8) -> Result<AlertSubscription, &'static str>;
    pub fn should_deliver_alert(alert: &ComplianceAlert, subscription: &AlertSubscription) -> bool;
    pub fn record_alert_delivery(subscription: &mut AlertSubscription) -> Result<(), &'static str>;
    pub fn deactivate_supervisor(supervisor: &mut Supervisor);
    pub fn reactivate_supervisor(supervisor: &mut Supervisor);
}
```

### suptech_rules.rs

```rust
pub struct SupervisionRule { /* ... */ }

pub struct RuleSet {
    pub ruleset_id: BytesN<32>,
    pub framework: u8,
    pub rules: Vec<BytesN<32>>,
    pub version: u32,
    pub is_active: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct RulesEngine;

impl RulesEngine {
    pub fn create_rule(env: &Env, framework: RegulatoryFramework, name: Bytes, condition: Bytes, action: Bytes, severity: u8) -> Result<SupervisionRule, &'static str>;
    pub fn evaluate_rule(env: &Env, rule: &SupervisionRule, context: Bytes) -> Result<RuleEvaluation, &'static str>;
    pub fn generate_alert_from_rule(env: &Env, rule: &SupervisionRule, institution: Address, supporting_data: Bytes) -> Result<ComplianceAlert, &'static str>;
    pub fn update_rule_condition(env: &Env, rule: &mut SupervisionRule, new_condition: Bytes) -> Result<(), &'static str>;
    pub fn disable_rule(rule: &mut SupervisionRule);
    pub fn enable_rule(rule: &mut SupervisionRule);
    pub fn create_ruleset(env: &Env, framework: RegulatoryFramework) -> Result<RuleSet, &'static str>;
    pub fn add_rule_to_set(ruleset: &mut RuleSet, rule_id: BytesN<32>) -> Result<(), &'static str>;
    pub fn remove_rule_from_set(ruleset: &mut RuleSet, rule_id: &BytesN<32>) -> Result<(), &'static str>;
    pub fn execute_ruleset(env: &Env, ruleset: &RuleSet, rules: &Vec<SupervisionRule>, context: Bytes) -> Result<Vec<RuleEvaluation>, &'static str>;
    pub fn compute_ruleset_stats(ruleset: &RuleSet, rules: &Vec<SupervisionRule>) -> RuleSetStatistics;
}
```

### suptech_integration.rs

```rust
pub enum EndpointStatus { Connected, Disconnected, Error, Maintenance }
pub enum TransmissionStatus { Pending, Transmitted, Acknowledged, Failed, RetransmissionScheduled }

pub struct RegulatoryEndpoint {
    pub endpoint_id: BytesN<32>,
    pub framework: u8,
    pub endpoint_address: Bytes,
    pub protocol_version: u32,
    pub last_sync: u64,
    pub is_active: bool,
    pub status: u8,
    pub sync_frequency: u64,
}

pub struct TransmissionRecord {
    pub transmission_id: BytesN<32>,
    pub source: Address,
    pub destination: Address,
    pub data_type: Bytes,
    pub data_hash: BytesN<32>,
    pub transmitted_at: u64,
    pub acknowledged_at: Option<u64>,
    pub status: u8,
}

pub struct IntegrationManager;

impl IntegrationManager {
    pub fn register_endpoint(env: &Env, framework: RegulatoryFramework, endpoint_address: Bytes, protocol_version: u32) -> Result<RegulatoryEndpoint, &'static str>;
    pub fn create_transmission(env: &Env, source: Address, destination: Address, data_type: Bytes, data_hash: BytesN<32>) -> Result<TransmissionRecord, &'static str>;
    pub fn acknowledge_transmission(env: &Env, transmission: &mut TransmissionRecord) -> Result<(), &'static str>;
    pub fn fail_transmission(env: &Env, transmission: &mut TransmissionRecord) -> Result<(), &'static str>;
    pub fn schedule_retransmission(transmission: &mut TransmissionRecord) -> Result<(), &'static str>;
    pub fn is_transmission_acknowledged(transmission: &TransmissionRecord) -> bool;
    pub fn is_endpoint_healthy(endpoint: &RegulatoryEndpoint, current_time: u64) -> bool;
    pub fn sync_endpoint(env: &Env, endpoint: &mut RegulatoryEndpoint) -> Result<(), &'static str>;
    pub fn endpoint_error(endpoint: &mut RegulatoryEndpoint);
    pub fn is_acknowledgment_overdue(transmission: &TransmissionRecord, current_time: u64, timeout_seconds: u64) -> bool;
    pub fn get_bis_rules() -> Vec<Bytes>;
    pub fn get_fsb_standards() -> Vec<Bytes>;
    pub fn get_national_requirements(framework: RegulatoryFramework) -> Vec<Bytes>;
}
```

## Error Codes

| Error | Meaning | Solution |
|-------|---------|----------|
| `Initial data cannot be empty` | Feed requires data | Provide valid data payload |
| `Feed is not active` | Feed deactivated | Reactivate feed first |
| `Report data cannot be empty` | Report requires content | Include report data |
| `Invalid reporting period` | Period dates invalid | Ensure start < end |
| `Insufficient permissions` | Role lacks access | Upgrade supervisor role |
| `Rule is not active` | Rule disabled | Enable rule first |
| `Report is not in pending state` | Already validated | Cannot re-validate |
| `Transmission is not in transmitted state` | Invalid state | Check transmission status |
| `Endpoint address cannot be empty` | Missing endpoint | Provide valid endpoint |

## Performance Metrics

- **Feed creation:** O(1)
- **Data point publishing:** O(1)
- **Report validation:** O(data_size)
- **Rule evaluation:** O(number_of_rules)
- **Transmission:** O(1)
- **Endpoint health check:** O(1)

## Constants

- **Default max supervisors:** 1000
- **Default max feeds:** 100
- **Default alert escalation threshold:** 7 (severity 0-10)
- **Default data retention:** 90 days
- **Data feed quality factors:** Freshness, subscriber count, active status

## Testing

```bash
# Run all SupTech tests
cargo test suptech_

# Run specific module tests
cargo test suptech_types::
cargo test suptech_feeds::
cargo test suptech_reporting::
cargo test suptech_api::
cargo test suptech_rules::
cargo test suptech_integration::

# Run with output
cargo test suptech_ -- --nocapture
```

## Common Workflows

### Register Supervisor
```rust
let supervisor = SupervisorAPI::register_supervisor(
    &env,
    supervisor_address,
    RegulatoryFramework::FSB,
    SupervisorRole::Analyst,
    Bytes::from_slice(&env, b"John Analyst"),
)?;
```

### Create Data Feed
```rust
let feed = FeedManager::create_feed(
    &env,
    DataFeedType::TransactionStream,
    Bytes::from_slice(&env, b"initial_data"),
)?;
```

### Publish Report
```rust
let report = ReportingManager::create_report(
    &env,
    ReportingStandard::BCBS239,
    institution,
    1000,  // period_start
    2000,  // period_end
    report_bytes,
)?;
```

### Create Supervision Rule
```rust
let rule = RulesEngine::create_rule(
    &env,
    RegulatoryFramework::FSB,
    Bytes::from_slice(&env, b"High Volume Alert"),
    Bytes::from_slice(&env, b"volume > 1000000"),
    Bytes::from_slice(&env, b"alert_supervisor"),
    8,  // severity
)?;
```

### Register Regulatory Endpoint
```rust
let endpoint = IntegrationManager::register_endpoint(
    &env,
    RegulatoryFramework::FSB,
    Bytes::from_slice(&env, b"https://fsb.example.org"),
    1,  // protocol_version
)?;
```
