# Penetration Testing Rules of Engagement (RoE)

## 1. Testing Windows & Environments
- **Primary Target Environment**: Staging and Dedicated Security Testnets (Stellar Testnet / Futurenet).
- **Production Testing**: Allowed only during designated maintenance windows (Sundays 01:00 - 05:00 UTC) and strictly non-destructive.
- **Notification**: Minimum 48 hours prior notice to the Security Operations Center (SOC) before active scanning.

## 2. Test Accounts & Authentication
- Testers are provisioned with dedicated test addresses (e.g. prefixed with `TEST_AUDITOR_`).
- Off-chain testing tokens carry an explicit `X-Security-Test: true` request header to prevent false alarms in operational telemetry.

## 3. Communication & Emergency Escalation
- **Daily Status Updates**: Slack/Discord `#security-audit-sync` channel.
- **Emergency Escalation Protocol (P0/Critical)**:
  - If a Critical vulnerability (CVSS >= 9.0) allowing remote code execution, fund theft, or historical ledger corruption is discovered:
  - **Immediate Action**: Testing on the vulnerable vector must cease immediately.
  - **Direct Notification**: Page on-call security incident commander via PagerDuty / Security Hotline within 1 hour.
  - **Encrypted Delivery**: Transmit full reproduction details encrypted via PGP key `security@auditledger.org`.

## 4. Prohibited Techniques
- Brute-forcing passwords or API keys beyond 100 requests per minute without rate-limit bypass PoC.
- Permanent destruction or irreversible modification of production state.
- Lateral movement outside the specified Kubernetes cluster namespace.
