# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Contract event versioning with semantic versioning for event schemas.
- Version registry to store and look up event schema versions.
- Migration engine for automatic migration between schema versions.
- Compatibility checking between event schema versions.
- Deprecation policies for event schema versions.
- SDK support for event schema versioning.
- Dedicated contract monitoring dashboard with layout for overview, events, governance, performance, and health with real-time alerts (#404).
- Comprehensive developer portal with interactive API explorer, runnable code playground, and SDK guides for JS, Python, and Rust (#403).
- Event compliance and regulatory reporting engine for SOX, GDPR, MiCA, automated report generation, and GDPR erasure preservation (#402).
- Event replay protocol and state reconstruction from ledger history with incremental checkpointing, verification, and CLI tooling (#405).
- Hardened Content Security Policy with all no-fallback directives (base-uri, form-action, frame-ancestors, media-src, worker-src, manifest-src), Permissions-Policy, strict no-cache headers, and removal of X-Powered-By header across UI and REST endpoints (#729, #730, #731, #732).
