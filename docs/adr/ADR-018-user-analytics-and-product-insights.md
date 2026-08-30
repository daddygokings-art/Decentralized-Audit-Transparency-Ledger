# ADR-018: Privacy-Compliant User Analytics and Product Insights

## Status
Accepted

## Context
Understanding user behavior, conversion funnels (e.g., from connecting a wallet to verifying an audit proof), feature adoption depth, and cohort retention is essential for optimizing product experience. However, this must strictly comply with global privacy regulations (GDPR, CCPA, ePrivacy Directive) without compromising user privacy.

## Decision
We implemented `@audit-ledger/user-analytics`, a privacy-first product analytics engine supporting:
1. **Consent Management & Header Compliance**: Explicit opt-in consent controls and automatic honoring of `DNT: 1` and `Sec-GPC: 1` headers.
2. **Salted Pseudonymization**: HMAC-SHA256 hashing of wallet addresses and IP masking to prevent PII exposure.
3. **Automated Right-to-be-Forgotten Erasure**: Instant purge of all analytics records and session histories on user request.
4. **Multi-Stage Funnel Analyzer**: Evaluates step-by-step conversion, time-to-convert, and drop-off bottlenecks.
5. **Feature Adoption & Stickiness Engine**: Calculates unique users, power user distribution, and DAU/MAU stickiness per feature.
6. **Cohort Retention Heatmaps**: Evaluates recurring engagement over daily, weekly, and monthly intervals.

## Consequences
- Product teams have detailed visibility into user flows, adoption friction, and retention without storing any personal data.
- Full compliance with international data protection standards.
