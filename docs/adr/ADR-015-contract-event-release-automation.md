# ADR-015: Contract Event Release Automation with Semantic Versioning

## Status
Accepted

## Context
As the `AuditLedger` contract ecosystem scales across enterprise audit, regulatory reporting, supply chain, and CBDC integrations, release operations must guarantee strict semantic versioning, reproducible asset verification, automated changelog generation, release candidate validation on testnet, and instantaneous rollback mechanisms.

Manual tagging and manual asset uploads create risk of schema mismatches, missing SBOMs, unverified bytecode hashes, and delayed incident recovery.

## Decision
We introduce an end-to-end Contract Event Release Automation architecture comprising:

1. **Semantic Versioning & Compatibility Enforcement**:
   - Structured SemVer parsing (`MAJOR.MINOR.PATCH[-PRERELEASE]`).
   - Breaking changes (major bump) require explicit migration approval.
   - Minor bumps add backward-compatible features and event topics.
   - Patch bumps fix bugs with zero schema disruption.

2. **On-Chain Release Registry (`src/event_release.rs`)**:
   - `EventReleaseRecord` records version numbers, status (`Draft`, `ReleaseCandidate`, `Published`, `Deprecated`, `RolledBack`), changelog hashes, and asset integrity digests.
   - Event emissions (`release_cand_created`, `release_promoted`, `release_published`, `release_rolled_back`) anchor state transitions in the immutable audit ledger.

3. **Release Candidate (RC) Staging Workflow**:
   - Dedicated staging workflow for tags matching `v*-rc.*`.
   - Smoke tests, event schema diff checks, and staging testnet validation prior to production release promotion.

4. **Cryptographic Asset Publishing**:
   - Automated packaging of optimized WASM binaries, CycloneDX 1.5 SBOMs, JSON event schemas, and SHA-256 / SHA-512 checksum manifests.

5. **Automated Rollback Engine**:
   - Atomic rollback capabilities via script (`scripts/release/rollback-release.sh`) and GitHub Action (`.github/workflows/release-rollback.yml`).
   - Deprecates faulty versions, repoints active schema resolvers to previous stable baselines, and publishes on-chain rollback audit events.

## Consequences

### Positive
- Zero manual toil in packaging releases, generating release notes, or computing digests.
- Strict backward compatibility checks prevent breaking downstream event indexers.
- Clear RC verification lifecycle before hitting production.
- Automated rollback minimizes Mean Time to Recovery (MTTR) during critical incidents.

### Trade-offs
- Releases require adherence to conventional commit guidelines for automated version determination.
