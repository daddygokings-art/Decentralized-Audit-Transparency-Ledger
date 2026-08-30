# Security Scanning in CI/CD

This document describes the security scanning pipeline integrated into the
Decentralized Audit & Transparency Ledger CI/CD workflows.

Resolves: #484 (security scanning in CI/CD)
Related: #481 (disaster recovery), #482 (backup/restore), #483 (capacity planning)

---

## Overview

The security scanning pipeline provides defence-in-depth by running multiple
complementary tools across six distinct scan categories on every push and PR,
with additional scheduled runs for freshness:

| Workflow | Category | Key Tools | Trigger |
|----------|----------|-----------|---------|
| `sast.yml` | SAST | CodeQL, Semgrep, Clippy | push/PR/weekly |
| `dependency-scan.yml` | Dependency Scanning | cargo-audit, cargo-deny, npm audit, pip-audit | push/PR/daily |
| `secret-scan.yml` | Secret Scanning | Gitleaks, TruffleHog | push/PR/daily |
| `container-security.yml` | Container Scanning | Trivy, Grype, Dockle | push (docker paths)/nightly |
| `dast.yml` | DAST | OWASP ZAP, Nuclei | push (api paths)/weekly |
| `license-compliance.yml` | License Compliance | cargo-license, license-checker, pip-licenses | push/PR/weekly |
| `security-gate.yml` | Policy Gate & Reporting | Aggregator | push/PR/weekly |

All SARIF-formatted findings are uploaded to **GitHub Security → Code scanning**.
Raw JSON reports are available as workflow artifacts for 90 days.

---

## 1. SAST — Static Application Security Testing (`sast.yml`)

### Tools

| Tool | Languages | Purpose |
|------|-----------|---------|
| **CodeQL** | Rust, JavaScript/TypeScript | Deep semantic analysis; detects injection, XSS, SSRF, RCE patterns |
| **Semgrep** | Rust, TS/JS, smart contracts | Rule-set scanning (OWASP Top 10, blockchain-specific patterns) |
| **Clippy** | Rust | Compiler-level lints with `pedantic` security warnings |

### Policy

- CodeQL failures on `security-extended` queries → **block PR**
- Semgrep critical/high findings → **block PR**
- Clippy `-D warnings` → **block PR**

### Configuration

Semgrep uses the following rule sets:
- `p/default` — general-purpose rules
- `p/rust` — Rust-specific patterns
- `p/typescript` / `p/javascript` — TS/JS patterns
- `p/secrets` — accidental secret patterns
- `p/owasp-top-ten` — OWASP Top 10 coverage
- `p/smart-contracts` — Soroban/blockchain-specific rules

To add a Semgrep exclusion, create `.semgrepignore` in the repository root.

---

## 2. Dependency Scanning (`dependency-scan.yml`)

### Tools

| Tool | Ecosystem | Blocks On |
|------|-----------|-----------|
| **cargo-audit** | Rust (Cargo.lock) | Any advisory matching `--deny warnings` |
| **cargo-deny** | Rust | License violations + banned crates |
| **npm audit** | Node (package-lock.json) | `--audit-level=critical` |
| **pip-audit** | Python (pyproject.toml) | Any known CVE |
| **GitHub Dependency Review** | All (PRs only) | High severity + denied licenses |

### Policy

Denied licenses (all ecosystems):
```
AGPL-3.0, GPL-2.0, GPL-3.0, LGPL-2.0, LGPL-2.1, LGPL-3.0, SSPL-1.0
```

The Rust license policy is also enforced via `deny.toml`.

### Adding Exceptions

For time-bounded exceptions, add to `scripts/vulnerability-management/exceptions.yaml`:
```yaml
exceptions:
  - id: RUSTSEC-XXXX-XXXX
    reason: "Awaiting upstream fix — tracked in #NNN"
    expires: "2026-03-01"
    sign_off: "security-team"
```

---

## 3. Secret Scanning (`secret-scan.yml`)

### Tools

| Tool | Approach | Scope |
|------|----------|-------|
| **Gitleaks** | Regex + entropy | Full git history |
| **TruffleHog** | Entropy + provider verification | Full filesystem + git history |

### Policy

Any detected secret (unverified OR verified) → **block PR / fail CI**.

### Allowlists

Test fixtures and documentation placeholders are excluded via `.gitleaks.toml`.
Add patterns to the `[[allowlists]]` sections for legitimate false positives.

### Pre-commit Hook

Install Gitleaks as a pre-commit hook to catch secrets before they are committed:

