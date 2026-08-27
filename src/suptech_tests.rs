// SupTech Integration Tests
#![cfg(test)]

use soroban_sdk::{Bytes, BytesN, Env};

use crate::suptech_types::*;
use crate::suptech_feeds::*;
use crate::suptech_reporting::*;
use crate::suptech_api::*;
use crate::suptech_rules::*;
use crate::suptech_integration::*;

fn create_test_env() -> Env {
    Env::default()
}

// ============ SupTech Types Tests ============

#[test]
fn test_regulatory_framework_names() {
    assert_eq!(RegulatoryFramework::BIS.name(), "Basel Committee on Banking Supervision");
    assert_eq!(RegulatoryFramework::FSB.name(), "Financial Stability Board");
    assert_eq!(RegulatoryFramework::ECB.name(), "European Central Bank");
}

#[test]
fn test_data_feed_update_frequencies() {
    assert_eq!(DataFeedType::TransactionStream.update_frequency_seconds(), 1);
    assert_eq!(DataFeedType::BalanceSnapshot.update_frequency_seconds(), 300);
    assert_eq!(DataFeedType::RiskMetrics.update_frequency_seconds(), 3600);
    assert_eq!(DataFeedType::StressTestResults.update_frequency_seconds(), 86400);
}

#[test]
fn test_supervisor_role_permissions() {
    assert!(SupervisorRole::Observer.can_read());
    assert!(!SupervisorRole::Observer.can_query());

    assert!(SupervisorRole::Analyst.can_read());
    assert!(SupervisorRole::Analyst.can_query());
    assert!(!SupervisorRole::Analyst.can_manage_rules());

    assert!(SupervisorRole::Administrator.can_manage_rules());
    assert!(!SupervisorRole::Administrator.can_override());

    assert!(SupervisorRole::SuperAdministrator.can_override());
}

#[test]
fn test_reporting_standard_formats() {
    assert_eq!(ReportingStandard::BCBS239.description(), "Principles for effective risk data aggregation");
    assert_eq!(ReportingStandard::COREP.description(), "Common Reporting Framework");
    assert_eq!(ReportingStandard::FINREP.description(), "Financial Reporting");
}

#[test]
fn test_report_validation_status() {
    assert!(!ReportValidationStatus::Pending.is_terminal());
    assert!(ReportValidationStatus::Accepted.is_terminal());
    assert!(ReportValidationStatus::Rejected.is_terminal());
}

#[test]
fn test_suptech_config_defaults() {
    let config = SupTechConfig::default();
    assert_eq!(config.max_supervisors, 1000);
    assert_eq!(config.max_data_feeds, 100);
    assert!(config.can_add_supervisor());
    assert!(config.can_add_feed());
}

// ============ SupTech Feeds Tests ============

#[test]
fn test_feed_creation() {
    let env = create_test_env();
    let data = Bytes::from_slice(&env, b"test_data");

    let feed = FeedManager::create_feed(&env, DataFeedType::TransactionStream, data).unwrap();
    assert!(feed.is_active);
    assert_eq!(feed.update_frequency, 1);
}

#[test]
fn test_data_freshness() {
    let env = create_test_env();
    let feed = DataFeed {
        feed_id: BytesN::zero(),
        feed_type: DataFeedType::BalanceSnapshot as u8,
        current_data: Bytes::from_slice(&env, b"data"),
        last_updated: 1000,
        update_frequency: 300,
        subscriber_count: 5,
        is_active: true,
        metadata: Bytes::new(&env),
    };

    assert!(FeedManager::is_data_fresh(&feed, 1200));
    assert!(!FeedManager::is_data_fresh(&feed, 1400));
}

#[test]
fn test_subscription_creation() {
    let env = create_test_env();
    let feed_id = BytesN::zero();
    let subscriber = soroban_sdk::Address::generate(&env);

    let sub = FeedManager::create_subscription(&env, feed_id, subscriber.clone()).unwrap();
    assert!(sub.is_active);
    assert_eq!(sub.data_point_count, 0);
}

#[test]
fn test_data_quality_score() {
    let env = create_test_env();
    let feed = DataFeed {
        feed_id: BytesN::zero(),
        feed_type: DataFeedType::TransactionStream as u8,
        current_data: Bytes::from_slice(&env, b"data"),
        last_updated: 1000,
        update_frequency: 300,
        subscriber_count: 10,
        is_active: true,
        metadata: Bytes::new(&env),
    };

    let score = FeedManager::compute_data_quality_score(&feed, 1200, 5);
    assert!(score > 80);
}

