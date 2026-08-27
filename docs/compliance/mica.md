# MiCA (EU Markets in Crypto-Assets Regulation)

MiCA applies to crypto-asset service providers (CASPs) and issuers, not
directly to open-source infrastructure like this repository — the
mapping below covers the ICT/operational-security requirements
(Title V, largely cross-referencing DORA-style ICT risk management) that
would apply if AuditLedger is operated *as part of* a regulated CASP's
infrastructure.

| Requirement area | Controls in this repo |
|---|---|
| Art. 66 — Record-keeping | CTRL-AUD-01 (immutable, publicly verifiable on-chain event log) |
| Art. 68 — ICT and security risk management | CTRL-SEC-01, CTRL-SEC-02, CTRL-SEC-03, CTRL-SEC-04, CTRL-ACC-01, CTRL-IR-01 |

## What this mapping does not cover

MiCA's substantive requirements — authorization as a CASP, whitepaper
disclosure, market-abuse prevention, custody rules, complaint handling —
are business/regulatory obligations on the *operator*, not something
infrastructure tooling in this repo satisfies. This page only maps the
technical ICT-security controls a MiCA-regulated operator would point to
as part of their Art. 68 risk-management framework; it is not a MiCA
compliance program on its own, and a CASP deploying AuditLedger still
needs separate legal/regulatory review.
