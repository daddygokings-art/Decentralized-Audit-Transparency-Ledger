# Formal Verification of AuditLedger

Issue #378 — This document describes the formal verification approach used to
prove critical safety and access-control properties of the AuditLedger Soroban
smart contract.

---

## Overview

Formal verification uses mathematical proofs to guarantee that smart contract
code behaves correctly for **all possible inputs and states** — not just the
inputs covered by tests. For an immutable audit ledger, correctness properties
such as event immutability, cap enforcement, and hash-chain integrity are
non-negotiable: a single violation could permanently corrupt the audit record.

Two complementary tools are used:

| Tool | Strengths | File |
|------|-----------|------|
| **Certora Prover** | Exhaustive symbolic execution over EVM bytecode; CVL property language | `formal-verification/audit_ledger.spec` |
| **K-framework** | Operational semantics; arbitrary rewrite rules; SMT-backed proofs | `formal-verification/k-spec/audit_ledger-spec.k` |

A human-readable property catalog with verification status is in
[`formal-verification/properties.md`](../formal-verification/properties.md).

---

## Verified Properties

| ID | Property | Category | Status |
|----|----------|----------|--------|
| P-01 | Event immutability | safety | ✅ |
| P-02 | totalEvents monotonically increases | safety | ✅ |
| P-03 | Global cap enforcement | safety | ✅ |
| P-04 | Per-event-type cap enforcement | safety | 🔶 |
| P-05 | setGlobalMaxLogs owner-only | access-control | ✅ |
| P-06 | setEventMaxLogs owner-only | access-control | ✅ |
| P-07 | removeEventCap owner-only | access-control | ✅ |
| P-08 | transferOwnership owner-only | access-control | ✅ |
| P-09 | setPaused owner-only | access-control | ✅ |
| P-10 | Paused contract blocks writes | safety | ✅ |
| P-11 | Initialization idempotence | safety | ✅ |
| P-12 | Zero-address owner rejected | safety | ✅ |
| P-13 | Ownership transfer correctness | safety | ✅ |
| P-14 | Hash chain integrity (prev_hash) | safety | 🔶 |
| P-15 | Nonce strictly monotone per submitter | safety | 🔶 |
| P-16 | Cap rejects values below current count | safety | 🔶 |
| P-17 | Cap removal idempotence | safety | 🔶 |
| P-18 | Uninitialised contract blocks writes | safety | ✅ |
| P-19 | Same-owner transfer rejected | safety | 🔶 |
| P-20 | Inverted index count consistent | safety | 🔲 |

✅ Verified · 🔶 Partial (bounded) · 🔲 Pending

---

## Certora Prover

### Setup

```bash
# Install the Certora CLI
pip install certora-cli

# Set your API key (register at https://www.certora.com)
export CERTORAKEY="<your-api-key>"
```

### Running the verifier

```bash
# Full property suite
certoraRun bridge/evm/ZkVerifier.sol \
  --verify ZkVerifier:formal-verification/audit_ledger.spec \
  --solc solc8.24 \
  --msg "AuditLedger full verification"

# Single rule (faster for iteration)
certoraRun bridge/evm/ZkVerifier.sol \
  --verify ZkVerifier:formal-verification/audit_ledger.spec \
  --rule eventImmutability \
  --msg "Event immutability only"

# Invariants only
certoraRun bridge/evm/ZkVerifier.sol \
  --verify ZkVerifier:formal-verification/audit_ledger.spec \
  --rule globalCapRespected contractMustBeInitialized ownerIsNeverZero
```

### CVL specification anatomy

The spec file (`formal-verification/audit_ledger.spec`) is organised as:

1. **Ghost variables** — shadow copies of critical storage values, updated via `hook`s.
2. **Hooks** — intercept `SSTORE` operations to keep ghosts in sync.
3. **Methods** — declarations of the functions to be verified.
4. **Rules** — individual property proofs using `assert` / `@withrevert`.
5. **Invariants** — global properties that must hold after every function call.

### Reading counterexamples

When Certora finds a violation, it produces a counterexample trace in the web UI:

1. Open the dashboard at `https://prover.certora.com`.
2. Click the failing rule → **Counterexample trace**.
3. Examine:
   - **`callValue`**: ETH sent to the call.
   - **`caller`**: `msg.sender` value.
   - **Storage slots**: values before and after the call.
4. Reproduce locally:
   ```bash
   certoraRun ... --rule <failing_rule> --smt_timeout 600 --debug
   ```
5. Fix the contract or add a tighter `require` to the rule's precondition.

### Common pitfalls

- **Vacuous rules**: a `require` that is never satisfiable makes a rule always
  pass trivially. Add `satisfy true;` at the end of a rule to check it is
  reachable.
- **Ghost desync**: if you add a new state-modifying function, add a hook or
  update the preserved block of every invariant that uses a related ghost.
