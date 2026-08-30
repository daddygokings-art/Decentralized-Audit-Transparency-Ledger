# Product Analytics and User Insights Guide

This document describes the privacy-compliant user behavior tracking, funnel analysis, feature adoption metrics, and cohort retention engine implemented in `@audit-ledger/user-analytics`.

## Privacy-First Architecture (GDPR / CCPA / ePrivacy)

1. **Consent Management**:
   - Explicit opt-in tracking for `analytics`, `performance`, and `marketing` categories.
   - Automatically honors browser `DNT: 1` (Do Not Track) and `Sec-GPC: 1` (Global Privacy Control) headers.
2. **Cryptographic Salted Pseudonymization**:
   - Wallet addresses, IP addresses, and user identifiers are hashed using `HMAC-SHA256` with rotating server salt before storage.
   - Raw PII is never stored in analytics storage.
3. **Right to be Forgotten (GDPR Article 17)**:
   - Automated erasure engine permanently purges all event records, session logs, and consent preferences for a requested pseudonymized ID.

---

## Funnel Analysis

Multi-step conversion funnels allow identifying drop-off bottlenecks in critical user workflows:
- **Audit Submission Funnel**:
  1. `connect_wallet` (Step 1)
  2. `view_audit_form` (Step 2)
  3. `submit_event` (Step 3)
  4. `verify_proof` (Step 4)
  5. `export_report` (Step 5)

The analyzer calculates:
- Step-by-step conversion rates and drop-off percentages.
- Average completion latency between sequential steps.
- Identification of biggest friction points (`biggestDropoffStep`).

---

## Feature Adoption & Stickiness

- **Adoption Rate**: Percentage of overall active users interacting with a specific capability (e.g. `token_gating`, `tax_engine`, `digital_passport`, `cross_chain_bridge`).
- **Feature Stickiness**: Feature-level DAU / MAU ratio.
- **Power User Identification**: Accounts with $\ge 10$ interactions in a 30-day window.

---

## Cohort Retention Analysis

- Groups users into acquisition cohorts based on their first activity timestamp.
- Calculates recurring retention rates across daily (Day 1, 7, 14, 30), weekly, or monthly periods.
- Generates cohort retention matrices and estimates user churn rates.

---

## API Endpoints

- `POST /api/v1/analytics/consent` - Update user consent preferences.
- `POST /api/v1/analytics/track` - Track user interaction event.
- `POST /api/v1/analytics/erasure` - Execute Right to be Forgotten data deletion.
- `GET /api/v1/analytics/funnels` - Retrieve conversion funnel analysis.
- `GET /api/v1/analytics/features` - Retrieve feature adoption statistics.
- `GET /api/v1/analytics/cohorts` - Retrieve cohort retention matrix.
