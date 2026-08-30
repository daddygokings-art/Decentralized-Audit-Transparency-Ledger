# Documentation Contribution Guidelines

Thank you for contributing to the AuditLedger documentation! We treat documentation with the same rigor as code ("Docs-as-Code").

## 1. Docs-as-Code Workflow

1. **Create a Feature Branch**:
   ```bash
   git checkout -b docs/add-new-feature-docs
   ```

2. **Run Local Automated Validations**:
   Before opening a PR, ensure all checks pass:
   ```bash
   # 1. Check internal and external links
   ./scripts/docs/check-links.sh

   # 2. Check spelling and domain terminology
   ./scripts/docs/check-spelling.sh

   # 3. Validate contract event JSON schemas
   python3 ./scripts/docs/validate-event-docs.py

   # 4. Preview multi-version build
   ./scripts/docs/build-multiversion.sh
   ```

3. **Adding New Contract Events**:
   When introducing or modifying a Soroban contract event:
   - Update `docs/contract-events/event-schemas.json` with the event payload schema.
   - Add a reference section in the relevant category doc (`core-events.md`, `compliance-events.md`, `financial-events.md`, or `governance-events.md`).
   - Include clear description of topics, payloads, and regulatory mappings.

4. **Submitting a Documentation PR**:
   - Use the `documentation.md` PR template.
   - Check all automated validation boxes.

## 2. Style Conventions

- Follow [Documentation Style Guide](../style-guide/documentation.md).
- Use title case for headings.
- Format code blocks with specific syntax highlighters (`rust`, `json`, `bash`, `rego`).
- Ensure all custom terms are added to `docs/.cspell/project-words.txt`.
