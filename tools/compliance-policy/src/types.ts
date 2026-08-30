export type Severity = 'CRITICAL' | 'HIGH' | 'MEDIUM' | 'LOW';

export interface ContractEvent {
  id: string;
  topic: string;
  action: string;
  timestamp?: string;
  contract_id?: string;
  tx_hash?: string;
  payload: Record<string, any>;
}

export interface PolicyViolation {
  rule_id: string;
  title: string;
  framework: string;
  severity: Severity;
  event_id: string;
  message: string;
}

export interface PolicyMetrics {
  total_events_evaluated: number;
  total_violations: number;
  compliant: boolean;
}

export interface PolicyEvaluationResult {
  policy_package: string;
  compliant: boolean;
  violations: PolicyViolation[];
  metrics: PolicyMetrics;
  evaluated_at: string;
}

export interface DriftFinding {
  drift_id: string;
  category: string;
  severity: Severity;
  framework: string;
  current_value: any;
  baseline_value: any;
  message: string;
}

export interface DriftDetectionResult {
  baseline_id: string;
  detected_at: string;
  has_drift: boolean;
  total_findings: number;
  findings: DriftFinding[];
  score_delta: number;
  current_score_pct: number;
  baseline_score_pct: number;
}

export interface RegulatoryControlMapping {
  rule_id: string;
  clause: string;
  name: string;
}

export interface RegulatoryFrameworkConfig {
  id: string;
  name: string;
  description: string;
  controls: RegulatoryControlMapping[];
}

export interface FrameworkComplianceSummary {
  framework_id: string;
  framework_name: string;
  total_controls: number;
  passing_controls: number;
  failing_controls: number;
  compliance_pct: number;
  status: 'COMPLIANT' | 'NON_COMPLIANT' | 'WARNING';
  violations: PolicyViolation[];
}

export interface AuditReport {
  report_id: string;
  generated_at: string;
  environment: string;
  total_events: number;
  overall_compliance_score: number;
  overall_status: 'COMPLIANT' | 'NON_COMPLIANT' | 'NEEDS_ATTENTION';
  total_violations: number;
  severity_breakdown: Record<Severity, number>;
  frameworks: FrameworkComplianceSummary[];
  policy_results: PolicyEvaluationResult[];
  drift?: DriftDetectionResult;
}