- **Missing `requireInvariant`**: invariants are not automatically assumed for
  `preserved` blocks — list them explicitly.

---

## K-framework

### Setup

```bash
# Install K (requires Java 17+)
bash <(curl -s https://kframework.org/install)
export PATH="$HOME/.local/lib/kframework/bin:$PATH"

# Verify
kompile --version   # K >= 6.x
kprove  --version
```

### Compile the specification

```bash
cd formal-verification/k-spec

kompile audit_ledger-spec.k \
    --backend haskell \
    --main-module AUDIT-LEDGER-SPEC \
    --syntax-module AUDIT-LEDGER-SYNTAX \
    -o .kompiled-audit-ledger
```

This step takes 2–5 minutes the first time (it compiles the K semantics to a
Haskell interpreter).

### Run all proof claims

```bash
kprove audit_ledger-spec.k \
    --definition formal-verification/k-spec/.kompiled-audit-ledger \
    --spec-module AUDIT-LEDGER-SPEC
```

Expected output: `#Top` for every claim.

### Run a single claim

```bash
kprove audit_ledger-spec.k \
    --definition formal-verification/k-spec/.kompiled-audit-ledger \
    --spec-module AUDIT-LEDGER-SPEC \
    --claim SPEC-3    # Cap enforcement
```

### K specification structure

The K spec (`formal-verification/k-spec/audit_ledger-spec.k`) has four modules:

| Module | Purpose |
|--------|---------|
| `AUDIT-LEDGER-SYNTAX` | Sorts, productions, and token types |
| `AUDIT-LEDGER-CONFIG` | `configuration` cell declarations (contract storage) |
| `AUDIT-LEDGER-RULES` | Rewrite rules for every state transition |
| `AUDIT-LEDGER-SPEC` | Proof claims (`claim` declarations) |

### Interpreting a failed proof

When a claim fails K prints the **residual configuration** — the undischarged
part of the proof obligation. Key cells:

```
<k> #error(2) </k>          // unexpected error code
<totalEvents>  100 </totalEvents>
<globalMaxLogs> 100 </globalMaxLogs>
```

This shows the precondition `TOTAL <Int MAX` was not strong enough. Add it
explicitly to the claim's `requires` clause.

---

## How to Add New Properties

1. **Identify the property**: write a plain-English invariant (e.g., "the
   contract cannot log more than N events per ledger").

2. **Add to `properties.md`**: assign a `P-XX` ID, category, description,
   and initial status `🔲 Pending`.

3. **Write the CVL rule** in `audit_ledger.spec`:

   ```cvl
   rule myNewProperty(method f, env e) {
       uint32 before = someState(e);
       calldataarg args;
       f(e, args);
       uint32 after = someState(e);
       assert after <= before + MAX_DELTA,
           "myNewProperty: constraint violated";
   }
   ```

4. **Write the K claim** in `audit_ledger-spec.k`:

   ```k
   claim
       <audit-ledger>
           <k> myFunction(ARGS) => EXPECTED_RESULT </k>
           <someState> BEFORE => AFTER </someState>
           ...
       </audit-ledger>
       requires PRECONDITION
   ```

5. **Run both tools** and update the status column.

6. **Open a PR** with all three files changed together.

---

## CI Integration

Add `.github/workflows/formal-verification.yml`:

```yaml
name: Formal Verification

on:
  push:
    branches: [main, 'feat/**']
    paths:
      - 'src/lib.rs'
      - 'formal-verification/**'
  workflow_dispatch:

jobs:
  certora:
    runs-on: ubuntu-latest
    if: ${{ secrets.CERTORA_KEY != '' }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with: { python-version: '3.11' }
      - run: pip install certora-cli
      - run: |
          certoraRun bridge/evm/ZkVerifier.sol \
            --verify ZkVerifier:formal-verification/audit_ledger.spec \
            --solc solc8.24 \
            --msg "CI: AuditLedger formal verification (commit ${{ github.sha }})"
        env:
          CERTORAKEY: ${{ secrets.CERTORA_KEY }}

  k-proofs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install K
        run: bash <(curl -sL https://kframework.org/install)
      - name: Compile K spec
        run: |
          cd formal-verification/k-spec
          kompile audit_ledger-spec.k --backend haskell \
            --main-module AUDIT-LEDGER-SPEC --syntax-module AUDIT-LEDGER-SYNTAX \
            -o .kompiled
      - name: Run K proofs
        run: |
          cd formal-verification/k-spec
          kprove audit_ledger-spec.k --definition .kompiled \
            --spec-module AUDIT-LEDGER-SPEC
```

---

## References

- [Certora Verification Language docs](https://docs.certora.com)
- [K-framework tutorial](https://kframework.org/k-distribution/k-tutorial/)
- [formal-verification/README.md](../formal-verification/README.md) — quick-start
- [formal-verification/properties.md](../formal-verification/properties.md) — property catalog
- [AuditLedger contract](../src/lib.rs)
