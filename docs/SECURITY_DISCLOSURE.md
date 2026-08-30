# Responsible Disclosure Program

## Table of Contents

1. [Introduction](#introduction)
2. [Security Policy](#security-policy)
3. [Scope](#scope)
4. [Disclosure Process](#disclosure-process)
5. [Reward Tiers](#reward-tiers)
6. [Hall of Fame](#hall-of-fame)
7. [Contact Information](#contact-information)
8. [Safe Harbor](#safe-harbor)

## Introduction

The Decentralized Audit & Transparency Ledger project takes security seriously. We welcome responsible disclosure of security vulnerabilities from the community and offer rewards for qualifying reports.

### Our Commitment

- We will acknowledge your report within 48 hours
- We will investigate and respond with our assessment within 5 business days
- We will notify you when a fix is deployed
- We will credit you in our security advisory (unless you prefer anonymity)
- We will not take legal action against researchers who follow this policy

## Security Policy

### Reporting a Vulnerability

**DO NOT** file a public GitHub issue for security vulnerabilities.

Instead, send your report to: **security@auditledger.xyz**

### Required Information

Please include the following in your report:

```
Subject: [Security Vulnerability] Brief Description

Description:
- Type of vulnerability (e.g., Reentrancy, Access Control, etc.)
- Component affected (Contract, API, Bridge, SDK, etc.)
- Contract/service version or commit hash
- Steps to reproduce (detailed)
- Proof of concept (if available)
- Potential impact assessment
- Suggested fix (if any)

Environment:
- Network (testnet/mainnet)
- Soroban SDK version
- Relevant configuration

Contact Information:
- Your name/handle (for Hall of Fame)
- Preferred contact method
- PGP key (optional, for encrypted communication)
```

### Response Timeline

| Step | Timeline |
|------|----------|
| Acknowledgment | Within 48 hours |
| Triage & Severity Assessment | Within 5 business days |
| Fix Development (Critical) | Within 7 days |
| Fix Development (High) | Within 14 days |
| Fix Development (Medium) | Within 30 days |
| Fix Development (Low) | Within 90 days |
| Public Disclosure | After fix deployment or 90 days |

## Scope

### In Scope

- **Smart Contracts**: AuditLedger Soroban contract (`src/`)
- **Off-chain Services**: API services (`api/`), background services (`services/`)
- **Cross-chain Bridge**: Bridge implementation (`bridge/`)
- **SDKs**: JavaScript SDK (`sdk/js`), Python SDK (`sdk/python/`)
- **Build & Deployment**: Scripts (`scripts/`), Docker configurations (`docker/`)
- **Infrastructure**: Deployment configurations (`infra/`)
- **Cryptography**: Custom cryptographic implementations

### Out of Scope

- Vulnerabilities in the Stellar network or Soroban runtime
- Vulnerabilities in third-party dependencies (report to respective maintainers)
- Theoretical attacks without demonstrated exploit path
- Social engineering of project contributors
- Denial of service attacks that do not demonstrate security impact
- Issues requiring physical access to infrastructure

## Disclosure Process

### Step 1: Initial Report

Send your vulnerability report to security@auditledger.xyz

### Step 2: Acknowledgment

You will receive an acknowledgment within 48 hours with:
- A unique report ID
- Assigned severity level (preliminary)
- Expected timeline for assessment

### Step 3: Assessment

Our security team will:
- Reproduce the vulnerability
- Assess severity and impact
- Determine affected components
- Develop a fix plan

### Step 4: Fix Development

- Fix is developed and tested
- You will be notified of progress
- You may be asked to verify the fix

### Step 5: Deployment

- Fix is deployed to affected systems
- You will be notified of deployment
- Bounty is processed

### Step 6: Public Disclosure

- Security advisory is published
- You are credited (unless anonymity requested)
- CVE is assigned (if applicable)

## Reward Tiers

### Severity Classification

Rewards are based on the severity and impact of the vulnerability:

| Severity | Description | Reward Range |
|----------|-------------|--------------|
| **Critical** | Direct loss of funds, permanent contract compromise, or complete system takeover | $5,000 - $25,000 |
| **High** | Bypass of access controls, significant data integrity issues, or temporary fund freeze | $2,000 - $5,000 |
| **Medium** | Limited information disclosure, denial of service under specific conditions | $500 - $2,000 |
| **Low** | Best-practice violations, minor information leakage | $100 - $500 |

### Bonus Rewards

Additional rewards may be given for:

| Bonus Type | Description | Bonus |
|------------|-------------|-------|
| **Quality Report** | Exceptionally detailed report with clear reproduction steps | +10-25% |
| **Fix Provided** | Working fix included with the report | +25-50% |
| **Novel Finding** | New class of vulnerability not previously seen | +25-100% |
| **Responsible Disclosure** | Following all guidelines and timeline | +10% |

### Reward Eligibility

To be eligible for rewards:

1. You must be the first to report the vulnerability
2. You must follow the responsible disclosure process
3. You must not exploit the vulnerability beyond what's necessary to demonstrate it
4. You must not access or modify other users' data
5. You must not publicly disclose before we deploy a fix

### Reward Payment

Rewards are paid via:
- Cryptocurrency (XLM, USDC on Stellar)
- Bank wire transfer
- Other methods by agreement

## Hall of Fame

We publicly acknowledge security researchers who have helped improve our security.

### 2026

| Researcher | Finding | Severity | Date |
|------------|---------|----------|------|
| *Your name here* | - | - | - |

### Recognition

Researchers in our Hall of Fame receive:
- Recognition on our website and documentation
- Exclusive project swag
- Priority access to future bug bounty programs
- Invitation to private security channels

## Contact Information

### Security Team

- **Email**: security@auditledger.xyz
- **PGP Key**: [Download PGP Key](https://auditledger.xyz/pgp-key.txt)
- **Key Fingerprint**: `ABCD 1234 5678 90EF GHIJ 1234 5678 90AB CDEF 1234`

### Encryption

For sensitive communications, please encrypt your messages with our PGP key:

```bash
# Import our public key
curl https://auditledger.xyz/pgp-key.txt | gpg --import

# Encrypt your message
gpg --encrypt --armor --recipient security@auditledger.xyz message.txt
```

### Alternative Contact

If you cannot use email:
- **GitHub**: Send a DM to @daddygokings-art requesting security contact
- **Discord**: Join our server and message a maintainer privately

## Safe Harbor

### Our Promise

We consider security research conducted in accordance with this policy to be:

- **Authorized** access to the systems under our control
- **Exempt** from DMCA takedown requests
- **Not a violation** of our acceptable use policy
- **Protected** from legal action by the project

### Requirements for Safe Harbor

To be protected under safe harbor, researchers must:

1. **Follow this policy** in good faith
2. **Stop testing immediately** upon discovering a vulnerability
3. **Report promptly** to security@auditledger.xyz
4. **Not access more data** than necessary to demonstrate the vulnerability
5. **Not modify or exfiltrate data** beyond the minimum necessary
6. **Not cause permanent damage** to systems or data
7. **Not use social engineering** or physical attacks
8. **Respect the disclosure timeline** and embargo period

### Safe Harbor Limitations

Safe harbor does not apply if:

- The researcher causes willful harm or damage
- The researcher accesses data unrelated to the vulnerability
- The researcher violates laws outside the scope of authorized testing
- The researcher publicly discloses before the agreed timeline

## Security Advisories

### Published Advisories

| Advisory ID | Title | Severity | Date |
|-------------|-------|----------|------|
| *None yet* | - | - | - |

### Advisory Format

Our security advisories follow this format:

```
SA-2026-001: [Title]

Severity: Critical/High/Medium/Low
CVSS Score: X.X
Affected Versions: v0.x.x - v0.x.x
Fixed In: v0.x.x

Description:
[Brief description of the vulnerability]

Impact:
[What an attacker could do]

Mitigation:
[How to protect yourself before upgrading]

Credits:
[Researcher name/handle]
```

## Security Best Practices for Users

### For Contract Deployers

- Always verify contract WASM hashes before deployment
- Use multi-sig for admin operations
- Monitor contract events for anomalies
- Keep dependencies up to date

### For Integration Partners

- Validate all contract responses
- Implement rate limiting
- Use secure key storage
- Monitor for security advisories

### For Developers

- Follow secure coding practices
- Run security audits before deployment
- Use the latest SDK versions
- Report suspicious behavior

---

**Last Updated:** 2026-08-29

**Version:** 1.0
