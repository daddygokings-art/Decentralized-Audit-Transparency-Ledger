# Data Subject Rights Automation

The REST API provides a request portal and fulfillment ledger for access, rectification, erasure, portability, restriction, and objection requests.

## Lifecycle

1. `POST /v1/privacy/requests` accepts a request only after the caller supplies a verification token. The API returns a request ID and records `request_received`.
2. `GET /v1/privacy/requests/:id` returns status and the append-only operational audit trail.
3. Operators move a request through `in_progress`, `fulfilled`, or `rejected` with `PATCH /v1/privacy/requests/:id`.

Erasure is implemented as crypto-shredding by the Soroban retention module: personal metadata is redacted while the original digest and decision remain auditable. Legal holds and compliance exceptions prevent fulfillment until released or expired. All requests must be identity-verified, minimised, access-controlled, and fulfilled within the applicable statutory deadline.
