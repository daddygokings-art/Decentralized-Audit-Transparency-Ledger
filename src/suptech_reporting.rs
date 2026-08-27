#![no_std]

use crate::suptech_types::{ReportingStandard, SupervisoryReport, ReportValidationStatus};
use soroban_sdk::{contracttype, Address, Bytes, BytesN, Env, Symbol, Vec};

/// Report submission record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportSubmission {
    /// Report ID
    pub report_id: BytesN<32>,
    /// Submitter institution
    pub submitter: Address,
    /// Reporting standard
    pub standard: u8, // ReportingStandard as u8
    /// Reporting period (epoch seconds start)
    pub period_start: u64,
    /// Reporting period (epoch seconds end)
    pub period_end: u64,
    /// Submission timestamp
    pub submitted_at: u64,
    /// Report data
    pub data: Bytes,
    /// Validation status
    pub status: u8, // ReportValidationStatus as u8
}

/// Report validation result.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationResult {
    /// Report ID validated
    pub report_id: BytesN<32>,
    /// Validation timestamp
    pub validated_at: u64,
    /// Validator address
    pub validator: Address,
    /// Validation status
    pub status: u8, // ReportValidationStatus as u8
    /// Issues found
    pub issues: Vec<Bytes>,
    /// Validation score (0-100)
    pub validation_score: u32,
}

/// Reporting framework compliance checker.
pub struct ReportingManager;

impl ReportingManager {
    /// Create supervisory report
    pub fn create_report(
        env: &Env,
        standard: ReportingStandard,
        submitter: Address,
        period_start: u64,
        period_end: u64,
        report_data: Bytes,
    ) -> Result<SupervisoryReport, &'static str> {
        if report_data.is_empty() {
            return Err("Report data cannot be empty");
        }

        if period_start >= period_end {
            return Err("Invalid reporting period");
        }

        let report_id = Self::compute_report_id(env, &submitter, period_start, period_end);

        Ok(SupervisoryReport {
            report_id,
            standard: standard as u8,
            reporting_period: period_end,
            report_data,
            submitter,
            submitted_at: env.ledger().timestamp(),
            validated_at: None,
            validation_status: ReportValidationStatus::Pending as u8,
            validation_notes: Bytes::new(env),
        })
    }

    /// Compute deterministic report ID
    pub fn compute_report_id(
        env: &Env,
        submitter: &Address,
        period_start: u64,
        period_end: u64,
    ) -> BytesN<32> {
        use soroban_sdk::crypto::sha256;

        let mut input = Bytes::new(env);
        input.append(&Bytes::from_slice(env, submitter.to_xdr().as_ref()));
        input.append(&Bytes::from_slice(env, &period_start.to_le_bytes()));
        input.append(&Bytes::from_slice(env, &period_end.to_le_bytes()));

        sha256(&input)
    }

    /// Validate report format against standard
    pub fn validate_report_format(
        report: &SupervisoryReport,
        standard: ReportingStandard,
    ) -> Result<(), &'static str> {
        // Check standard matches
        if report.standard != standard as u8 {
            return Err("Report standard mismatch");
        }

        // Standard-specific validations
        match standard {
            ReportingStandard::BCBS239 => {
                // BCBS 239 requires specific data aggregation structure
                if report.report_data.len() < 100 {
                    return Err("BCBS239: Insufficient data aggregation");
                }
            }
            ReportingStandard::COREP => {
                // Common Reporting Framework validation
                if report.report_data.len() < 50 {
                    return Err("COREP: Incomplete reporting data");
                }
            }
            ReportingStandard::FINREP => {
                // Financial Reporting validation
                if report.report_data.len() < 75 {
                    return Err("FINREP: Missing financial statement data");
                }
            }
            ReportingStandard::SREP => {
                // Supervisory Review validation
                if report.report_data.len() < 60 {
                    return Err("SREP: Missing review assessment data");
                }
            }
            ReportingStandard::AMLCFT => {
                // AML/CFT validation
                if report.report_data.len() < 40 {
                    return Err("AMLCFT: Missing compliance transaction data");
                }
            }
            _ => {
                // Generic validation
                if report.report_data.is_empty() {
                    return Err("Report data required");
                }
            }
        }

        Ok(())
    }

    /// Accept report as valid
    pub fn accept_report(
        env: &Env,
        report: &mut SupervisoryReport,
        validator: Address,
    ) -> Result<ValidationResult, &'static str> {
        if !matches!(
            report.validation_status,
            s if s == ReportValidationStatus::Pending as u8
        ) {
            return Err("Report is not in pending state");
        }

        let now = env.ledger().timestamp();
        report.validated_at = Some(now);
        report.validation_status = ReportValidationStatus::Accepted as u8;

        Ok(ValidationResult {
            report_id: report.report_id.clone(),
            validated_at: now,
            validator,
            status: ReportValidationStatus::Accepted as u8,
            issues: Vec::new(env),
            validation_score: 100,
        })
    }

    /// Reject report with corrections needed
    pub fn request_corrections(
        env: &Env,
        report: &mut SupervisoryReport,
        issues: Vec<Bytes>,
        validator: Address,
    ) -> Result<ValidationResult, &'static str> {
        if issues.is_empty() {
            return Err("At least one issue must be specified");
        }

        let now = env.ledger().timestamp();
        report.validated_at = Some(now);
        report.validation_status = ReportValidationStatus::RequiresCorrections as u8;

        Ok(ValidationResult {
            report_id: report.report_id.clone(),
            validated_at: now,
            validator,
            status: ReportValidationStatus::RequiresCorrections as u8,
            issues,
            validation_score: 50,
        })
    }

    /// Flag report for investigation
    pub fn flag_report(
        env: &Env,
        report: &mut SupervisoryReport,
        reason: Bytes,
        validator: Address,
    ) -> Result<ValidationResult, &'static str> {
        let now = env.ledger().timestamp();
        report.validated_at = Some(now);
        report.validation_status = ReportValidationStatus::Flagged as u8;
        report.validation_notes = reason.clone();

        let mut issues = Vec::new(env);
        issues.push_back(reason);

        Ok(ValidationResult {
            report_id: report.report_id.clone(),
            validated_at: now,
            validator,
            status: ReportValidationStatus::Flagged as u8,
            issues,
            validation_score: 30,
        })
    }

    /// Reject report
    pub fn reject_report(
        env: &Env,
        report: &mut SupervisoryReport,
        reason: Bytes,
        validator: Address,
    ) -> Result<ValidationResult, &'static str> {
        let now = env.ledger().timestamp();
        report.validated_at = Some(now);
        report.validation_status = ReportValidationStatus::Rejected as u8;
        report.validation_notes = reason.clone();

        let mut issues = Vec::new(env);
        issues.push_back(reason);

        Ok(ValidationResult {
            report_id: report.report_id.clone(),
            validated_at: now,
            validator,
            status: ReportValidationStatus::Rejected as u8,
            issues,
            validation_score: 0,
        })
    }

    /// Compute reporting deadline (period + submission window)
    pub fn compute_reporting_deadline(period_end: u64, submission_window_days: u32) -> u64 {
        let submission_window_seconds = (submission_window_days as u64) * 86400;
        period_end.saturating_add(submission_window_seconds)
    }

    /// Check if report is overdue
    pub fn is_report_overdue(deadline: u64, current_time: u64) -> bool {
        current_time > deadline
    }

    /// Get days until deadline
    pub fn days_until_deadline(deadline: u64, current_time: u64) -> u32 {
        if current_time >= deadline {
            return 0;
        }
        ((deadline - current_time) / 86400) as u32
    }

    /// Compute data completeness score (0-100)
    pub fn compute_data_completeness(report: &SupervisoryReport) -> u32 {
        // Simple heuristic based on data size
        let size = report.report_data.len() as u32;
        let min_size = 40u32;
        let max_size = 10000u32;

        if size < min_size {
            0
        } else if size >= max_size {
            100
        } else {
            ((size - min_size) * 100) / (max_size - min_size)
        }
    }

    /// Extract reporting period from report
    pub fn get_reporting_period(report: &SupervisoryReport) -> Symbol {
        // Determine from period_end
        let quarter = (report.reporting_period / 7776000) % 4; // Rough quarter estimate
        match quarter {
            0 => Symbol::new(&[b"Q1"]),
            1 => Symbol::new(&[b"Q2"]),
            2 => Symbol::new(&[b"Q3"]),
            _ => Symbol::new(&[b"Q4"]),
        }
    }
}

