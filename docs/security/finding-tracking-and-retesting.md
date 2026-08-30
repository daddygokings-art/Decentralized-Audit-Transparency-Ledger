# Finding Tracking, SLAs, and Retesting Lifecycle

## 1. Remediation SLAs by Severity

Remediation timelines are strictly enforced based on CVSS v3.1 base score:

| Severity | CVSS Range | Remediation SLA | Escalation Window |
|----------|------------|-----------------|-------------------|
| **Critical** | 9.0 - 10.0 | **48 Hours** | 12 Hours |
| **High** | 7.0 - 8.9 | **7 Calendar Days** | 48 Hours |
| **Medium** | 4.0 - 6.9 | **30 Calendar Days** | 14 Days |
| **Low** | 0.1 - 3.9 | **90 Calendar Days** | 60 Days |
| **Informational** | N/A | **180 Days** | Best Effort |

## 2. Finding Lifecycle States

1. **`Reported`**: Initial finding submitted by auditor with CVSS score and reproduction steps.
2. **`Triaged`**: Internal security lead validates vulnerability and confirms severity.
3. **`In Remediation`**: Assigned engineer develops fix and adds regression tests.
4. **`Ready for Retest`**: Fix deployed to security testbed with commit hash documented.
5. **`Retesting`**: External auditor executes re-verification of the fix against original exploit vector.
6. **`Retest Passed`**: Auditor signs off on remediation effectiveness.
7. **`Closed`**: Finding resolved and merged to production.

## 3. Retesting Protocol
- No finding may be closed without written retest verification from the discovering auditor or a senior internal security architect.
- All code fixes must include dedicated automated regression test cases in the test suite.