```bash
# Install gitleaks
brew install gitleaks   # macOS
# or download from https://github.com/gitleaks/gitleaks/releases

# Install as pre-commit hook
cat > .git/hooks/pre-commit <<'EOF'
#!/bin/sh
gitleaks protect --staged --redact --exit-code 1
EOF
chmod +x .git/hooks/pre-commit
```

---

## 4. Container Scanning (`container-security.yml`)

### Tools

| Tool | Focus | Blocks On |
|------|-------|-----------|
| **Trivy** | OS packages + app libs | CRITICAL/HIGH (ignore-unfixed) |
| **Grype** | Second-opinion CVE DB | HIGH+ |
| **Dockle** | CIS Docker Benchmark | WARN+ |
| **Trivy config** | Dockerfile / IaC misconfigs | Reported (advisory) |

### Scanned Images

- `docker/Dockerfile.api` → `audit-ledger-api`
- `docker/Dockerfile.ui` → `audit-ledger-ui`
- `docker/Dockerfile.contract` → `audit-ledger-contract`

### Updating Base Images

When Trivy reports OS-level vulnerabilities that cannot be fixed in the
application layer, update the base image tag:

```dockerfile
# Before
FROM node:20-slim
# After — bump to latest minor to pull security patches
FROM node:20.19-slim
```

---

## 5. DAST — Dynamic Application Security Testing (`dast.yml`)

### Tools

| Tool | Scan Type | Target |
|------|-----------|--------|
| **OWASP ZAP Baseline** | Passive crawl | REST API (localhost or staging) |
| **OWASP ZAP API Scan** | OpenAPI-driven active scan | `api/openapi.yaml` |
| **Nuclei** | Template-based | REST API |

### Configuration

ZAP rule suppressions are in `.zap/rules.tsv`. Add rows to suppress known
false positives:

```tsv
<alert-id>\tIGNORE\t(Reason)
```

### Staging Environment

To run DAST against a real staging environment, add the secret:
```
STAGING_API_URL = https://api-staging.your-domain.example
```

DAST findings are currently **advisory only** and do not block PRs.
Once the ZAP baseline is established and all findings are triaged, promote
the gate to blocking by changing `dast-gate` to `exit 1` on failure.

---

## 6. License Compliance (`license-compliance.yml`)

### Tools

| Tool | Ecosystem |
|------|-----------|
| **cargo-deny** | Rust |
| **cargo-license** | Rust (report generation) |
| **license-checker** | Node.js |
| **pip-licenses** | Python |
| **FOSSA** | All (optional — requires `FOSSA_API_KEY` secret) |

### Denied Licenses

```
AGPL-3.0, GPL-2.0, GPL-3.0, LGPL-2.0, LGPL-2.1, LGPL-3.0, SSPL-1.0, Commons-Clause
```

License violations → **block PR**.

---

## 7. Security Policy Gate (`security-gate.yml`)

The `security-gate.yml` workflow aggregates all scan results and:

1. Posts a summary table as a PR comment (updates in-place)
2. Enforces the blocking policy (see table below)
3. Uploads a unified `security-report.json` artifact
4. On schedule: opens/updates a monthly posture tracking issue

### Blocking Policy

| Scan | Blocks Merge? |
|------|---------------|
| SAST failure | ✅ Yes |
| Dependency critical/high CVE | ✅ Yes |
| Committed secrets | ✅ Yes |
| Container CRITICAL/HIGH CVE | ✅ Yes |
| DAST findings | ⚠️ Advisory (not blocking) |
| License violation | ✅ Yes |

---

## Remediation SLA

| Severity | SLA |
|----------|-----|
| Critical | 24 hours |
| High | 7 days |
| Medium | 30 days |
| Low | 90 days |

Time-bounded exceptions with sign-off can be added to
`scripts/vulnerability-management/exceptions.yaml`.

---

## GitHub Permissions Required

The following GitHub secrets / permissions enable the full pipeline:

| Feature | Required |
|---------|---------|
| SARIF upload to Code scanning | `security-events: write` (set on each job) |
| PR comments | `pull-requests: write` |
| Issue creation | `issues: write` |
| FOSSA integration | `FOSSA_API_KEY` repository secret (optional) |
| Staging DAST | `STAGING_API_URL` repository secret (optional) |

All other scans run without additional secrets.

---

## Local Development

Run security scans locally before pushing:

```bash
# Rust — dependency + license audit
cargo install cargo-audit cargo-deny
cargo audit
cargo deny check

# Secret scan
brew install gitleaks   # or download binary
gitleaks detect --source . --redact

# Container scan
brew install trivy
trivy image audit-ledger-api:latest

# SAST (Semgrep)
pip install semgrep
semgrep --config p/default --config p/rust .
```
