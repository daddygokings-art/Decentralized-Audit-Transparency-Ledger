# Contract Event Security Incident Response

This playbook covers incidents affecting the AuditLedger contract, its bridge, and the services that publish or consume contract events. The incident commander records every decision and timestamp in the incident ticket. Preserve evidence before making destructive changes.

## Severity and first response

| Severity | Example | Acknowledge | Page |
| --- | --- | --- | --- |
| SEV-1 | Active exploit, bridge compromise, or confirmed key exposure | 15 minutes | Security, on-call, owner |
| SEV-2 | Suspected compromise, data breach, or supply-chain anomaly | 30 minutes | Security and on-call |
| SEV-3 | Control failure without evidence of compromise | 4 hours | Service owner |

For every incident:

1. Name an incident commander and communications lead.
2. Open an incident ticket and record UTC timestamps, affected contract IDs, deployment versions, and responders.
3. Preserve API, relayer, Kubernetes audit, admission-controller, Falco/Tetragon, and cloud identity logs.
4. Create a read-only evidence copy with a SHA-256 manifest; do not overwrite the originals.
5. Notify affected stakeholders according to the disclosure and regulatory requirements for the deployment region.

## Data breach

**Contain:** revoke exposed API keys and tokens, disable affected accounts, isolate the API or exporter namespace, and block suspicious egress at the Cilium policy layer.

**Investigate:** identify data accessed, principal, first and last observed access, and whether on-chain event metadata contains personal or confidential data. Compare access logs with the immutable contract event history.

**Recover:** rotate secrets using the Vault procedures, deploy from a verified image digest, restore only from a verified backup, and require security approval before reopening traffic.

**Close:** document notification decisions, affected subjects, root cause, control gaps, and a 30-day remediation owner.

## Contract exploit

**Contain:** pause integrations and write paths where operationally possible, preserve the affected contract ID and ledger range, and move the owner key to the incident response hardware wallet. Do not attempt an ad hoc contract upgrade.

**Investigate:** capture transaction hashes, event topics, authorization entries, WASM checksum, and deployer identity. Reproduce the exploit only on an isolated testnet fork or test environment.

**Recover:** have governance approve a remediation contract, verify its WASM checksum and initialization parameters, and replay only validated events from a signed snapshot. Publish the old and replacement contract IDs together.

**Close:** obtain an independent review, add a regression test, and monitor the replacement contract for at least one full operating cycle.

## Bridge compromise

**Contain:** stop the relayer, revoke its signing credentials, block bridge egress, and freeze downstream settlement or minting controls. Preserve the relayer cache and queue before restart.

**Investigate:** correlate source-chain events, destination transactions, relayer logs, nonce usage, and signer activity. Identify replayed, missing, or forged messages.

**Recover:** rotate bridge keys, deploy a reviewed relayer image by immutable digest, reconcile both chains from signed event snapshots, and resume traffic in a capped canary mode.

**Close:** reconcile balances and event counts, publish affected transaction ranges, and obtain security sign-off for normal throughput.

## Insider threat

**Contain:** suspend the suspected principal, revoke sessions and credentials, preserve identity-provider and Kubernetes audit logs, and require two-person approval for owner or deployment actions.

**Investigate:** review privileged actions, repository changes, admission decisions, Vault access, and contract governance events against the approved change ticket.

**Recover:** rotate credentials and owner keys if exposure is possible, remove unapproved workloads, and redeploy from a reviewed commit with verified signatures.

**Close:** complete access review, document chain of custody, and add preventive separation-of-duties controls.

## Supply-chain attack

**Contain:** quarantine the image digest, stop affected workloads, deny unsigned images through Kyverno, and preserve the image manifest, SBOM, provenance, and Rekor entry.

**Investigate:** compare the digest with the signed GitHub Actions workflow identity, source commit, build logs, dependency scan, and deployment audit trail.

**Recover:** rebuild from a known-good commit, sign with keyless cosign, verify the Rekor entry in admission, and roll out by digest after a canary.

**Close:** rotate compromised build credentials, invalidate the affected artifact, publish an advisory when required, and add a regression check to CI.

## Tabletop exercises

Run one scenario every quarter, rotating the incident commander. Use a synthetic contract and test credentials only. The facilitator injects a timeline, evidence, and one communications decision without pre-announcing the expected response.

| Exercise | Inject | Required outcome |
| --- | --- | --- |
| Data breach | API token accesses event metadata outside its tenant | Containment, scope, notification decision |
| Contract exploit | Unexpected privileged contract event | Reproducible evidence and migration decision |
| Bridge compromise | Relayer signs an unknown destination transaction | Key revocation and chain reconciliation |
| Insider threat | Unapproved owner action appears in audit logs | Access suspension and two-person review |
| Supply-chain attack | A deployment references an unsigned digest | Admission denial and clean rebuild |

## Metrics and evidence

Track these for each incident and tabletop:

- Mean time to detect, acknowledge, contain, eradicate, and recover.
- Percentage of evidence artifacts with a verified SHA-256 manifest.
- Percentage of production images admitted only after signature verification.
- Number of privileged actions with two-person approval.
- Bridge reconciliation variance and time to reach zero variance.
- Tabletop action items closed by their due date.

The incident commander closes the incident only when containment is verified, recovery is monitored, notifications are complete, and corrective actions have owners and due dates.