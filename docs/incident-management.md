# Contract Event Incident Management & On-Call System

This document outlines the incident management framework, on-call rotation policies, escalation workflows, and postmortem processes for the Decentralized Audit Transparency Ledger.

---

## 1. Overview

The contract event incident management system combines on-chain circuit breakers and immutable timeline anchoring with off-chain notification and escalation engines (PagerDuty & Opsgenie).

```
┌─────────────────────────┐
│ Stellar Soroban Ledger  │
│  & Bridge Relayers      │
└────────────┬────────────┘
             │ Metrics & Logs
             ▼
┌─────────────────────────┐      Webhook / API       ┌────────────────────────┐
│ Prometheus Alertmanager ├─────────────────────────►│ Incident Management Svc│
└─────────────────────────┘                          └──┬───────────────────┬─┘
                                                        │                   │
                                          PagerDuty API │      Opsgenie API │
                                                        ▼                   ▼
                                                ┌──────────────┐    ┌──────────────┐
                                                │  PagerDuty   │    │   Opsgenie   │
                                                └──────┬───────┘    └──────┬───────┘
                                                       │                   │
                                                       ▼                   ▼
                                                ┌──────────────────────────────────┐
                                                │   On-Call Engineering Team       │
                                                └──────────────────────────────────┘
```

---

## 2. Severity Classification Matrix

| Severity | Definition | Target MTTA | Target MTTR | Escalation Policy |
|---|---|---|---|---|
| **SEV-1 (Critical)** | Core ledger down, active exploit, consensus mismatch, financial risk | < 5 mins | < 30 mins | Immediate page to Primary + Secondary + VP |
| **SEV-2 (High)** | Bridge relayer stalled, elevated event drop rate, major latency degradation | < 15 mins | < 2 hours | Page Primary, escalate to Secondary in 10 mins |
| **SEV-3 (Medium)** | Gas cost spike, minor rate limit pressure, non-critical service degradation | < 1 hour | < 8 hours | Slack / Email alert to on-call |
| **SEV-4 (Low)** | Operational warning, cosmetic telemetry glitch | < 4 hours | < 24 hours | Ticket creation |
| **SEV-5 (Info)** | Scheduled drill, maintenance window | N/A | N/A | Logged only |

---

## 3. On-Call Rotations & Escalation Policies

### Rotations
- **Follow-the-Sun**: 3 regional 8-hour handover shifts (US-East, EMEA, APAC).
- **Weekly Core Rotations**: Primary on-call holds active pager; Secondary acts as hot backup.
- **Overrides**: Temporary shift coverage managed via `/api/v1/on-call/override`.

### Multi-Tier Escalation
1. **Tier 1 (0 min)**: Primary on-call responder receives SMS, Push, and automated voice page.
2. **Tier 2 (+10 min)**: If unacknowledged, Secondary on-call responder is paged.
3. **Tier 3 (+20 min)**: Engineering Leads & System Architects notified.

---

## 4. Soroban Smart Contract Integration

The on-chain module (`src/incident_management.rs`) provides:
- Content-addressed incident registration (`trigger_incident`).
- Cryptographic timeline entry auditing (`add_timeline_entry`).
- On-chain circuit-breaker actuation (`set_circuit_breaker`).
- Blameless postmortem hash anchoring (`record_postmortem`).

---

## 5. Postmortem Workflow

1. Following incident resolution, the Incident Commander drafts a postmortem using `docs/postmortems/TEMPLATE.md`.
2. A blameless retrospective review is scheduled within 48 hours.
3. Remediation action items are committed to Jira/GitHub with designated owners.
4. The signed postmortem root cause hash is anchored to the Soroban smart contract.
