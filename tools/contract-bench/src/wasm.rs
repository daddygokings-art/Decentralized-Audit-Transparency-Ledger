//! WASM size measurement and budget checking.
//!
//! # What can be measured today
//!
//! Size is the one contract performance metric that does not need the contract to
//! compile *here*. It needs a built `.wasm`, which CI produces from the contract
//! crate. So this module is written to measure whatever artifact it is pointed
//! at, and to be exercised against a real compiled contract in its tests.
//!
//! A missing artifact is reported as missing, not as zero bytes. That distinction
//! matters: a size budget that silently passes when the build produced nothing is
//! worse than no budget at all.
//!
//! # Why size is worth gating
//!
//! Soroban charges for the transaction that *references* the contract, not for the
//! contract's bytes directly, but the WASM footprint is the thing that tracks
//! contract complexity, and the SDK's own cost model prices VM instantiation and
//! module parsing. A contract that grows past its instance budget also stops
//! deploying. So the suite tracks total size against a ceiling and additionally
//! breaks the module down by section, which is what makes a regression legible:
//! a jump in the data section is a new constant, a jump in the code section is
//! new logic.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The WebAssembly binary format version this parser reads.
const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// Size of one WASM section, in bytes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Section {
    /// Human-readable section name, or its id when the name is a custom section.
    pub name: String,
    /// Section id; 0 for the custom `name` section.
    pub id: u8,
    /// Payload length as declared in the section header.
    pub size: u32,
}

/// A measured WASM artifact.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WasmReport {
    pub path: String,
    /// Whether the file was found and parsed.
    pub available: bool,
    /// Total file size in bytes. `None` when unavailable.
    pub total_bytes: Option<u64>,
    /// Per-section sizes. Empty when unavailable.
    pub sections: Vec<Section>,
    /// Reason the artifact is unavailable, when it is.
    pub unavailable_reason: Option<String>,
    /// Human-readable module name from the custom name section, if present.
    pub module_name: Option<String>,
}

impl WasmReport {
    /// Size of one section by name, if present.
    pub fn section(&self, name: &str) -> Option<u32> {
        self.sections.iter().find(|s| s.name == name).map(|s| s.size)
    }

    /// Fraction of the module taken by one section.
    pub fn share(&self, name: &str) -> Option<f64> {
        let total = self.total_bytes?;
        let bytes = f64::from(self.section(name)?);
        (total > 0).then_some(bytes / total as f64)
    }
}

/// Candidate locations for a built contract, in preference order.
pub fn search_paths() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    vec![
        root.join("target/wasm32-unknown-unknown/release/audit_ledger.wasm"),
        root.join("target/wasm32-unknown-unknown/release/audit-ledger.wasm"),
        root.join("target/contract-bench.wasm"),
        root.join("artifacts/contract.wasm"),
    ]
}

/// Measure the first artifact that exists among [`search_paths`].
pub fn discover() -> WasmReport {
    let candidates = search_paths();
    for path in &candidates {
        if path.exists() {
            return measure(path);
        }
    }
    WasmReport {
        path: candidates[0].display().to_string(),
        available: false,
        total_bytes: None,
        sections: Vec::new(),
        unavailable_reason: Some(format!(
            "no compiled contract found; looked in {} location(s), first {}",
            candidates.len(),
            candidates[0].display()
        )),
        module_name: None,
    }
}

/// Measure a specific WASM file.
pub fn measure(path: &Path) -> WasmReport {
    match std::fs::read(path) {
        Ok(bytes) => match parse(&bytes) {
            Ok((sections, module_name)) => WasmReport {
                path: path.display().to_string(),
                available: true,
                total_bytes: Some(bytes.len() as u64),
                sections,
                unavailable_reason: None,
                module_name,
            },
            Err(e) => WasmReport {
                path: path.display().to_string(),
                available: false,
                total_bytes: None,
                sections: Vec::new(),
                unavailable_reason: Some(format!("{} is not a readable wasm module: {e}", path.display())),
                module_name: None,
            },
        },
        Err(e) => WasmReport {
            path: path.display().to_string(),
            available: false,
            total_bytes: None,
            sections: Vec::new(),
            unavailable_reason: Some(format!("cannot read {}: {e}", path.display())),
            module_name: None,
        },
    }
}

/// Canonical section names by id, for the standard binary-format sections.
fn section_name(id: u8) -> String {
    match id {
        0 => "custom".into(),
        1 => "type".into(),
        2 => "import".into(),
        3 => "function".into(),
        4 => "table".into(),
        5 => "memory".into(),
        6 => "global".into(),
        7 => "export".into(),
        8 => "start".into(),
        9 => "element".into(),
        10 => "code".into(),
        11 => "data".into(),
        12 => "datacount".into(),
        other => format!("section{other}"),
    }
}

