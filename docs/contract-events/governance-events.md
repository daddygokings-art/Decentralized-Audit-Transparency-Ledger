# Governance & Identity Events

This document details all decentralized governance, token gating, submitter DIDs, and reputation system events.

---

## 1. DAO Governance & Dispute Resolution (`src/dao_governance.rs`)

### `proposal_created`
- **Topic**: `Symbol("dao_governance")`, `Symbol("proposal_created")`
- **Payload Schema**:
  ```json
  {
    "proposal_id": 42,
    "proposer": "GAK4K6K4Z67A5M7X5SLLM",
    "title": "Upgrade Compliance Threshold",
    "voting_start": 1756483200,
    "voting_end": 1757088000
  }
  ```

---

## 2. Token Gating & Role-Based Access (`src/token_gating.rs`)

### `access_granted`
- **Topic**: `Symbol("token_gating")`, `Symbol("access_granted")`
- **Payload Schema**:
  ```json
  {
    "account": "GAK4K6K4Z67A5M7X5SLLM",
    "gated_resource": "ADMIN_COMPLIANCE_VAULT",
    "token_balance": 5000,
    "expires_at": 1759088000
  }
  ```

---

## 3. Submitter DIDs & Reputation (`src/submitter_dids.rs`)

### `did_registered`
- **Topic**: `Symbol("submitter_dids")`, `Symbol("did_registered")`
- **Payload Schema**:
  ```json
  {
    "did": "did:stellar:GAK4K6K4Z67A5M7X5SLLM",
    "reputation_score": 100,
    "verified_credentials_count": 3
  }
  ```
