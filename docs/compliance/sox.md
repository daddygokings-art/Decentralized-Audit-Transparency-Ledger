# Sarbanes-Oxley Act (SOX) Compliance

## Overview

AuditLedger provides automated compliance controls and continuous monitoring for Sarbanes-Oxley (SOX) Section 302 and Section 404 requirements.

## Key Controls

| Control ID | Description | Automated Evidence | Verification Frequency |
|------------|-------------|--------------------|------------------------|
| `SOX-404-01` | Access Control & Segregation of Duties | Multisig approvals, role separation logs | Continuous / Weekly |
| `SOX-404-02` | Change Management & Audit Trail Integrity | Immutable ledger logs for configuration updates | Continuous |
| `SOX-302-01` | Financial Reporting Accuracy & Ledger Attestation | Hash-chained event digests and cryptographic seals | Monthly |

## Policy Enforcement

- **Segregation of Duties**: Transactions above sensitive thresholds require distinct initiators and approvers.
- **Tamper Evidence**: All administrative ledger changes emit deterministic cryptographic event logs preventing backdated alterations.
