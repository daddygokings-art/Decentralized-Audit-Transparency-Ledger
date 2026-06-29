# TODO

- [ ] Implement RBAC: add `Role` enum and storage `Role(Address) -> Role`.
- [ ] Add `set_role` (Admin-only) and `get_role` (public, returns `Option<Role>`).
- [ ] Add role gate helpers: `require_role_min` and role precedence (Admin > Auditor > Submitter > Viewer).
- [ ] Replace owner-only governance checks with RBAC (Admin-only).
- [ ] Gate `log_event` / `log_event_with_nonce` by `Submitter` role.
- [ ] Gate read operations by `Viewer` role (and `get_statistics` by `Auditor`).
- [ ] Update `transfer_ownership` to be Admin-only (legacy owner transfer removed as governance authority).
- [ ] Update/extend contract errors for role failures.
- [ ] Add tests for each role’s permissions + unauthorized operations + role transfers.
- [ ] Run `cargo test` and fix any ABI/client compilation issues.
