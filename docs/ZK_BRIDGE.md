# ZK Bridge — Zero-Knowledge Proof Verification for AuditLedger Events

Issue #374 — This document describes the ZK proof system used to verify
AuditLedger event inclusion on EVM chains without revealing full event data.

---

## Overview

The ZK bridge allows any party to prove that a specific event was logged in the
Stellar/Soroban AuditLedger without disclosing the event's metadata. The proof
is verified by `ZkVerifier.sol` on the EVM side in a single transaction.

**Proof system**: Groth16 over BN254 (alt_bn128 / EIP-197).

Benefits:
- Privacy: metadata is kept off-chain; only the event hash (commitment) is public.
- Succinctness: constant-size proofs (256 bytes) regardless of event count.
- Gas efficiency: BN254 pairing precompile at 0x08 costs ~45,000 gas.
- Trustless bridging: no signatures from centralised oracles needed.

---

## Circuit Design — AuditLedgerInclusion

The inclusion proof circuit proves:

> "I know an event E such that:
>  1. sha256(E) == eventHash (a public input), AND
>  2. E is a leaf in the Merkle tree with root merkleRoot (a public input)."

### Public inputs

| Index | Name | Type | Description |
|-------|------|------|-------------|
| 0 | `eventHash` | BN254 field element | sha256 of the serialised Event struct |
| 1 | `merkleRoot` | BN254 field element | Merkle root of all logged events |

### Private inputs (witness)

| Name | Type | Description |
|------|------|-------------|
| `eventLeaf` | bytes[256] | Serialised Event struct (without metadata) |
| `merklePathNodes` | bytes32[N] | Sibling hashes along the Merkle path |
| `merklePathIndices` | bool[N] | Left/right bit at each Merkle level |

### Circuit file

```circom
pragma circom 2.0.0;

include "circomlib/circuits/sha256/sha256.circom";
include "circomlib/circuits/merkleProof.circom";

template AuditLedgerInclusion(MERKLE_DEPTH) {
    // Public
    signal input  eventHash;
    signal input  merkleRoot;

    // Private
    signal input  eventLeaf[256];          // 256 bits = 32 bytes
    signal input  merklePathNodes[MERKLE_DEPTH];
    signal input  merklePathIndices[MERKLE_DEPTH];

    // 1. Verify eventHash == sha256(eventLeaf)
    component hasher = Sha256(256);
    for (var i = 0; i < 256; i++) {
        hasher.in[i] <== eventLeaf[i];
    }
    // SHA-256 output is 256 bits; pack to field element.
    var computedHash = 0;
    for (var i = 0; i < 256; i++) {
        computedHash += hasher.out[i] * (1 << i);
    }
    eventHash === computedHash;

    // 2. Verify Merkle path
    component merkle = MerkleProof(MERKLE_DEPTH);
    merkle.leaf        <== eventHash;
    merkle.root        <== merkleRoot;
    for (var i = 0; i < MERKLE_DEPTH; i++) {
        merkle.pathElements[i] <== merklePathNodes[i];
        merkle.pathIndices[i]  <== merklePathIndices[i];
    }
}

component main { public [eventHash, merkleRoot] } =
    AuditLedgerInclusion(20);   // supports up to 2^20 = 1M events
```

### Merkle tree construction

Events are stored with SHA-256 leaf hashes:

```
leaf_i = sha256(event_i.event_hash || event_i.index_as_u32_le)
```

The tree is balanced; odd layers duplicate the last leaf. The root is updated
after every `logEvent` call (off-chain; the on-chain contract stores per-event
`event_hash` and `prev_hash` for the hash chain, not the full Merkle root).

The Merkle root is computed off-chain by the relayer and committed to the EVM
chain via `CrossChainSync.recordCheckpoint`.

---

## Trusted Setup

Groth16 requires a one-time **trusted setup** ceremony (powers-of-tau + circuit-
specific phase 2). The ceremony is irreversible: if any participant's randomness
is leaked, the system is compromised. To minimise trust, use a multi-party
computation (MPC) ceremony.

### Steps

1. **Phase 1 (universal)** — Powers of Tau. Download an existing ceremony
   output or run your own:
   ```bash
   snarkjs powersoftau new bn128 20 pot20_0.ptau -v
   snarkjs powersoftau contribute pot20_0.ptau pot20_1.ptau --name "Contributor 1" -v
   snarkjs powersoftau prepare phase2 pot20_1.ptau pot20_final.ptau -v
   ```

2. **Phase 2 (circuit-specific)** — Circuit-specific randomness:
   ```bash
   snarkjs groth16 setup audit_ledger_inclusion.r1cs pot20_final.ptau circuit_0.zkey
   snarkjs zkey contribute circuit_0.zkey circuit_1.zkey --name "Contributor 1" -v
   snarkjs zkey beacon circuit_1.zkey circuit_final.zkey \
       0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 10 -n "Final beacon"
   ```

