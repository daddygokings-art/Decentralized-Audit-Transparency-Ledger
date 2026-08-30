# Contract Event Release Automation & Semantic Versioning Guide

This guide describes the release automation system for the `AuditLedger` contract and associated SDKs.

---

## 🏗️ Architecture Overview

The release automation system integrates:
1. **Semantic Versioning CLI (`scripts/release/semver-manager.sh`)**
2. **Release Notes & Changelog Generator (`scripts/release/generate-release-notes.sh`)**
3. **Asset Packaging & Attestation (`scripts/release/publish-assets.sh`)**
4. **Release Candidate (RC) Staging Engine (`scripts/release/rc-workflow.sh`)**
5. **Emergency Rollback Playbook (`scripts/release/rollback-release.sh`)**
6. **On-Chain Release Registry (`src/event_release.rs`)**
7. **CI/CD GitHub Actions Pipelines**

---

## 🚦 Semantic Versioning Rules

We adhere strictly to [SemVer 2.0.0](https://semver.org/):

- `MAJOR`: Incompatible schema changes, removed event fields, altered topic encodings.
- `MINOR`: New event types, optional metadata fields, backward-compatible enhancements.
- `PATCH`: Internal bug fixes, documentation, gas optimizations with identical ABI.
- `RC`: Pre-release candidates in the format `vX.Y.Z-rc.N`.

---

## 🚀 Release Lifecycle

### 1. Creating a Release Candidate (RC)

To prepare a release candidate for testnet verification:

```bash
./scripts/release/rc-workflow.sh create-rc 1.2.0
```

This updates version manifests, generates RC release notes, and stages `v1.2.0-rc.1`.

### 2. Validating the Release Candidate

```bash
./scripts/release/rc-workflow.sh validate-rc v1.2.0-rc.1
```

### 3. Promoting to Production

Once RC validation passes:

```bash
./scripts/release/rc-workflow.sh promote v1.2.0-rc.1
```

Tag and push to trigger automated GitHub Release and asset publishing:

```bash
git tag -a v1.2.0 -m "Release v1.2.0"
git push origin v1.2.0
```

---

## 🚨 Emergency Rollback Procedures

If an incident occurs with release `v1.2.0`:

```bash
./scripts/release/rollback-release.sh \
  --from v1.2.0 \
  --target v1.1.9 \
  --reason "Identified schema decoding regression in event stream"
```

Or trigger the GitHub Action:
- Go to **Actions** → **Release Rollback Automation** → **Run workflow**
- Specify `from_version: v1.2.0`, `target_version: v1.1.9`.