// ============ SupTech Reporting Tests ============

#[test]
fn test_report_creation() {
    let env = create_test_env();
    let submitter = soroban_sdk::Address::generate(&env);
    let data = Bytes::from_slice(&env, b"x".repeat(100).as_slice());

    let report = ReportingManager::create_report(
        &env,
        ReportingStandard::BCBS239,
        submitter,
        1000,
        2000,
        data,
    )
    .unwrap();

    assert_eq!(report.validation_status, ReportValidationStatus::Pending as u8);
}

#[test]
fn test_report_validation() {
    let env = create_test_env();
    let submitter = soroban_sdk::Address::generate(&env);
    let data = Bytes::from_slice(&env, b"x".repeat(100).as_slice());

    let report = ReportingManager::create_report(
        &env,
        ReportingStandard::BCBS239,
        submitter,
        1000,
        2000,
        data,
    )
    .unwrap();

    assert!(ReportingManager::validate_report_format(&report, ReportingStandard::BCBS239).is_ok());
}

#[test]
fn test_data_completeness() {
    let env = create_test_env();
    let submitter = soroban_sdk::Address::generate(&env);
    let small_data = Bytes::from_slice(&env, b"small");
    let large_data = Bytes::from_slice(&env, b"x".repeat(5000).as_slice());

    let small_report = ReportingManager::create_report(
        &env,
        ReportingStandard::COREP,
        submitter.clone(),
        1000,
        2000,
        small_data,
    )
    .unwrap();

    let large_report = ReportingManager::create_report(
        &env,
        ReportingStandard::COREP,
        submitter,
        1000,
        2000,
        large_data,
    )
    .unwrap();

    let small_score = ReportingManager::compute_data_completeness(&small_report);
    let large_score = ReportingManager::compute_data_completeness(&large_report);

    assert!(large_score > small_score);
}

// ============ SupTech API Tests ============

#[test]
fn test_supervisor_registration() {
    let env = create_test_env();
    let address = soroban_sdk::Address::generate(&env);
    let name = Bytes::from_slice(&env, b"Test Supervisor");

    let supervisor = SupervisorAPI::register_supervisor(
        &env,
        address,
        RegulatoryFramework::FSB,
        SupervisorRole::Analyst,
        name,
    )
    .unwrap();

    assert!(supervisor.is_active);
    assert_eq!(supervisor.framework, RegulatoryFramework::FSB as u8);
}

#[test]
fn test_permission_checking() {
    let env = create_test_env();
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
}

#[test]
fn test_dashboard_view_creation() {
    let env = create_test_env();
    let owner = soroban_sdk::Address::generate(&env);
    let name = Bytes::from_slice(&env, b"Dashboard");
    let config = Bytes::from_slice(&env, b"{}");

    let view = SupervisorAPI::create_dashboard_view(&env, owner, name, config, 60).unwrap();
    assert_eq!(view.refresh_interval, 60);
}

#[test]
fn test_alert_subscription() {
    let env = create_test_env();
    let subscriber = soroban_sdk::Address::generate(&env);

    let sub = SupervisorAPI::subscribe_to_alerts(&env, subscriber, 5).unwrap();
    assert!(sub.is_active);
    assert_eq!(sub.severity_threshold, 5);
}

// ============ SupTech Rules Tests ============

#[test]
fn test_rule_creation() {
    let env = create_test_env();
    let name = Bytes::from_slice(&env, b"Test Rule");
    let condition = Bytes::from_slice(&env, b"amount > 1000000");
    let action = Bytes::from_slice(&env, b"alert");

    let rule = RulesEngine::create_rule(
        &env,
        RegulatoryFramework::FSB,
        name,
        condition,
        action,
        7,
    )
    .unwrap();

    assert!(rule.is_active);
    assert_eq!(rule.severity, 7);
}

#[test]
fn test_rule_evaluation() {
    let env = create_test_env();
    let name = Bytes::from_slice(&env, b"Test");
    let condition = Bytes::from_slice(&env, b"test");
    let action = Bytes::from_slice(&env, b"alert");

    let rule = RulesEngine::create_rule(
        &env,
        RegulatoryFramework::BIS,
        name,
        condition,
        action,
        5,
    )
    .unwrap();

    let context = Bytes::from_slice(&env, b"transaction data");
    let eval = RulesEngine::evaluate_rule(&env, &rule, context).unwrap();
    assert_eq!(eval.severity_score, 5);
}

