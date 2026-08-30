# ADR 016: Contract Event Security Training and Awareness Architecture

## Status
Accepted

## Context
As the Decentralized Audit Ledger expands with high-stakes financial, regulatory, ESG, and CBDC integrations, the security posture of the codebase directly depends on the continuous training, threat modeling capabilities, and security awareness of contributors and maintainers. Regulatory frameworks including SOC 2 (CC2.1, CC6.1), ISO 27001 (A.7.2.2), and MiCA (Article 73) require verifiable evidence of security training, phishing resilience, and role-based security governance.

## Decision
1. **On-Chain Training Registry**: Implement `src/security_training.rs` (`SecurityTrainingProgram`) to record course modules, developer training completions, cryptographic certificate hashes, expiry timestamps, phishing campaign metrics, and security champion credentials.
2. **Four-Pillar Curriculum**: Standardize core developer training across (1) Secure Smart Contract Coding, (2) Threat Modeling (STRIDE/DREAD), (3) Incident Response Runbooks, and (4) Regulatory Compliance.
3. **Phishing Simulation Orchestration**: Implement campaign management tracking reporting rate, click rate, and credentials compromise rate, integrated with automated re-training triggers.
4. **Security Champion Network**: Formalize a four-tier champion hierarchy (Associate, Practitioner, Lead, Fellow) with on-chain activity logging for PR reviews and threat modeling.
5. **CI/CD Enforcement Gate**: Enforce developer training validity via automated pre-merge checks in GitHub Actions.

## Consequences
### Positive
- Verifiable, tamper-evident record of developer security qualifications.
- Automated compliance evidence generation for SOC 2 and ISO 27001 audits.
- Proactive reduction of smart contract vulnerabilities and social engineering risks.
- Decentralized scaling of security reviews through trained Security Champions.

### Tradeoffs
- Slight operational overhead for developers to complete annual recertifications.
- Gas / storage footprint for storing training records and phishing campaign aggregates on-chain (mitigated by storing detailed content hashes rather than full courseware).
