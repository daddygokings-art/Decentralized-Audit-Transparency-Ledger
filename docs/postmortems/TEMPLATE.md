# Blameless Postmortem Template

**Incident ID**: `INC-XXXX`  
**Title**: Short descriptive title of the incident  
**Date**: YYYY-MM-DD  
**Authors**: @investigator  
**Incident Commander**: @commander  
**Severity**: `SEV-1` | `SEV-2` | `SEV-3`  

---

## 1. Executive Summary
Brief summary of the incident, user impact, duration, and final resolution.

## 2. Impact Analysis
- **Service Degradation / Outage**:
- **Contract Events Delayed / Dropped**:
- **Transactions Affected**:
- **Estimated Financial / Gas Cost Impact**:

## 3. Timeline of Events
All timestamps in UTC.

| Time (UTC) | Source / Actor | Action / Event |
|---|---|---|
| 00:00 | Alertmanager | Automated alert triggered (`BridgeRelayerHalted`) |
| 00:03 | PagerDuty | Primary on-call paged |
| 00:05 | On-Call | Incident acknowledged, triage started |
| 00:15 | On-Call | Circuit breaker activated on-chain |
| 00:40 | Team | Fix deployed to relayer |
| 00:45 | On-Call | Circuit breaker reset, events resumed |
| 01:00 | Team | Incident marked as resolved |

## 4. Root Cause Analysis (5 Whys)
1. **Why did the failure occur?** ...
2. **Why was that condition present?** ...
3. **Why did earlier checks not catch it?** ...
4. **Why was the fallback not triggered automatically?** ...
5. **Why was the threshold configured as such?** ...

## 5. Lessons Learned
### What went well:
- Automated monitoring triggered within 60s.
- Clear communication in incident war room.

### What went wrong:
- Escalation delay to secondary team.
- Missing runbook step for queue draining.

### Where we got lucky:
- Ledger state was completely preserved without data loss.

## 6. Action Items & Remediation
| Action Item | Type | Owner | Target Date | Tracking Ticket |
|---|---|---|---|---|
| Add automated failover runbook | Preventative | @dev | 2026-09-15 | JIRA-1001 |
| Adjust Prometheus alert sensitivity | Monitoring | @sre | 2026-09-05 | JIRA-1002 |