#[test]
fn test_ruleset_creation() {
    let env = create_test_env();
    let ruleset = RulesEngine::create_ruleset(&env, RegulatoryFramework::FSB).unwrap();

    assert!(ruleset.is_active);
    assert_eq!(ruleset.version, 1);
}

// ============ SupTech Integration Tests ============

#[test]
fn test_endpoint_registration() {
    let env = create_test_env();
    let address = Bytes::from_slice(&env, b"https://bis.example.com");

    let endpoint = IntegrationManager::register_endpoint(
        &env,
        RegulatoryFramework::BIS,
        address,
        1,
    )
    .unwrap();

    assert!(endpoint.is_active);
    assert_eq!(endpoint.status, EndpointStatus::Connected as u8);
}

#[test]
fn test_transmission_acknowledgment() {
    let env = create_test_env();
    let source = soroban_sdk::Address::generate(&env);
    let dest = soroban_sdk::Address::generate(&env);
    let data_type = Bytes::from_slice(&env, b"report");

    let mut transmission =
        IntegrationManager::create_transmission(&env, source, dest, data_type, BytesN::zero())
            .unwrap();

    assert!(IntegrationManager::acknowledge_transmission(&env, &mut transmission).is_ok());
    assert!(IntegrationManager::is_transmission_acknowledged(&transmission));
}

#[test]
fn test_endpoint_health() {
    let env = create_test_env();
    let address = Bytes::from_slice(&env, b"https://example.com");

    let endpoint =
        IntegrationManager::register_endpoint(&env, RegulatoryFramework::FSB, address, 1)
            .unwrap();

    assert!(IntegrationManager::is_endpoint_healthy(&endpoint, env.ledger().timestamp()));
}

#[test]
fn test_bis_rules() {
    let rules = IntegrationManager::get_bis_rules();
    assert!(rules.len() > 0);
}

#[test]
fn test_fsb_standards() {
    let standards = IntegrationManager::get_fsb_standards();
    assert!(standards.len() > 0);
}

// ============ Integration Workflow Tests ============

#[test]
fn test_complete_supervision_workflow() {
    let env = create_test_env();

    // 1. Register supervisor
    let supervisor_addr = soroban_sdk::Address::generate(&env);
    let supervisor = SupervisorAPI::register_supervisor(
        &env,
        supervisor_addr.clone(),
        RegulatoryFramework::FSB,
        SupervisorRole::Administrator,
        Bytes::from_slice(&env, b"Supervisor"),
    )
    .unwrap();

    assert!(supervisor.is_active);

    // 2. Create and publish data feed
    let feed_data = Bytes::from_slice(&env, b"market_data");
    let feed = FeedManager::create_feed(&env, DataFeedType::MarketData, feed_data).unwrap();
    assert!(feed.is_active);

    // 3. Create supervision rule
    let rule = RulesEngine::create_rule(
        &env,
        RegulatoryFramework::FSB,
        Bytes::from_slice(&env, b"High Volume Alert"),
        Bytes::from_slice(&env, b"volume > threshold"),
        Bytes::from_slice(&env, b"alert"),
        8,
    )
    .unwrap();

    assert_eq!(rule.severity, 8);

    // 4. Register regulator endpoint
    let endpoint = IntegrationManager::register_endpoint(
        &env,
        RegulatoryFramework::FSB,
        Bytes::from_slice(&env, b"https://fsb.org"),
        1,
    )
    .unwrap();

    assert!(endpoint.is_active);
}

#[test]
fn test_reporting_validation_workflow() {
    let env = create_test_env();

    // 1. Create report
    let submitter = soroban_sdk::Address::generate(&env);
    let report_data = Bytes::from_slice(&env, b"x".repeat(150).as_slice());

    let mut report = ReportingManager::create_report(
        &env,
        ReportingStandard::BCBS239,
        submitter,
        1000,
        2000,
        report_data,
    )
    .unwrap();

    assert_eq!(report.validation_status, ReportValidationStatus::Pending as u8);

    // 2. Validate format
    assert!(ReportingManager::validate_report_format(&report, ReportingStandard::BCBS239).is_ok());

    // 3. Accept report
    let validator = soroban_sdk::Address::generate(&env);
    let result =
        ReportingManager::accept_report(&env, &mut report, validator).unwrap();

    assert_eq!(result.validation_score, 100);
    assert_eq!(report.validation_status, ReportValidationStatus::Accepted as u8);
}
