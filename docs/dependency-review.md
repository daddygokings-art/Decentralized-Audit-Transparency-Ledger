# Dependency Review Policy

Issue #143: Automated vulnerability scanning for dependencies.

## Overview

The `dependency-review.yml` GitHub Action automatically scans new/updated dependencies in pull requests for known vulnerabilities using the [GitHub Dependency Review API](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/about-dependency-review).

## Workflow Details

| Property | Value |
|----------|-------|
| Trigger | `pull_request` events targeting `main` branch |
| Failure Condition | Any **high-severity** CVE found |
| Supported Ecosystems | npm, pip, Maven, Gradle, bundler, etc. |
| Action Version | `actions/dependency-review-action@v4` |

### Configuration

- **fail-on-severity**: `high` — blocks PR merge if high-severity vulnerability detected
- **allow-licenses**: Whitelist of acceptable open-source licenses
  - MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, MPL-2.0

## How It Works

1. PR submitted with `Cargo.toml`, `package.json`, or `poetry.lock` changes
2. Action fetches the dependency diff
3. Queries GitHub's vulnerability database (powered by OSV.dev)
4. Fails if high-severity CVE found in new/updated transitive dependencies
5. Blocks merge until vulnerability is resolved (updated/removed)

## Common Scenarios

### High-Severity CVE Found

```
❌ PR check fails:
  Example: lodash@4.17.15 has CVE-2021-23337 (High)
```

**Resolution:**
- Update to patched version
- Use `npm audit fix` or `cargo update`
- Re-push changes; action re-scans

### Low/Medium Severity Allowed

```
⚠️ PR check passes:
  Example: mongoose@5.0.0 has CVE-2022-24999 (Medium) — allowed
```

Only high-severity blocks. Medium/low require manual review.

### Transitive Dependency Vuln

```
❌ PR check fails:
  Direct: lodash-es@4.17.15
  Transitive: lodash@4.17.15 (dep of lodash-es)
```

Update the direct dependency to one with patched transitive deps.

## Maintenance

- Review and update `allow-licenses` quarterly
- Monitor [GitHub Advisories](https://github.com/advisories)
- Coordinate high-severity updates with release cycles

## References

- [Dependency Review Action](https://github.com/actions/dependency-review-action)
- [GitHub Advisory Database](https://github.com/advisories)
- [OSV.dev](https://osv.dev/)
