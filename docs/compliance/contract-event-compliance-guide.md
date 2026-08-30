# Contract Event Compliance Automation Guide

This guide describes how to author, test, and execute OPA/Rego policies for smart contract event compliance in the AuditLedger ecosystem.

## 1. Directory Structure

```
policies/compliance/
├── baseline/
│   ├── regulatory_baselines.rego
│   └── regulatory_baselines_test.rego
├── config/
│   └── regulatory-frameworks.yaml
├── events/
│   ├── anti_corruption.rego
│   ├── anti_corruption_test.rego
│   ├── data_retention.rego
│   ├── data_retention_test.rego
│   ├── export_controls.rego
│   ├── export_controls_test.rego
│   ├── financial_regulation.rego
│   ├── financial_regulation_test.rego
│   ├── security_integrity.rego
│   ├── security_integrity_test.rego
│   ├── trade_compliance.rego
│   └── trade_compliance_test.rego
└── fixtures/
    ├── baseline-snapshot.json
    └── sample-events.json
```

## 2. Policy Authoring Conventions

Each policy package follows standard conventions:
- Package names begin with `compliance.events.<domain>`
- Decisions define `default compliant := false` and `compliant if { count(violations) == 0 }`
- Rules populate `violations contains violation if { ... }`
- Violations include: `rule_id`, `title`, `framework`, `severity`, `event_id`, and `message`.

## 3. Running Continuous Compliance & Drift Detection

Using the `@audit-ledger/compliance-policy` CLI tool:

```bash
cd tools/compliance-policy

# Check events against policies
npm run check

# Check for drift against baseline
npm run drift

# Generate comprehensive audit report
npm run report
```

## 4. Remediation Workflow

When a violation or drift finding occurs:
1. **Critical Violations (CRITICAL)**: Triggers immediate CI failure and notification to compliance officer.
2. **High Violations (HIGH)**: Flagged in audit reports, requires compliance sign-off or remediation within 7 days.
3. **Drift Findings**: Highlighted in the drift analysis section of the audit report.