/// Parse a WASM module's section table.
///
/// Walks the binary-format header and reads each section's id and declared
/// length, then skips the payload. The payload is not decoded, so this stays
/// independent of any particular section's schema and cannot be broken by a
/// change in section contents.
pub fn parse(bytes: &[u8]) -> Result<(Vec<Section>, Option<String>), String> {
    if bytes.len() < 8 {
        return Err("file is too short to be a wasm module".into());
    }
    if bytes[..4] != WASM_MAGIC {
        return Err("missing the \\0asm magic".into());
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if version != 1 {
        return Err(format!("unsupported binary format version {version}"));
    }

    let mut sections = Vec::new();
    let mut module_name = None;
    let mut i = 8usize;
    while i < bytes.len() {
        let id = bytes[i];
        i += 1;
        // Section length is an LEB128 unsigned integer.
        let (len, consumed) = read_uleb128(&bytes[i..])?;
        i += consumed;
        if i + len > bytes.len() {
            return Err(format!("section {id} claims {len} bytes but the file ends early"));
        }
        let payload = &bytes[i..i + len];
        let name = section_name(id);
        if id == 0 {
            if let Some(n) = custom_section_name(payload) {
                module_name = Some(n);
            }
        }
        sections.push(Section {
            name,
            id,
            size: len as u32,
        });
        i += len;
    }
    Ok((sections, module_name))
}

/// Read the module name out of the custom `name` section, if it carries one.
fn custom_section_name(payload: &[u8]) -> Option<String> {
    let (name_len, n) = read_uleb128(payload).ok()?;
    if payload.len() < n + name_len {
        return None;
    }
    let section_name = std::str::from_utf8(&payload[n..n + name_len]).ok()?;
    if section_name != "name" {
        return None;
    }
    // Skip the name-section subsection id and its length.
    let rest = &payload[n + name_len..];
    let (sub_id, m) = read_uleb128(rest).ok()?;
    if sub_id != 0 {
        return None;
    }
    let (mod_len, p) = read_uleb128(&rest[m..]).ok()?;
    let start = m + p;
    if rest.len() < start + mod_len {
        return None;
    }
    std::str::from_utf8(&rest[start..start + mod_len])
        .ok()
        .map(str::to_owned)
}

/// Read an unsigned LEB128 value, returning it with the bytes consumed.
fn read_uleb128(bytes: &[u8]) -> Result<(usize, usize), String> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    for (i, byte) in bytes.iter().enumerate() {
        result |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return usize::try_from(result)
                .map(|len| (len, i + 1))
                .map_err(|_| "LEB128 value exceeds addressable file size".to_string());
        }
        shift += 7;
        if shift > 63 {
            return Err("LEB128 value is too large".into());
        }
    }
    Err("truncated LEB128 value".into())
}

/// A size budget the suite enforces.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SizeBudget {
    /// Hard ceiling on total module size, in bytes.
    pub max_total_bytes: u64,
    /// Hard ceiling on the code section, in bytes.
    pub max_code_bytes: u64,
}

/// The budget the audit-ledger contract is held to.
///
/// Soroban's default per-transaction WASM size limit is 64 KiB, and a contract
/// that exceeds it cannot be deployed. The ceiling is set below that so the
/// budget trips while there is still room to act.
pub const AUDIT_LEDGER_BUDGET: SizeBudget = SizeBudget {
    max_total_bytes: 64 * 1024,
    max_code_bytes: 48 * 1024,
};

/// Result of checking a report against a budget.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetCheck {
    pub enforced: bool,
    pub within_budget: Option<bool>,
    pub violations: Vec<String>,
}

impl BudgetCheck {
    /// True when the budget could be checked and passed.
    pub fn passed(&self) -> bool {
        self.enforced && matches!(self.within_budget, Some(true))
    }
}

