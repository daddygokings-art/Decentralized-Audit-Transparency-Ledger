# Supply-Chain Controls

JavaScript packages use the public npm registry by default. The reserved `@audit-ledger` scope can be mapped to an approved private registry by copying `.npmrc.example` to `.npmrc` and setting `NPM_PRIVATE_REGISTRY_URL` to an HTTPS endpoint. Credentials must be supplied through the developer or CI environment; they must never be committed.

`scripts/supply-chain/verify-dependencies.sh` runs `npm ci --ignore-scripts` in every package root, which verifies lockfile resolution and integrity without executing install hooks. It also rejects git, HTTP, and direct tarball dependencies in manifests. Dependabot keeps dependency updates visible, while `cargo-deny`, GitHub Dependency Review, and the JavaScript license gate enforce the approved source and license policy in CI.

When adding a dependency:

1. Use a package name and an approved registry, not a git URL or direct archive.
2. Commit the corresponding lockfile and review its `integrity` and `resolved` entries.
3. Explain private-scope ownership and any license exception in the PR review notes.