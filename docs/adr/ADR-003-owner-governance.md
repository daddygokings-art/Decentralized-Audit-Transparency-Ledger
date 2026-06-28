# ADR-003: Owner-Only Governance Model

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2024-01-01 |
| **Deciders** | Core team |

---

## Context

Governance functions — adjusting caps, pausing the contract, transferring ownership, upgrading the WASM — must be restricted to trusted parties. The question is how many parties constitute a quorum and how that is enforced on-chain.

A single-owner model is the simplest to reason about and audit. A multi-sig model provides stronger security at the cost of operational complexity.

---

## Decision

The contract launches with a **single-owner governance model**:

- The deployer sets the initial owner address during `initialize`.
- All governance functions (`set_global_max_logs`, `set_event_max_logs`, `remove_event_cap`, `transfer_ownership`, `pause`, `unpause`, `upgrade_contract`, etc.) check `caller == owner` and call `caller.require_auth()`.
- Ownership can be transferred atomically via `transfer_ownership`; the old owner loses all privileges immediately.

Multi-owner support is available as an **opt-in extension** via `add_owner`, `remove_owner`, `set_required_signatures`, `submit_proposal`, `approve_proposal`, and `execute_proposal`. This allows teams to migrate to multi-sig governance without redeploying the contract.

---

## Consequences

**Positive:**
- Simple, auditable governance: every privileged action has a single accountable party.
- Zero coordination overhead for single-operator deployments.
- `transfer_ownership` allows ownership to move to a multi-sig wallet at any time.

**Negative:**
- A single compromised owner key gives full control of the contract.
- No timelock on governance actions; an owner can change caps or pause the contract instantly.

**Mitigations:**
- Owners should use a hardware wallet or a multi-sig address from day one.
- The multi-sig proposal system (ADR-005 scope) can be activated to add an approval buffer.
- Future work: add a timelock delay to critical governance actions.

---

## Alternatives Considered

| Alternative | Reason Rejected |
|-------------|----------------|
| DAO / token-weighted voting | Disproportionate complexity for most deployments; can be layered on top later. |
| Immutable (no governance) | Prevents necessary maintenance (cap adjustments, upgrades, emergency pause). |
| Hard-coded multi-sig at deploy | Forces all operators into multi-sig; adds friction for solo developers. |
| Role-based access control (RBAC) | Adds significant complexity; the current use case does not need more than two roles (owner, submitter). |
