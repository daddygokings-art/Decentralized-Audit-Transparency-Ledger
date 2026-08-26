# GDPR

AuditLedger's dedicated GDPR-alignment documentation already lives at
[`../security/privacy-by-design.md`](../security/privacy-by-design.md)
(DPIA template, data minimization, on-chain-immutability-vs-erasure
tradeoffs for the `AuditLedger` contract). This page adds only the
infrastructure-security-control mapping that page doesn't cover.

| Article | Controls in this repo |
|---|---|
| Art. 25 — Data protection by design and by default | CTRL-PRIV-01 (DPIA process, see privacy-by-design.md) |
| Art. 32 — Security of processing | CTRL-SEC-01, CTRL-SEC-02, CTRL-SEC-03, CTRL-ACC-01 |
| Art. 33 / 34 — Breach notification | CTRL-IR-01 |
| Art. 35 — DPIA | CTRL-PRIV-01 |

## On-chain data and the right to erasure

The append-only, tamper-evident design that makes `AuditLedger` useful
as an audit trail is in direct tension with Art. 17 (right to erasure)
for any personal data that ends up in event metadata. This is a known,
already-documented tradeoff — see privacy-by-design.md's guidance on
keeping personal data *off-chain* with only references/hashes on-chain
— not something the infra controls in this directory change. Nothing
here should be read as making on-chain personal data erasable.
