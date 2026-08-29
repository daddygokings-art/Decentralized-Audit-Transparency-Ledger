# Documentation Style Guide

This guide establishes conventions for writing and formatting technical documentation in the AuditLedger repository.

## 1. Principles

- **Accuracy**: Code examples and JSON schemas must match live contract interfaces.
- **Clarity**: Write concise, direct sentences in active voice.
- **Completeness**: Every event, function, and regulatory control must document inputs, outputs, errors, and regulatory impacts.

## 2. Markdown Standards

- **Headings**: Use `H1` (`#`) once per file for the title. Do not skip heading levels (e.g. `H1` -> `H2` -> `H3`).
- **Code Blocks**: Always specify the language identifier:
  ````markdown
  ```json
  { "status": "ok" }
  ```
  ````
- **Alerts**: Use standard GitHub-style alerts:
  ```markdown
  > [!NOTE]
  > Helpful context or clarification.

  > [!IMPORTANT]
  > Essential compliance or security requirements.
  ```
- **Tables**: Use standard GitHub Flavored Markdown table syntax with aligned columns.

## 3. Terminology & Acronyms

- Spell out acronyms on first mention: e.g. "Real-World Assets (RWA)", "Markets in Crypto-Assets (MiCA)".
- Maintain custom vocabulary entries in `docs/.cspell/project-words.txt`.