3. **Export verification key**:
   ```bash
   snarkjs zkey export verificationkey circuit_final.zkey vk.json
   ```

4. **Deploy ZkVerifier.sol** with the values from `vk.json`.

5. **Publish** the `circuit_final.zkey`, `pot20_final.ptau`, and
   `audit_ledger_inclusion.r1cs` for public verification.

### Security considerations

- Use at least 5 independent contributors for the MPC ceremony.
- Verify that all contributors' randomness contributions are included in the
  transcript.
- Publish all ceremony artifacts (ptau files, contribution hashes).
- Consider using the Hermez/Iden3 publicly audited Phase 1 ceremony output.

---

## Proof Generation Workflow

```
Soroban Event Log         Relayer (TypeScript)         EVM Chain
──────────────────        ─────────────────────        ─────────────────
  logEvent(...)    ──→   ZkProofGenerator               ZkVerifier.sol
                           .generateEventInclusionProof()
                           (computes Merkle proof)
                           .generateBatchProof()      ──→ verifyEventZkProof()
                                                         verifyBatchProof()
                                                         verifyEventInclusion()
```

### Step-by-step

1. **Fetch event from Soroban RPC**:
   ```typescript
   const event = await sorobanRpc.getEvent(eventId);
   ```

2. **Build Merkle tree**:
   ```typescript
   const leaves = allEvents.map(e => e.eventHash);
   const root   = computeMerkleRoot(leaves);
   const proof  = buildMerkleProof(leaves, event.index);
   ```

3. **Generate ZK proof**:
   ```typescript
   const gen = new ZkProofGenerator({ mockMode: false, wasmPath, zkeyPath });
   const zkProof = await gen.generateEventInclusionProof(
       event.eventHash, proof, root
   );
   ```

4. **Submit to EVM**:
   ```solidity
   bool valid = zkVerifier.verifyEventInclusion(eventHash, zkProof.proofHex);
   ```

---

## Gas Cost Analysis

| Operation | Gas (approx.) | Notes |
|-----------|---------------|-------|
| `verifyEventZkProof` (single) | ~250,000 | 4 pairings + 2 G1 mul |
| `verifyBatchProof` (10 proofs) | ~2,400,000 | 10 × single + overhead |
| `verifyEventInclusion` | ~255,000 | single + event emission |
| ecAdd precompile (0x06) | 150 | per call |
| ecMul precompile (0x07) | 6,000 | per call |
| ecPairing (0x08, 4 pairs) | 45,000 + 34,000×4 | = 181,000 per call |

**Optimisation strategies**:
- Use `verifyBatchProof` to amortise per-tx overhead.
- Cache the linear combination (`vk_x`) off-chain when public inputs are
  unchanged between proofs.
- Consider Plonk or STARKs for batches > 100 proofs (no trusted setup, better
  amortisation).

---

## Integration Guide

### Deploy ZkVerifier

```typescript
import { ethers } from 'ethers';
import vk from './vk.json';

const factory = new ethers.ContractFactory(abi, bytecode, signer);
const contract = await factory.deploy(
    vk.vk_alpha_1[0], vk.vk_alpha_1[1],
    [vk.vk_beta_2[0][0], vk.vk_beta_2[0][1]],
    [vk.vk_beta_2[1][0], vk.vk_beta_2[1][1]],
    [vk.vk_gamma_2[0][0], vk.vk_gamma_2[0][1]],
    [vk.vk_gamma_2[1][0], vk.vk_gamma_2[1][1]],
    [vk.vk_delta_2[0][0], vk.vk_delta_2[0][1]],
    [vk.vk_delta_2[1][0], vk.vk_delta_2[1][1]],
    vk.IC.map(p => p[0]),
    vk.IC.map(p => p[1]),
);
```

### Verify a proof on-chain

```typescript
const valid = await zkVerifier.verifyEventInclusion(
    eventHash,    // bytes32
    proof.proofHex, // bytes (256 bytes)
);
```

### Listen for ProofVerified events

```typescript
zkVerifier.on('ProofVerified', (eventHash, verifier, timestamp, success) => {
    console.log(`Proof for ${eventHash}: ${success ? 'VALID' : 'INVALID'}`);
});
```

---

## References

- [`bridge/evm/ZkVerifier.sol`](../bridge/evm/ZkVerifier.sol)
- [`bridge/relayer/zkProofGenerator.ts`](../bridge/relayer/zkProofGenerator.ts)
- [snarkjs documentation](https://github.com/iden3/snarkjs)
- [circom language reference](https://docs.circom.io)
- [EIP-197: BN254 precompiles](https://eips.ethereum.org/EIPS/eip-197)
- [Groth16 original paper](https://eprint.iacr.org/2016/260)
