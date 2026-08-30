# ADR-015: Contract Event Compliance Automation with Policy-as-Code

| Field | Value |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-08-29 |
| **Deciders** | Security, Compliance, and Architecture Teams |

---

## Context

AuditLedger emits dozens of on-chain contract events across critical business domains (anti-corruption, export controls, trade compliance, stablecoin reserves, and data retention). Historically, verifying that ledger operations complied with external regulatory mandates (e.g. EU MiCA, GDPR, FATF Travel Rule, SOC 2, ISO 27001, US EAR/ITAR) required asynchronous, manual audits or fragmented ad-hoc scripts.

To guarantee continuous regulatory adherence, we needed a unified **Policy-as-Code** framework capable of:
1. Expressing regulatory requirements declaratively.
2. Evaluating live and archived contract events continuously.
3. Detecting compliance drift and policy degradation over time.
4. Generating formal audit reports suitable for regulatory examinations.

---

## Decision

We adopt **Open Policy Agent (OPA) / Rego** as the standard policy-as-code engine for contract event compliance automation:

1. **Declarative Rego Policy Suites (`policies/compliance/`)**:
   - `anti_corruption.rego`: Enforces FCPA, UK Bribery Act, and ISO 37001 requirements (officer assignment, whistleblower identity protection, gift limits).
   - `export_controls.rego`: Enforces EAR/ITAR and OFAC sanctions (denied party checks, dual-use licenses, embargoed jurisdictions).
   - `trade_compliance.rego`: Enforces WTO/WCO standards (Certificate of Origin verification, HS code syntax, customs valuation benchmarks).
   - `data_retention.rego`: Enforces GDPR Art. 17 right to erasure SLAs and SOC 2 CC6.5 legal hold non-deletion.
   - `financial_regulation.rego`: Enforces MiCA reserve ratios (>=100%), FATF Travel Rule KYC identification, and FinCEN CTR threshold reporting.
   - `security_integrity.rego`: Enforces multi-sig governance quorum and cryptographic hash formatting.

2. **Automated Continuous Evaluation & Drift Detection Tool (`@audit-ledger/compliance-policy`)**:
   - Evaluates event streams against Rego policies via OPA CLI or high-performance embedded evaluator.
   - Computes framework-level compliance scores (SOC 2, ISO 27001, GDPR, MiCA, FATF).
   - Compares event sequences against approved baseline snapshots to detect governance drift, schema drift, and compliance degradation.
   - Automatically generates structured JSON and Markdown audit reports for auditors.

3. **CI/CD Integration (`.github/workflows/contract-event-compliance.yml`)**:
   - Runs Rego unit tests (`opa test`) on every PR touching contracts or policies.
   - Executes daily scheduled compliance evaluations and publishes dated audit artifacts.

---

## Consequences

### Positive
- **Declarative & Version-Controlled**: Regulatory policies are code, peer-reviewed in PRs, and tested with unit test suites.
- **Continuous Verification**: Eliminates point-in-time compliance gaps through continuous event stream evaluations.
- **Multi-Framework Mapping**: Centralized mapping (`regulatory-frameworks.yaml`) links single policy rules to multiple regulatory standards.
- **Automated Drift Detection**: Early warnings when quorum rules or compliance scores degrade below baseline thresholds.

### Negative
- Requires maintaining Rego policies as regulatory standards evolve.
- Edge cases in contract event payloads must have schema validation prior to policy evaluation.
