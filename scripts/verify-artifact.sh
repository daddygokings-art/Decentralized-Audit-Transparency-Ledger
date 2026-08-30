#!/usr/bin/env bash
# scripts/verify-artifact.sh — Standalone auditor script for verifying the
# audit-ledger WASM binary's Sigstore signature and SLSA provenance.
#
# Usage:
#   ./scripts/verify-artifact.sh [OPTIONS]
#
# Options:
#   --wasm PATH          Path to audit_ledger.wasm  (required)
#   --bundle PATH        Path to audit_ledger.wasm.bundle (Sigstore cosign bundle)
#   --provenance PATH    Path to audit_ledger.wasm.intoto.jsonl (SLSA provenance)
#   --checksums PATH     Path to wasm-checksums.txt
#   --source URI         Source repository URI (default: github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger)
#   --tag TAG            Release tag to verify against (e.g. v1.2.3)
#   --expected-sha256 H  Expected SHA-256 hex to compare against
#   --cosign-version V   cosign version to install if not present (default: 2.4.3)
#   --slsa-version V     slsa-verifier version to install (default: 2.7.0)
#   --skip-cosign        Skip Sigstore cosign signature verification
#   --skip-slsa          Skip SLSA provenance verification
#   --skip-sha256        Skip SHA-256 checksum verification
#   --install-tools      Auto-install cosign + slsa-verifier if missing
#   --help               Show this help
#
# Exit codes:
#   0  All requested verifications passed
#   1  One or more verifications failed
#   2  Prerequisites missing or bad arguments
#
# Issue #504 — SLSA Level 3 supply chain security
set -euo pipefail

# ── Colours ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; BOLD='\033[1m'; NC='\033[0m'
log()   { echo -e "${BLUE}[verify]${NC} $*"; }
ok()    { echo -e "${GREEN}[verify]${NC} ✓ $*"; }
warn()  { echo -e "${YELLOW}[verify]${NC} ⚠ $*"; }
error() { echo -e "${RED}[verify]${NC} ✗ $*" >&2; }
die()   { error "$*"; exit 1; }

# ── Defaults ──────────────────────────────────────────────────────────────────
WASM_PATH=""
BUNDLE_PATH=""
PROVENANCE_PATH=""
CHECKSUMS_PATH=""
SOURCE_URI="github.com/daddygokings-art/Decentralized-Audit-Transparency-Ledger"
TAG=""
EXPECTED_SHA256=""
COSIGN_VERSION="2.4.3"
SLSA_VERSION="2.7.0"
SKIP_COSIGN=0
SKIP_SLSA=0
SKIP_SHA256=0
INSTALL_TOOLS=0

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --wasm)             WASM_PATH="$2"; shift ;;
        --wasm=*)           WASM_PATH="${1#*=}" ;;
        --bundle)           BUNDLE_PATH="$2"; shift ;;
        --bundle=*)         BUNDLE_PATH="${1#*=}" ;;
        --provenance)       PROVENANCE_PATH="$2"; shift ;;
        --provenance=*)     PROVENANCE_PATH="${1#*=}" ;;
        --checksums)        CHECKSUMS_PATH="$2"; shift ;;
        --checksums=*)      CHECKSUMS_PATH="${1#*=}" ;;
        --source)           SOURCE_URI="$2"; shift ;;
        --source=*)         SOURCE_URI="${1#*=}" ;;
        --tag)              TAG="$2"; shift ;;
        --tag=*)            TAG="${1#*=}" ;;
        --expected-sha256)  EXPECTED_SHA256="$2"; shift ;;
        --expected-sha256=*) EXPECTED_SHA256="${1#*=}" ;;
        --cosign-version)   COSIGN_VERSION="$2"; shift ;;
        --slsa-version)     SLSA_VERSION="$2"; shift ;;
        --skip-cosign)      SKIP_COSIGN=1 ;;
        --skip-slsa)        SKIP_SLSA=1 ;;
        --skip-sha256)      SKIP_SHA256=1 ;;
        --install-tools)    INSTALL_TOOLS=1 ;;
        --help|-h)
            sed -n '2,/^set -/{ /^set -/d; s/^# \{0,3\}//; p }' "$0"
            exit 0 ;;
        *) die "Unknown argument: $1" ;;
    esac
    shift
