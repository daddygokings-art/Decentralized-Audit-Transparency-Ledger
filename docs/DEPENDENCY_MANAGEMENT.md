# Dependency Management

## Overview

This project uses [Renovate](https://docs.renovatebot.com/) for automated dependency updates. Renovate helps keep our dependencies secure and up-to-date by automatically opening PRs when new versions are available.

## Renovate Configuration

Our Renovate configuration is located in [`renovate.json`](../renovate.json).

### Features

- **Automated Updates**: PRs are opened automatically when new dependency versions are available
- **Grouping**: Related updates are grouped into single PRs to reduce noise
- **Auto-merge**: Patch and minor updates for non-breaking dependencies are auto-merged
- **Security Prioritization**: Security vulnerabilities are prioritized and flagged
- **Schedule**: Updates are batched weekly to reduce CI load

### Supported Ecosystems

| Ecosystem | Manager | File(s) | Auto-merge |
|-----------|---------|---------|------------|
| Rust | cargo | Cargo.toml, Cargo.lock | Patch & minor |
| JavaScript/Node | npm | package.json | Patch & minor |
| GitHub Actions | github-actions | .github/workflows/*.yml | Patch & minor |
| Docker | dockerfile | Dockerfile | Patch & minor |
| Python | pip | requirements.txt, setup.py | Patch & minor |

### Package Rules

#### Patch Updates

All patch updates are grouped and auto-merged:

```json
{
  "matchUpdateTypes": ["patch"],
  "groupName": "all patch updates",
  "automerge": true
}
```

#### Minor Updates

Minor updates are grouped by ecosystem and auto-merged for development dependencies:

```json
{
  "matchUpdateTypes": ["minor"],
  "matchDepTypes": ["devDependencies"],
  "automerge": true
}
```

#### Major Updates

Major (breaking) updates require manual review:

```json
{
  "matchUpdateTypes": ["major"],
  "automerge": false,
  "labels": ["breaking-change", "needs-review"]
}
```

#### Security Updates

Security updates are prioritized and processed immediately:

```json
{
  "matchCategories": ["security"],
  "prPriority": 10,
  "schedule": ["at any time"],
  "labels": ["security", "critical"]
}
```

### Schedule

| Ecosystem | Schedule |
|-----------|----------|
| Cargo (Rust) | Monday 6:00 AM UTC |
| npm (Node.js) | Tuesday 6:00 AM UTC |
| GitHub Actions | Monday 6:00 AM UTC |
| Docker | Wednesday 6:00 AM UTC |
| Security updates | Any time |

### PR Labels

Renovate PRs are automatically labeled:

- `dependencies` - All dependency updates
- `security` - Security-related updates
- `critical` - Critical security updates
- `breaking-change` - Updates with breaking changes
- `needs-review` - Updates requiring manual review
- `automation` - Automated PRs

## Auto-merge Policy

### Auto-merged

- Patch updates (x.y.Z)
- Minor updates for development dependencies
- GitHub Actions updates (minor and patch)
- Lock file maintenance

### Requires Manual Review

- Major updates (X.0.0)
- Updates to critical dependencies (e.g., soroban-sdk)
- Updates with known breaking changes
- Security updates (to verify fix effectiveness)

## Security Vulnerability Handling

When a security vulnerability is detected:

1. Renovate creates a high-priority PR
2. The PR is labeled `security` and `critical`
3. The security team is notified
4. The fix is reviewed and merged promptly
5. A security advisory is published if needed

### Vulnerability Alerts

Enable GitHub's Dependabot alerts for additional security monitoring:

- Navigate to Repository Settings → Security → Code security and analysis
- Enable "Dependabot alerts"
- Enable "Dependabot security updates"

## Lock File Maintenance

Renovate automatically maintains lock files:

- **Cargo.lock**: Updated weekly
- **package-lock.json**: Updated weekly
- **yarn.lock**: Updated weekly

Lock file maintenance ensures all transitive dependencies are up-to-date.

## Dependency Dashboard

Renovate provides a dependency dashboard issue that shows:

- All pending updates
- Scheduled updates
- Dashboard of dependency health

To enable, set `dependencyDashboard: true` in `renovate.json`.

## Manual Dependency Updates

If you need to update a dependency manually:

### Rust (Cargo)

```bash
# Update a specific crate
cargo update -p <crate-name>

# Update all crates
cargo update
```

### JavaScript/Node (npm)

```bash
# Update a package
npm update <package-name>

# Update all packages
npm update
```

### Docker

Update the base image tag in your Dockerfile:

```dockerfile
FROM rust:1.75-slim  # Update version as needed
```

## Best Practices

1. **Review PRs carefully** - Even auto-merged PRs should be reviewed
2. **Run tests** - Always run the test suite after dependency updates
3. **Check changelogs** - Review dependency changelogs for breaking changes
4. **Update regularly** - Don't let dependencies become outdated
5. **Monitor security** - Keep an eye on security advisories

## Troubleshooting

### Renovate not creating PRs

1. Check that Renovate is installed on the repository
2. Verify the `renovate.json` file is valid JSON
3. Check the Renovate logs for errors

### Auto-merge failing

1. Check CI status - Auto-merge requires passing CI
2. Verify branch protection rules allow auto-merge
3. Check for merge conflicts

### Too many PRs

Adjust the `prHourlyLimit` and `prConcurrentLimit` in `renovate.json`:

```json
{
  "prHourlyLimit": 5,
  "prConcurrentLimit": 3
}
```

## Configuration Reference

For full configuration options, see the [Renovate documentation](https://docs.renovatebot.com/configuration-options/).

---

**Last Updated:** 2026-08-29
