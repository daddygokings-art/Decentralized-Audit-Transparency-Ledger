# Compliance Automation

This directory covers infrastructure/organizational compliance
automation — evidence collection, control mapping, and continuous
compliance monitoring for SOC 2, ISO 27001, PCI DSS, GDPR, and MiCA.

It is distinct from the on-chain compliance *smart-contract modules*
(`src/anti_corruption.rs`, `src/export_controls.rs`,
`src/trade_compliance.rs`, etc., documented in `docs/anti_corruption_compliance.md`
and siblings) — those implement regulatory business logic on-chain for
AuditLedger's users; this directory covers compliance *of the AuditLedger
system itself* as an operator would need to demonstrate it to an
auditor.

## How it fits together

```
docs/compliance/control-matrix.yaml   <- single source of truth: one control,
                                          mapped to every framework it satisfies
        |
        +-- scripts/compliance/collect-evidence.sh   (scheduled, weekly)
        |       writes evidence/<control-id>/<date>.json
        |
        +-- scripts/compliance/generate-compliance-report.py
                writes compliance-reports/<framework>.md
                (per-framework coverage + evidence-staleness report)
```

`.github/workflows/compliance-evidence.yml` runs both steps weekly and
uploads the results as a build artifact — see
[evidence-collection.md](evidence-collection.md) for the retention and
access model, and [auditor-access.md](auditor-access.md) for how an
external auditor gets a read-only view of this without repo write
access.

## Framework-specific notes

- [soc2.md](soc2.md)
- [iso27001.md](iso27001.md)
- [pci-dss.md](pci-dss.md)
- [gdpr.md](gdpr.md)
- [mica.md](mica.md)

## Continuous vs. point-in-time compliance

Because `collect-evidence.sh` runs weekly rather than being generated
once before an audit, `generate-compliance-report.py` can flag evidence
older than 8 days as stale — the automation is meant to catch a broken
collector (e.g., a scanner that silently stopped running) before an
auditor does, not just to produce a report on demand.
