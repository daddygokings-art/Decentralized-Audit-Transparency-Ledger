## Secure change review

### Automated checks

- [ ] CI passes, including dependency, license, secret, and vulnerability scans.
- [ ] New or changed dependencies are pinned in a lockfile and come from an approved registry.
- [ ] No package, image, or action is referenced by an untrusted mutable tag.

### Manual review

- [ ] Access control, authentication, authorization, and input validation were reviewed.
- [ ] Contract state changes and emitted events preserve auditability and do not leak secrets.
- [ ] Error paths fail closed and do not expose sensitive data.
- [ ] Infrastructure changes preserve least privilege, TLS, and secure secret handling.
- [ ] Tests cover the changed behavior, including negative and boundary cases.
- [ ] Any accepted risk has an owner, expiry date, and tracking issue.

### Reviewer notes

<!-- Summarize security-sensitive decisions, exceptions, and evidence here. -->