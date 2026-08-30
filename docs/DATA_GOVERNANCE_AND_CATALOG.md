# Contract Event Data Governance and Catalog

## Overview

This module provides an enterprise-grade data governance framework for contract events on the Decentralized Audit Transparency Ledger.

## Core Capabilities

1. **Searchable Data Catalog**:
   - Metadata discovery, dataset definitions, schema field cataloging, and classifications (`Public`, `Internal`, `Confidential`, `Restricted`).
   - Compliance tagging for regulatory frameworks (`GDPR`, `CCPA`, `HIPAA`, `ESG`, `SOC2`, `PCI_DSS`).

2. **End-to-End Lineage Tracking**:
   - Cryptographically anchored lineage DAG tracking event provenance from Stellar Soroban contract emit through Bridge Relayer, Data Lake, ClickHouse, REST API, and Grafana UI.
   - Upstream and downstream change impact analysis.

3. **Automated Quality Scorecards**:
   - Quality dimension profiling: Completeness, Validity, Accuracy, Uniqueness, and Timeliness.
   - SLA assertion evaluation and breach detection.

4. **Access Policies & Dynamic PII Masking**:
   - Role-based and attribute-based access control (RBAC/ABAC).
   - Dynamic masking engines supporting redaction (`[REDACTED_PII]`), SHA-256 pseudonymization hashing, and tokenization.

5. **Data Stewardship Workflows**:
   - Auditable request lifecycles for access elevation, schema changes, and retention overrides.
   - Reviewer audit trail with timestamped steward approvals.

## API Endpoints

- `GET /api/v1/governance/catalog`: Search catalog assets with filters.
- `GET /api/v1/governance/lineage`: Fetch DAG lineage graph or upstream origin.
- `GET /api/v1/governance/quality`: Retrieve data quality scorecards.
- `POST /api/v1/governance/policies/enforce`: Evaluate access permissions and apply role-based masking.
- `POST /api/v1/governance/stewardship/requests`: Create a new stewardship request.
- `POST /api/v1/governance/stewardship/review`: Review and approve/reject a stewardship request.
- `GET /api/v1/governance/health`: Governance subsystem health check.
