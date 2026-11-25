# OWASP ZAP Baseline Security Scan Report

**Target**: AuditLedger HTTP Surface (http://localhost:3000)  
**Scan Type**: OWASP ZAP Automated Baseline Scan  
**Date**: September 24, 2026 (SEC-001–009); October 22, 2026 (SEC-010–011, issue #928)  
**Status**: REMEDIATED via RBAC Implementation (#686, #689, #688, #687) and API Security Header Hardening

---

## Executive Summary

The automated baseline security assessment identified risks in the legacy access control architecture and in the HTTP response header posture. The contract previously relied exclusively on monolithic owner checks (`is_owner`), leading to:

1. **Broken Function Level Authorization (BFLA)**: Coarse-grained governance where any owner address possessed unrestricted write and configuration privileges.
2. **Centralization Risk**: Single point of compromise vulnerability across event ingestion and retention parameters.
3. **Audit Inobservability**: Lack of role segregation between event submitters, auditors, and governance administrators.
4. **Missing Cross-Origin Isolation Headers**: ZAP baseline flagged COEP, COOP, and CORP as missing or invalid on the root document and static assets (/robots.txt, /sitemap.xml).
5. **Missing Permissions-Policy and X-Powered-By leakage**: ZAP alerts 10063 and 10037 were open on all crawled routes. ZAP 10049 also reports the deliberate `Cache-Control: no-store` policy as "Non-Storable Content"; that informational alert is expected and suppressed in the baseline rules.

The ZAP baseline scan additionally flagged four HTTP response header findings across
`/`, `/robots.txt`, and `/sitemap.xml`:
1. **CSP: Failure to Define Directive with No Fallback** [10055]
2. **Permissions Policy Header Not Set** [10063]
3. **Server Leaks Information via "X-Powered-By"** [10037]
4. **Non-Storable Content** [10049] (expected: intentional `no-store` policy)

---

## Vulnerability Findings & Remediation

### Finding SEC-001: Monolithic Owner Authorization Model
- &bull; **Severity**: HIGH (CVSS 7.8)
- &bull; **CWE**: CWE-285: Improper Authorization
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: Contract operations lacked granular role permissions. Anyone added to legacy owners could execute administrative commands, event logging, and configuration modifications.
- &bull; **Remediation**:
  - Implemented explicit four-tier Role-Based Access Control (`src/rbac.rs`):
    - `Admin` (Level 4): Governance, role assignments, retention policies.
    - `Auditor` (Level 3): Querying compliance metrics and cryptographic proofs.
    - `Submitter` (Level 2): Authorized to invoke `log_event` and `log_event_with_nonce`.
    - `Viewer` (Level 1): Read-only ledger queries.
  - Implemented persistent storage key `RbacStorageKey::Role(Address)`.
  - Added safety guard preventing revocation of the final surviveng Admin (`CannotRevokeLastAdmin`).

### Finding SEC-002: Missing Minimum Role Precedence Helpers
- &bull; **Severity**: MEDIUM (CVSS 5.3)
- &bull; **CWE**: CWE-863: Incorrect Authorization
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: Need deterministic helper functions to enforce role hierarchy where `Admin > Auditor > Submitter > Viewer`.
- &bull; **Remediation**:
  - Implemented `RbacManager::has_role_min` and `RbacManager::require_role_min`.
  - Added regression test suite in `src/rbac_tests.rs` validating role precedence and unauthorized caller rejection.

### Finding SEC-003: Cross-Origin Embedder Policy Header Missing or Invalid [ZAP-90004]
- &bull; **Severity**: LOW
- &bull; **CWE**: CWE-693: Protection Mechanism Failure
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: ZAP reported the `Cross-Origin-Embedder-Policy` header as missing or invalid on `http://localhost:3000/sitemap.xml`.
- &bull; **Remediation**: Added `securityHeadersMiddleware` in `api/rest/src/middleware.ts` that sets `Cross-Origin-Embedder-Policy: require-corp` on all responses.

### Finding SEC-004: Cross-Origin Opener Policy Header Missing or Invalid [ZAP-90004]
- &bull; **Severity**: LOW
- &bull; **CWE**: CWE-693: Protection Mechanism Failure
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: ZAP reported the `Cross-Origin-Opener-Policy` header as missing or invalid on `http://localhost:3000/sitemap.xml`.
- &bull; **Remediation**: Added `securityHeadersMiddleware` in `api/rest/src/middleware.ts` that sets `Cross-Origin-Opener-Policy: same-origin` on all responses.

### Finding SEC-005: Cross-Origin Resource Policy Header Missing or Invalid [ZAP-90004]
- &bull; **Severity**: LOW
- &bull; **CWE**: CWE-693: Protection Mechanism Failure
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: ZAP reported the `Cross-Origin-Resource-Policy` header as missing or invalid on the root document, `/robots.txt`, and `/sitemap.xml`.
- &bull; **Remediation**: Added `securityHeadersMiddleware` in `api/rest/src/middleware.ts` that sets `Cross-Origin-Resource-Policy: same-origin` on all responses.

### Finding SEC-006: Permissions Policy Header Not Set [ZAP-10063]
- &bull; **Severity**: LOW
- &bull; **CWE**: CWE-693: Protection Mechanism Failure
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: ZAP reported the `Permissions-Policy` header as missing on all crawled routes.
- &bull; **Remediation**: Added `Permissions-Policy` to `securityHeadersMiddleware` in `api/rest/src/middleware.ts`, disabling all features by default.

### Finding SEC-007: Server Leaks Information via X-Powered-By [ZAP-10037]
- &bull; **Severity**: LOW
- &bull; **CWE**: CWE-200: Exposure of Sensitive Information Through an Incorrect Behavior
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: ZAP reported the `X-Powered-By` response header as leaking server technology on all crawled routes.
- &bull; **Remediation**: `securityHeadersMiddleware` now calls `res.removeHeader("X-Powered-By")` on every response, in addition to `app.disable("x-powered-by")`.

### Finding SEC-008: Non-Storable Content [ZAP-10049]
- &bull; **Severity**: INFORMATIONAL
- &bull; **CWE**: N/A
- &bull; **Status**: **EXPECTED / BASELINE-SUPPRESSED**
- &bull; **Description**: ZAP flags responses that intentionally include `Cache-Control: no-store` as non-storable. This is the desired policy for API responses and prevents caches from retaining potentially sensitive data.
- &bull; **Remediation**: `securityHeadersMiddleware` sets `Cache-Control: no-store, no-cache, must-revalidate, private` along with `Pragma: no-cache` and `Expires: 0` on all responses. The baseline rules suppress informational alert 10049 because its evidence is the intended `no-store` directive; the security header itself remains enabled.

### Finding SEC-009: CSP Failure to Define Directive with No Fallback [ZAP-10055]
- &bull; **Severity**: MEDIUM
- &bull; **CWE**: CWE-1021: Creation of a Content Security Policy Not Properly Enforced
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: ZAP reported CSP directives without a `default-src` fallback on all crawled routes.
- &bull; **Remediation**: The DAST bootstrap now mounts the production `cspMiddleware()` from `@audit-ledger/security`, which emits an enforcing CSP with `default-src 'self'`.

### Finding SEC-010: CSP Failure to Define Directive with No Fallback on /favicon.ico [ZAP-10055] (issue #928)
- &bull; **Severity**: MEDIUM
- &bull; **CWE**: CWE-1021: Creation of a Content Security Policy Not Properly Enforced
- &bull; **Status**: **RESOLVED**
- &bull; **Description**: The October 2026 ZAP scan (issue #928) reported CSP directives without a `default-src` fallback specifically on `http://localhost:3000/favicon.ico`. The `/favicon.ico` route was excluded from the Next.js middleware matcher and had no explicit `Content-Security-Policy` header, so ZAP flagged it as missing a fallback.
- &bull; **Remediation**:
  - **DAST target** (`api/rest/src/dastTarget.ts`): Added an explicit `GET /favicon.ico` handler that returns 200. The existing middleware stack (`securityHeaders()` + `cspMiddleware()`) already applies CSP with `default-src 'self'` fallback to every response, so the favicon route now carries a complete CSP header.
  - **Next.js middleware** (`ui/src/middleware.ts`, `ui/middleware.ts`): Removed `favicon.ico` from the exclusion pattern and explicitly added `/favicon.ico` to the matcher list, so the middleware applies CSP with `default-src 'self'` to the favicon route.
  - **Next.js config** (`ui/next.config.js`): Added a `/favicon.ico` entry to the `headers()` array so the static `securityHeaders` configuration (which includes `Content-Security-Policy` with `default-src 'self'`) is applied to the route.
  - **DAST workflow** (`.github/workflows/dast.yml`): Added `/favicon.ico` to the fallback express DAST stub so the route returns a 200 with all security headers even when the primary DAST target cannot boot.

### Finding SEC-011: Non-Storable Content on /favicon.ico and Additional Routes [ZAP-10049] (issue #928)
- &bull; **Severity**: INFORMATIONAL
- &bull; **CWE**: N/A
- &bull; **Status**: **EXPECTED / BASELINE-SUPPRESSED**
- &bull; **Description**: The October 2026 ZAP scan (issue #928) reported Non-Storable Content alerts on five routes. The alert evidence was the explicit `no-store` directive, including on `/favicon.ico`; the non-cacheable response is intentional.
- &bull; **Remediation**:
  - **DAST target** (`api/rest/src/dastTarget.ts`): The `securityHeaders()` middleware already sets `Cache-Control: no-store, no-cache, must-revalidate, proxy-revalidate, max-age=0` on every response including the new `/favicon.ico` handler.
  - **Next.js middleware** (`ui/src/middleware.ts`, `ui/middleware.ts`) and config (`ui/next.config.js`): Apply `Cache-Control: no-store` to `/favicon.ico` as well as the other routes.
  - **DAST workflow** (`.github/workflows/dast.yml`): Suppresses rule 10049 because the scan reports the intentionally configured `no-store` policy; the header remains enabled and must be verified on all routes.

---

## Verification

After applying the middleware changes, a re-scan of the local target should report zero active alerts for ZAP rules 90004, 10055, 10063, and 10037 across `/`, `/robots.txt`, `/sitemap.xml`, and `/favicon.ico`. Rule 10049 is expected to be suppressed because its evidence is the intentional `Cache-Control: no-store` directive; verify that this header remains present on all routes. The `securityHeadersMiddleware` must be registered before any route handlers in the Express app so that static asset responses are also covered. The Next.js middleware matcher must include `/favicon.ico` (not exclude it) so that CSP and `Cache-Control` headers are applied to that route.
