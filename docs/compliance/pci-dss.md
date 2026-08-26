# PCI DSS v4.0

AuditLedger does not process, store, or transmit cardholder data itself
— it logs financial *events* (references/metadata), not card numbers.
This mapping covers requirements that still apply to the surrounding
infrastructure if AuditLedger is deployed adjacent to a cardholder-data
environment (CDE), e.g. as an audit-trail component whose network
segment touches the CDE.

| Requirement | Controls in this repo |
|---|---|
| Req 3.6 — Cryptographic key management | CTRL-SEC-01 (Vault-managed rotation) |
| Req 4.2 — Strong cryptography in transit | CTRL-SEC-02 (TLS via cert-manager) |
| Req 6.3 / 11.3 — Vulnerability management | CTRL-SEC-03 |
| Req 6.3.2 — Inventory & review of custom/third-party software | CTRL-SEC-04 |
| Req 7.2 — Access control by role | CTRL-ACC-01 |
| Req 8.6 — Managed accounts/credentials | CTRL-SEC-01 |
| Req 10.2 — Audit logging | CTRL-AUD-01 |
| Req 12.10 — Incident response | CTRL-IR-01 |

## Scoping note

If AuditLedger is deployed such that no component stores/processes
cardholder data and it sits outside the CDE with proper network
segmentation, most PCI DSS requirements are out of scope entirely — this
table exists for the case where it doesn't (e.g. deployed in the same
Kubernetes cluster as CDE workloads). Confirm actual scope with a QSA
before treating this mapping as sufficient; it documents the *technical
controls that would help*, not a scope determination.
