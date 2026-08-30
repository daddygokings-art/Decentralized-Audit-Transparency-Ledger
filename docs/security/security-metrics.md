# Security Metrics and Board Reporting

The weekly vulnerability workflow produces `security-kpis.json` from the normalized findings ledger. The report is an evidence artifact, not a substitute for incident records or management sign-off.

| KPI | Definition | Target / action |
| --- | --- | --- |
| MTTD | Average time from a finding's creation to detection | Track trend; investigate increases |
| MTTR | Average days from `first_seen` to `resolved_at` | Meet the SLA in the vulnerability-management policy |
| Vulnerability aging | Age of the oldest unresolved finding | Escalate critical/high findings |
| Patch compliance | Assets patched within the applicable SLA | 95% or higher |
| Phishing click rate | Simulated phishing clicks divided by delivered tests | Trend downward; remediate repeat failures |
| Security training completion | Staff with current training divided by required staff | 100% |

Teams may provide optional `training.json` and `phishing.json` inputs to the generator. Missing operational inputs remain `null` so unavailable data is not misrepresented as zero.