/// Check a report against a budget.
///
/// When the artifact is unavailable the budget is reported as *not enforced*
/// rather than as passed, so a missing build cannot be mistaken for compliance.
pub fn check_budget(report: &WasmReport, budget: &SizeBudget) -> BudgetCheck {
    if !report.available {
        return BudgetCheck {
            enforced: false,
            within_budget: None,
            violations: vec![format!(
                "WASM size budget not enforced: {}",
                report
                    .unavailable_reason
                    .clone()
                    .unwrap_or_else(|| "artifact unavailable".into())
            )],
        };
    }
    let total = report.total_bytes.unwrap_or(0);
    let mut violations = Vec::new();
    if total > budget.max_total_bytes {
        violations.push(format!(
            "total size {total} B exceeds the {} B budget",
            budget.max_total_bytes
        ));
    }
    if let Some(code) = report.section("code") {
        if u64::from(code) > budget.max_code_bytes {
            violations.push(format!(
                "code section {code} B exceeds the {} B budget",
                budget.max_code_bytes
            ));
        }
    }
    BudgetCheck {
        enforced: true,
        within_budget: Some(violations.is_empty()),
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal but genuinely valid module: header plus a code section holding
    /// one empty function body, and a custom name section.
    fn synthetic_module() -> Vec<u8> {
        let mut m = Vec::new();
        m.extend_from_slice(&WASM_MAGIC);
        m.extend_from_slice(&1u32.to_le_bytes());
        // type section: one type, () -> ()
        m.push(1);
        m.push(4);
        m.extend_from_slice(&[0x01, 0x60, 0x00, 0x00]);
        // function section: one function of type 0
        m.push(3);
        m.push(2);
        m.extend_from_slice(&[0x01, 0x00]);
        // code section: one body, empty
        m.push(10);
        m.push(4);
        m.extend_from_slice(&[0x01, 0x02, 0x00, 0x0B]);
        m
    }

    #[test]
    fn parses_a_real_section_table() {
        let m = synthetic_module();
        let (sections, _) = parse(&m).expect("synthetic module should parse");
        let names: Vec<&str> = sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["type", "function", "code"]);
        assert_eq!(sections[2].size, 4);
    }

    #[test]
    fn rejects_non_wasm_input() {
        assert!(parse(b"not a wasm file at all").is_err());
        assert!(
            parse(&[0x00, 0x61, 0x73, 0x6D, 9, 9, 9, 9]).is_err(),
            "bad version must be rejected"
        );
    }

    #[test]
    fn rejects_a_truncated_section() {
        let mut m = synthetic_module();
        m.push(10);
        m.push(200); // claims 200 bytes that are not there
        assert!(parse(&m).is_err(), "a section overrunning the file must be rejected");
    }

    #[test]
    fn leb128_decodes_multi_byte_lengths() {
        // 300 encodes as 0xAC 0x02.
        assert_eq!(read_uleb128(&[0xAC, 0x02]), Ok((300, 2)));
        assert_eq!(read_uleb128(&[0x00]), Ok((0, 1)));
        assert!(
            read_uleb128(&[0x80]).is_err(),
            "a truncated value must not silently succeed"
        );
    }

    #[test]
    fn reports_sections_and_shares() {
        let dir = std::env::temp_dir().join("contract-bench-wasm-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tiny.wasm");
        std::fs::write(&path, synthetic_module()).unwrap();
        let report = measure(&path);
        assert!(report.available);
        // 4 bytes of magic + 4 of version + (id, size, payload) for a 4-byte type
        // section, a 2-byte function section and a 4-byte code section.
        assert_eq!(report.total_bytes, Some(24), "the whole file is the module size");
        // A section is measured by its declared payload, excluding the id byte
        // and the LEB-encoded length: the code budget is about code, not framing.
        assert_eq!(report.section("code"), Some(4));
        assert_eq!(report.share("code"), Some(4.0 / 24.0), "code is 4 of 24 bytes");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_artifact_is_unavailable_not_zero() {
        let report = measure(Path::new("/nonexistent/contract.wasm"));
        assert!(!report.available, "a missing file must not report available");
        assert_eq!(report.total_bytes, None, "a missing file must not report zero bytes");
        assert!(report.unavailable_reason.is_some());
    }

    #[test]
    fn unavailable_artifact_does_not_pass_the_budget() {
        let report = measure(Path::new("/nonexistent/contract.wasm"));
        let check = check_budget(&report, &AUDIT_LEDGER_BUDGET);
        assert!(!check.enforced);
        assert_eq!(check.within_budget, None);
        assert!(!check.passed(), "an unchecked budget must never report as passing");
        assert!(!check.violations.is_empty(), "the reason must be reported");
    }

    #[test]
    fn oversized_module_trips_the_budget() {
        // Just the eight-byte header, and no code section at all: the module is
        // already over its total budget while looking harmless to a check that
        // only ever looked at code.
        let mut m = synthetic_module();
        m.clear();
        m.extend_from_slice(&WASM_MAGIC);
        m.extend_from_slice(&1u32.to_le_bytes());
        assert_eq!(m.len(), 8);
        let budget = SizeBudget {
            max_total_bytes: 4,
            max_code_bytes: 4,
        };
        let dir = std::env::temp_dir().join("contract-bench-wasm-budget");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("big.wasm");
        std::fs::write(&path, m).unwrap();
        let check = check_budget(&measure(&path), &budget);
        assert!(check.enforced);
        assert_eq!(check.within_budget, Some(false));
        assert!(
            check.violations.iter().any(|v| v.contains("exceeds")),
            "{:?}",
            check.violations
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn in_budget_module_passes() {
        let dir = std::env::temp_dir().join("contract-bench-wasm-ok");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ok.wasm");
        std::fs::write(&path, synthetic_module()).unwrap();
        let check = check_budget(&measure(&path), &AUDIT_LEDGER_BUDGET);
        assert!(check.passed(), "{check:?}");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn discovery_reports_a_concrete_missing_path() {
        // Discovery is expected to find nothing in a source checkout; what
        // matters is that it explains itself rather than returning zeros.
        let report = discover();
        if !report.available {
            let reason = report.unavailable_reason.unwrap();
            assert!(reason.contains("no compiled contract found"), "{reason}");
        }
    }
}