/// Report statistics and tracking.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReportingStatistics {
    /// Total reports received
    pub total_reports: u32,
    /// Reports accepted
    pub reports_accepted: u32,
    /// Reports requiring corrections
    pub reports_corrections_needed: u32,
    /// Reports flagged
    pub reports_flagged: u32,
    /// Reports rejected
    pub reports_rejected: u32,
    /// Average validation time (seconds)
    pub avg_validation_time: u64,
    /// Compliance rate (0-100)
    pub compliance_rate: u32,
}

impl ReportingStatistics {
    pub fn new() -> Self {
        ReportingStatistics {
            total_reports: 0,
            reports_accepted: 0,
            reports_corrections_needed: 0,
            reports_flagged: 0,
            reports_rejected: 0,
            avg_validation_time: 0,
            compliance_rate: 100,
        }
    }

    pub fn record_submission(&mut self) {
        self.total_reports += 1;
    }

    pub fn record_acceptance(&mut self) {
        self.reports_accepted += 1;
    }

    pub fn compute_compliance_rate(&mut self) {
        if self.total_reports == 0 {
            self.compliance_rate = 100;
            return;
        }

        let compliant = self.reports_accepted;
        self.compliance_rate = ((compliant as u64 * 100) / self.total_reports as u64) as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_creation() {
        let env = soroban_sdk::Env::default();
        let submitter = soroban_sdk::Address::generate(&env);
        let data = Bytes::from_slice(&env, b"test_report_data");

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
        assert_eq!(report.reporting_period, 2000);
    }

    #[test]
    fn test_report_validation() {
        let env = soroban_sdk::Env::default();
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

        assert!(
            ReportingManager::validate_report_format(&report, ReportingStandard::BCBS239)
                .is_ok()
        );
    }

    #[test]
    fn test_reporting_deadline() {
        let deadline = ReportingManager::compute_reporting_deadline(1000000, 30);
        assert!(deadline > 1000000);
    }

    #[test]
    fn test_data_completeness() {
        let env = soroban_sdk::Env::default();
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
}
