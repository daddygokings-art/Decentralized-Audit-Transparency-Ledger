# @audit-ledger/compliance-policy

Contract Event Compliance Automation, OPA/Rego Policy-as-Code Evaluation, Drift Detection, and Regulatory Audit Reporting for the Decentralized Audit Transparency Ledger.

## Overview

This tool provides a policy-as-code automation framework that validates smart contract events emitted across Soroban ledger operations against major regulatory compliance frameworks:
- **SOC 2 Type II** (CC6.1, CC6.3, CC6.5, CC6.6)
- **ISO/IEC 27001:2022** & **ISO 37001** (Anti-Bribery, Cryptographic controls)
- **EU GDPR** (Article 17 Right to Erasure, Article 5 Storage Limitation)
- **EU MiCA** (Regulation 2023/1114 - 100% Stablecoin Reserve Backing)
- **FATF Recommendations & FinCEN BSA** (Travel Rule, Large CTR Reporting)
- **WTO & US Export Controls** (ITAR/EAR Dual-Use, Denied Parties, Embargoes)

## Architecture

- **OPA / Rego Policies** (`policies/compliance/`): Declarative rule suites covering anti-corruption, export controls, trade compliance, data retention, financial regulations, and cryptographic integrity.
- **Continuous Compliance Engine** (`src/evaluator.ts`): Stream/batch contract event evaluation, scoring, and severity classification.
- **Drift Detection Engine** (`src/drift.ts`): Automated detection of governance quorum changes, unregistered event schemas, and compliance score degradation against an approved baseline.
- **Audit Reporting Engine** (`src/reporter.ts`): Automated generation of JSON and Markdown audit reports for external and internal auditors.

## CLI Usage

```bash
# Evaluate contract events against Rego compliance policies
npx ts-node src/cli.ts check --events policies/compliance/fixtures/sample-events.json

# Detect compliance drift against baseline snapshot
npx ts-node src/cli.ts drift --events policies/compliance/fixtures/sample-events.json --baseline policies/compliance/fixtures/baseline-snapshot.json

# Generate compliance audit report
npx ts-node src/cli.ts report --events policies/compliance/fixtures/sample-events.json --output docs/compliance/reports

# Run Rego unit tests
npx ts-node src/cli.ts test-policies
```
