# Formal Verification — AuditLedger

This directory contains formal specifications and verification tooling for the
`AuditLedger` Soroban smart contract.

---

## Directory layout

```
formal-verification/
├── audit_ledger.spec       Certora CVL property rules
├── properties.md           Human-readable property catalog with status
├── README.md               This file
└── k-spec/
    └── audit_ledger-spec.k K-framework semantic specification
```

---

## Overview

Two complementary tools are used:

| Tool | What it proves |
|------|---------------|
| **Certora Prover** | Verifies safety/liveness properties via symbolic execution of the compiled bytecode (EVM harness). Exhaustive over all inputs and state space within the defined configuration. |
| **K-framework** | Models the operational semantics of state transitions as rewrite rules and verifies proof claims via symbolic execution and SMT solving (Z3). |

The full property list with verification status is in [`properties.md`](properties.md).

---

## Certora Prover

### Prerequisites

```bash
pip install certora-cli
export CERTORAKEY="<your API key>"   # https://www.certora.com
```

### Build the EVM harness

Because AuditLedger is a Soroban (WASM) contract, Certora verification uses a
thin Solidity harness that exposes the same interface. The harness lives in
`bridge/evm/` (see `ZkVerifier.sol` and `CrossChainSync.sol` for patterns).

For local iteration without a full harness, Certora also accepts a JSON ABI
configuration:

```bash
certoraRun --protocol soroban \
           AuditLedger \
           --verify AuditLedger:formal-verification/audit_ledger.spec \
           --msg "AuditLedger safety props"
```

### Running specific rules

```bash
# Single rule
certoraRun AuditLedger.sol --verify AuditLedger:formal-verification/audit_ledger.spec \
           --rule eventImmutability

# All invariants only
certoraRun AuditLedger.sol --verify AuditLedger:formal-verification/audit_ledger.spec \
           --rule globalCapRespected contractMustBeInitialized ownerIsNeverZero

# Full suite (takes ~10 min)
certoraRun AuditLedger.sol --verify AuditLedger:formal-verification/audit_ledger.spec
```

### Reading results

The Certora dashboard shows each rule as **PASS**, **FAIL (counterexample)**,
or **TIMEOUT**. For counterexamples:

1. Expand the failing rule in the UI.
2. Click **Counterexample trace** to see the sequence of calls and storage
   values that violate the property.
3. Check the `callValue`, `caller`, and storage slots in the trace.
4. Reproduce locally:
   ```bash
   certoraRun ... --rule <failing_rule> --smt_timeout 300
   ```
5. Fix the contract or tighten the spec and re-run.

### Invariant violations

CVL invariants are checked at the end of every function call. If an invariant
fires, the tool reports the function call that first broke it. Typical causes:

- Missing `require` in a state-modifying function.
- Ghost variable not updated in a hook.
- Incorrect preserved block missing `requireInvariant`.

---

## K-Framework

### Prerequisites

```bash
# Install K
bash <(curl https://kframework.org/install)
export PATH="$HOME/.local/lib/kframework/bin:$PATH"

# Verify installation
kompile --version   # should print K version >= 6.x
kprove  --version
```

### Compile the specification

```bash
cd formal-verification/k-spec

# Compile the AUDIT-LEDGER-SPEC module (haskell backend for proofs)
kompile audit_ledger-spec.k \
        --backend haskell \
        --main-module AUDIT-LEDGER-SPEC \
        --syntax-module AUDIT-LEDGER-SYNTAX \
        -o .kompiled-audit-ledger
```

### Run all proof claims

```bash
kprove audit_ledger-spec.k \
       --definition .kompiled-audit-ledger \
       --spec-module AUDIT-LEDGER-SPEC
```

Expected output: `#Top` for each claim (all proofs succeed).

### Run a single claim

```bash
kprove audit_ledger-spec.k \
       --definition .kompiled-audit-ledger \
       --spec-module AUDIT-LEDGER-SPEC \
       --claim SPEC-3          # Cap enforcement
```

### Interpreting counterexamples

When a claim fails K prints the residual configuration — the part of the state
that could not be reduced to `#Top`. Key cells to inspect:

- `<k>`: remaining computation (the stuck term).
- `<totalEvents>`: check against `<globalMaxLogs>`.
- `<initialized>`, `<owner>`, `<paused>`: check preconditions.

Example: a failing `SPEC-2` might print:

```
<k> #error(2) </k>
<totalEvents>   100 </totalEvents>
<globalMaxLogs> 100 </globalMaxLogs>
```

meaning the test provided `TOTAL == MAX`, violating the `requires TOTAL <Int MAX`
precondition of the claim.

---

## CI Integration

The formal verification workflow is separated from the main `cargo test` CI
because Certora requires a cloud API key and K proofs are resource-intensive.

Add this to `.github/workflows/formal-verification.yml`:

```yaml
name: Formal Verification

on:
  push:
    paths:
      - 'formal-verification/**'
      - 'src/lib.rs'
  workflow_dispatch:

jobs:
  certora:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with: { python-version: '3.11' }
      - run: pip install certora-cli
      - run: |
          certoraRun bridge/evm/ZkVerifier.sol \
            --verify ZkVerifier:formal-verification/audit_ledger.spec \
            --msg "CI: AuditLedger formal verification"
        env:
          CERTORAKEY: ${{ secrets.CERTORA_KEY }}

  k-proofs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install K
        run: bash <(curl -s https://kframework.org/install)
      - name: Compile spec
        run: |
          cd formal-verification/k-spec
          kompile audit_ledger-spec.k --backend haskell \
            --main-module AUDIT-LEDGER-SPEC \
            --syntax-module AUDIT-LEDGER-SYNTAX \
            -o .kompiled
      - name: Run proofs
        run: |
          cd formal-verification/k-spec
          kprove audit_ledger-spec.k \
            --definition .kompiled \
            --spec-module AUDIT-LEDGER-SPEC
```

---

## Adding New Properties

1. Describe the property in [`properties.md`](properties.md) and assign it the
   next `P-XX` identifier.
2. Add a CVL `rule` or `invariant` to `audit_ledger.spec`.
3. Add a K `claim` to `k-spec/audit_ledger-spec.k` (in `AUDIT-LEDGER-SPEC`).
4. Run both verification tools and update the **Status** column.
5. Commit all three files together in the same PR.

### Tips

- Write the plainest possible K claim first (happy path), then add failure cases.
- CVL `invariants` are cheaper to write than rules for global safety conditions.
- Use `requireInvariant` inside `preserved` blocks to avoid spurious violations.
- For Soroban-specific types (`BytesN<32>`, `Symbol`) use the `bytes32` and
  `uint256` EVM proxies in the CVL harness.

---

## References

- [Certora CVL Documentation](https://docs.certora.com/en/latest/docs/cvl/index.html)
- [K-Framework Documentation](https://kframework.org/k-distribution/k-tutorial/)
- [Soroban SDK Reference](https://docs.rs/soroban-sdk/)
- [`properties.md`](properties.md) — full property catalog
- [`docs/FORMAL_VERIFICATION.md`](../docs/FORMAL_VERIFICATION.md) — architecture overview
