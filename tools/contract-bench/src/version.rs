//! Which `soroban-sdk` version metered a set of results.
//!
//! The SDK's cost model moves between releases, so comparing two runs metered by
//! different SDKs is not a like-for-like comparison. Every result therefore
//! records the version that produced it, read from the crate's own `Cargo.lock`
//! rather than hardcoded, so that bumping the dependency updates the recorded
//! value automatically instead of silently leaving a stale one behind.

use std::path::Path;

/// Extract the `soroban-sdk` version from Cargo.lock contents.
///
/// Returns `None` if the package is absent, which the caller surfaces as
/// `unknown` rather than substituting a guess.
pub fn sdk_version(lock: &str) -> Option<String> {
    let mut in_soroban = false;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_soroban = false;
        } else if let Some(name) = line.strip_prefix("name = ") {
            in_soroban = name.trim_matches('"') == "soroban-sdk";
        } else if in_soroban {
            if let Some(version) = line.strip_prefix("version = ") {
                return Some(version.trim_matches('"').to_owned());
            }
        }
    }
    None
}

/// The `soroban-sdk` version this crate resolved to.
///
/// Recorded as `unknown` when the lockfile cannot be read or has no `soroban-sdk`
/// entry, so a missing version is visible in a report rather than being filled in
/// with a value that might be wrong.
pub fn sdk_version_from_lockfile() -> String {
    let lock = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
    std::fs::read_to_string(lock)
        .ok()
        .and_then(|contents| sdk_version(&contents))
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_from_a_package_stanza() {
        let lock = r#"
version = 4

[[package]]
name = "soroban-sdk"
version = "27.0.6"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
        assert_eq!(sdk_version(lock).as_deref(), Some("27.0.6"));
    }

    #[test]
    fn does_not_match_a_similarly_named_package() {
        let lock = r#"
[[package]]
name = "soroban-sdk-macros"
version = "27.0.6"

[[package]]
name = "soroban-sdk"
version = "27.0.5"
"#;
        assert_eq!(sdk_version(lock).as_deref(), Some("27.0.5"));
    }

    #[test]
    fn a_stanza_with_no_version_line_does_not_match() {
        let lock = r#"
[[package]]
name = "soroban-sdk"

[[package]]
name = "soroban-env-host"
version = "27.0.1"
"#;
        assert_eq!(sdk_version(lock), None);
    }

    #[test]
    fn reports_none_when_the_sdk_is_absent_or_the_lockfile_is_empty() {
        assert_eq!(sdk_version("[[package]]\nname = \"serde\"\nversion = \"1\"\n"), None);
        assert_eq!(sdk_version(""), None);
    }

    #[test]
    fn the_real_lockfile_resolves_to_a_concrete_version() {
        // Guards the wiring, not just the parser: a rename or a missing lockfile
        // would otherwise leave the suite reporting `unknown` forever.
        let version = sdk_version_from_lockfile();
        assert_ne!(version, "unknown", "this crate's own Cargo.lock must resolve");
        assert!(version.starts_with("27."), "unexpected soroban-sdk version: {version}");
    }
}
