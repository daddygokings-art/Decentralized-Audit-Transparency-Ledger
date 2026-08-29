# ADR-017: Docs-as-Code Workflow and Multi-Version Documentation

| Field | Value |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-08-29 |
| **Deciders** | Documentation, Security, and Core Engineering Teams |

---

## Context

As the AuditLedger smart contract ecosystem expanded across multiple compliance domains (anti-corruption, export controls, trade compliance, stablecoin reserves, RWA), documenting contract events, payloads, schemas, and topics became essential for API consumers, auditors, and external partners.

Previously, documentation lived in loose markdown documents without automated linting, link checking, spell checking, or versioned release pipelines. This led to broken links, terminology inconsistencies, and doc drift against actual contract interfaces.

---

## Decision

We establish an enterprise **Docs-as-Code** workflow across the repository:

1. **Contract Event Documentation Taxonomy (`docs/contract-events/`)**:
   - Centralized catalog detailing core ledger events, compliance events, financial events, and governance events.
   - Machine-readable JSON schema repository (`event-schemas.json`) against which documentation examples are validated.

2. **Automated Quality Tooling**:
   - **Markdown Linting**: Configured via `.markdownlint.json`.
   - **Internal & External Link Checking**: Automated via `scripts/docs/check-links.sh` and `.lychee.toml`.
   - **Spell Checking**: CSpell configuration with domain dictionary (`docs/.cspell/project-words.txt`).
   - **Event Schema Validation**: Automated parser (`scripts/docs/validate-event-docs.py`) validating all JSON blocks against schemas.

3. **Multi-Version Deployment**:
   - Material for MkDocs configured with versioning provider `mike` (`mkdocs.yml`).
   - Multi-version builder script (`scripts/docs/build-multiversion.sh`) packaging `v1.0.0`, `v2.0.0`, `latest`, and `dev` versions with automatic redirect index.

4. **Contribution Standards**:
   - Documentation contribution guidelines (`docs/onboarding/docs-contribution-guide.md`).
   - Dedicated documentation style guide (`docs/style-guide/documentation.md`).
   - Documentation PR template (`.github/PULL_REQUEST_TEMPLATE/documentation.md`).

5. **CI/CD Integration (`.github/workflows/docs-as-code.yml`)**:
   - Automated quality gating and multi-version artifact publishing on every PR and release.

---

## Consequences

### Positive
- Zero broken links and consistent technical terminology across the entire documentation set.
- Guaranteed schema synchronization between contract event implementations and public docs.
- Versioned documentation allows consumers on older contract releases to browse relevant documentation.

### Negative
- Requires maintaining vocabulary lists in `project-words.txt` when introducing new domain concepts.