done

# ── Validate inputs ───────────────────────────────────────────────────────────
[[ -n "$WASM_PATH" ]] || die "--wasm PATH is required"
[[ -f "$WASM_PATH" ]] || die "WASM file not found: $WASM_PATH"

echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  Audit Ledger WASM Artifact Verification${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
echo ""
log "Artifact:    $WASM_PATH"
log "Source:      $SOURCE_URI"
[[ -n "$TAG" ]] && log "Tag:         $TAG"
[[ -n "$BUNDLE_PATH" ]] && log "Bundle:      $BUNDLE_PATH"
[[ -n "$PROVENANCE_PATH" ]] && log "Provenance:  $PROVENANCE_PATH"
echo ""

FAILURES=0

# ── Tool installer helper ─────────────────────────────────────────────────────
install_cosign() {
    local version="$1"
    log "Installing cosign v${version}…"
    local url="https://github.com/sigstore/cosign/releases/download/v${version}/cosign-linux-amd64"
    local sha_url="${url}.sha256"
    local dest="/usr/local/bin/cosign"

    curl -fsSL "$url" -o "$dest"
    curl -fsSL "$sha_url" -o /tmp/cosign.sha256

    # Verify cosign's own checksum
    EXPECTED_COSIGN_SHA=$(cut -d' ' -f1 /tmp/cosign.sha256)
    ACTUAL_COSIGN_SHA=$(sha256sum "$dest" | cut -d' ' -f1)
    if [[ "$EXPECTED_COSIGN_SHA" != "$ACTUAL_COSIGN_SHA" ]]; then
        die "cosign checksum mismatch! Binary may have been tampered with."
    fi
    chmod +x "$dest"
    ok "cosign v${version} installed and checksum verified"
}

install_slsa_verifier() {
    local version="$1"
    log "Installing slsa-verifier v${version}…"
    local url="https://github.com/slsa-framework/slsa-verifier/releases/download/v${version}/slsa-verifier-linux-amd64"
    local sha_url="${url}.sha256"
    local dest="/usr/local/bin/slsa-verifier"

    curl -fsSL "$url" -o "$dest"
    curl -fsSL "$sha_url" -o /tmp/slsa-verifier.sha256

    EXPECTED_SLSA_SHA=$(cut -d' ' -f1 /tmp/slsa-verifier.sha256)
    ACTUAL_SLSA_SHA=$(sha256sum "$dest" | cut -d' ' -f1)
    if [[ "$EXPECTED_SLSA_SHA" != "$ACTUAL_SLSA_SHA" ]]; then
        die "slsa-verifier checksum mismatch! Binary may have been tampered with."
    fi
    chmod +x "$dest"
    ok "slsa-verifier v${version} installed and checksum verified"
}

# ── Check / install prerequisites ─────────────────────────────────────────────
if [[ $SKIP_COSIGN -eq 0 ]]; then
    if ! command -v cosign &>/dev/null; then
        if [[ $INSTALL_TOOLS -eq 1 ]]; then
            install_cosign "$COSIGN_VERSION"
        else
            die "cosign not found. Install it: https://docs.sigstore.dev/system_config/installation/ or run with --install-tools"
        fi
    else
        ok "cosign found: $(cosign version 2>&1 | head -1)"
    fi
fi

if [[ $SKIP_SLSA -eq 0 && -n "$PROVENANCE_PATH" ]]; then
    if ! command -v slsa-verifier &>/dev/null; then
        if [[ $INSTALL_TOOLS -eq 1 ]]; then
            install_slsa_verifier "$SLSA_VERSION"
        else
            die "slsa-verifier not found. Install it: https://github.com/slsa-framework/slsa-verifier or run with --install-tools"
        fi
    else
        ok "slsa-verifier found: $(slsa-verifier version 2>&1 | head -1)"
    fi
fi

echo ""

# ── Step 1: SHA-256 checksum ─────────────────────────────────────────────────
echo -e "${BOLD}Step 1: SHA-256 checksum${NC}"
echo "──────────────────────────────────────"

ACTUAL_SHA256=$(sha256sum "$WASM_PATH" | cut -d' ' -f1)
log "Computed SHA-256: ${ACTUAL_SHA256}"

if [[ $SKIP_SHA256 -eq 1 ]]; then
    warn "SHA-256 check skipped (--skip-sha256)"
else
    if [[ -n "$EXPECTED_SHA256" ]]; then
        if [[ "$ACTUAL_SHA256" == "$EXPECTED_SHA256" ]]; then
            ok "SHA-256 matches expected value"
        else
            error "SHA-256 MISMATCH:"
            error "  Expected: ${EXPECTED_SHA256}"
            error "  Actual:   ${ACTUAL_SHA256}"
            FAILURES=$((FAILURES + 1))
        fi
    elif [[ -n "$CHECKSUMS_PATH" && -f "$CHECKSUMS_PATH" ]]; then
        EXPECTED_FROM_FILE=$(grep 'audit_ledger.wasm' "$CHECKSUMS_PATH" | cut -d' ' -f1 || true)
        if [[ -z "$EXPECTED_FROM_FILE" ]]; then
            warn "audit_ledger.wasm not found in checksums file"
        elif [[ "$ACTUAL_SHA256" == "$EXPECTED_FROM_FILE" ]]; then
            ok "SHA-256 matches checksums file"
        else
            error "SHA-256 MISMATCH (vs checksums file):"
            error "  Expected: ${EXPECTED_FROM_FILE}"
            error "  Actual:   ${ACTUAL_SHA256}"
            FAILURES=$((FAILURES + 1))
        fi
    else
        warn "No expected SHA-256 provided (--expected-sha256 or --checksums). Skipping comparison."
        log "Computed: ${ACTUAL_SHA256}"
    fi
fi
echo ""

# ── Step 2: Sigstore cosign signature ────────────────────────────────────────
echo -e "${BOLD}Step 2: Sigstore cosign signature${NC}"
echo "──────────────────────────────────────"

if [[ $SKIP_COSIGN -eq 1 ]]; then
    warn "Cosign verification skipped (--skip-cosign)"
elif [[ -z "$BUNDLE_PATH" ]]; then
    warn "No --bundle provided. Skipping Sigstore verification."
    warn "Download the .bundle file from the GitHub Release to verify."
elif [[ ! -f "$BUNDLE_PATH" ]]; then
    error "Bundle file not found: $BUNDLE_PATH"
    FAILURES=$((FAILURES + 1))
else
    log "Verifying Sigstore bundle: $BUNDLE_PATH"
    log "Certificate identity regexp: https://github.com/${SOURCE_URI#github.com/}/"
    log "Certificate OIDC issuer: https://token.actions.githubusercontent.com"

    COSIGN_ARGS=(
        verify-blob
        --bundle "$BUNDLE_PATH"
        --certificate-identity-regexp "https://github.com/${SOURCE_URI#github.com/}/"
        --certificate-oidc-issuer "https://token.actions.githubusercontent.com"
    )

    if cosign "${COSIGN_ARGS[@]}" "$WASM_PATH" 2>&1; then
        ok "Sigstore cosign signature verified"
        ok "  The WASM binary was built by the CI workflow in ${SOURCE_URI}"
        ok "  The signing certificate was issued by Fulcio to the GitHub Actions OIDC token"
        ok "  The signature is recorded in the Sigstore Rekor transparency log"
    else
        error "Sigstore cosign signature verification FAILED"
        error "  This could indicate:"
        error "    - The artifact was modified after signing"
        error "    - The bundle does not correspond to this artifact"
        error "    - The signing identity does not match ${SOURCE_URI}"
        FAILURES=$((FAILURES + 1))
    fi
fi
echo ""

# ── Step 3: SLSA provenance ───────────────────────────────────────────────────
echo -e "${BOLD}Step 3: SLSA Level 3 provenance${NC}"
echo "──────────────────────────────────────"

if [[ $SKIP_SLSA -eq 1 ]]; then
    warn "SLSA provenance verification skipped (--skip-slsa)"
elif [[ -z "$PROVENANCE_PATH" ]]; then
    warn "No --provenance provided. Skipping SLSA verification."
    warn "Download audit_ledger.wasm.intoto.jsonl from the GitHub Release to verify."
elif [[ ! -f "$PROVENANCE_PATH" ]]; then
    error "Provenance file not found: $PROVENANCE_PATH"
    FAILURES=$((FAILURES + 1))
elif ! command -v slsa-verifier &>/dev/null; then
    warn "slsa-verifier not installed. Skipping SLSA check."
    warn "Install: https://github.com/slsa-framework/slsa-verifier/releases"
else
    log "Verifying SLSA provenance: $PROVENANCE_PATH"
    log "Source URI: ${SOURCE_URI}"
    [[ -n "$TAG" ]] && log "Source tag: ${TAG}"

    SLSA_ARGS=(
        verify-artifact
        "$WASM_PATH"
        --provenance-path "$PROVENANCE_PATH"
        --source-uri "${SOURCE_URI}"
    )
    [[ -n "$TAG" ]] && SLSA_ARGS+=(--source-tag "$TAG")

    if slsa-verifier "${SLSA_ARGS[@]}" 2>&1; then
        ok "SLSA Level 3 provenance verified"
        ok "  The provenance attests that this binary was built from ${SOURCE_URI}"
        ok "  The provenance was generated by slsa-framework/slsa-github-generator"
        ok "  (not by repository code — non-forgeable)"
        [[ -n "$TAG" ]] && ok "  Tag: ${TAG}"
    else
        error "SLSA provenance verification FAILED"
        error "  This could indicate:"
        error "    - The artifact was built from a different source"
        error "    - The provenance was forged or does not match this artifact"
        error "    - The source URI or tag does not match"
        FAILURES=$((FAILURES + 1))
    fi
fi
echo ""

# ── Step 4: Reproducibility hint ─────────────────────────────────────────────
echo -e "${BOLD}Step 4: Reproducibility (manual)${NC}"
echo "──────────────────────────────────────"
log "To independently reproduce this binary:"
echo ""
echo "    git clone https://${SOURCE_URI}.git"
echo "    cd $(basename "$SOURCE_URI")"
echo "    git checkout ${TAG:-<commit-sha>}"
echo "    rustup install 1.85.0"
echo "    rustup target add wasm32v1-none --toolchain 1.85.0"
echo "    SOURCE_DATE_EPOCH=0 CARGO_INCREMENTAL=0 \\"
echo "      cargo +1.85.0 build --target wasm32v1-none --release --locked --frozen"
echo "    sha256sum target/wasm32v1-none/release/audit_ledger.wasm"
echo "    # Should produce: ${ACTUAL_SHA256}"
echo ""

# ── Final result ──────────────────────────────────────────────────────────────
echo -e "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
if [[ $FAILURES -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}  ✓ ALL VERIFICATION CHECKS PASSED${NC}"
    echo ""
    echo -e "  Artifact: ${BOLD}${WASM_PATH}${NC}"
    echo -e "  SHA-256:  ${ACTUAL_SHA256}"
else
    echo -e "${RED}${BOLD}  ✗ VERIFICATION FAILED: ${FAILURES} check(s) did not pass${NC}"
    echo ""
    echo "  DO NOT deploy this artifact until verification passes."
fi
echo -e "${BOLD}═══════════════════════════════════════════════════════════════${NC}"
echo ""

exit $((FAILURES > 0 ? 1 : 0))
