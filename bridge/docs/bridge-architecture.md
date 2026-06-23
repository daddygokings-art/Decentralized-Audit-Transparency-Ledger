# Cross-Chain Bridge Architecture

## Overview

The AuditLedger cross-chain bridge enables events logged on the Stellar network to be independently verified on EVM-compatible chains (Ethereum, Polygon, etc.) without redeploying the contract. The design follows a **light-client + relay** pattern.

```
Stellar (Soroban)            Relayer (off-chain)            EVM Chain
─────────────────            ───────────────────            ─────────
 AuditLedger ──── events ───► poll / subscribe ───► proof ──► Verifier.sol
```

---

## Components

### 1. Stellar Relayer (`bridge/relayer/index.ts`)

An off-chain Node.js service that:
1. Polls (or subscribes via WebSocket) to `AuditLedger` events on Stellar testnet.
2. Fetches the raw **transaction envelope** and **ledger close meta** for each event.
3. Serializes the **event inclusion proof** — a Merkle path from the event hash to the ledger's transaction hash tree.
4. Submits the proof + event data to the EVM `Verifier` contract.

### 2. EVM Verifier (`bridge/evm/Verifier.sol`)

A Solidity contract that:
- Accepts `(proof bytes, eventData bytes)` and returns `bool`.
- Maintains a **trusted validator set** (multi-sig or a whitelisted relay address for testnet).
- For production: verifies a Stellar ledger header signed by a quorum of Stellar validators.
- Records verified events in `verifiedEvents[eventHash]` to prevent replay.

---

## Trust Model

| Component | Trusted? | How trust is established |
|-----------|----------|--------------------------|
| Stellar consensus | Trusted source of truth | Ledger headers signed by validator quorum |
| Relayer | **Not trusted** | Proof is cryptographically verified on-chain |
| Verifier contract | Trusted execution | Deployed on public EVM chain; source-verified |
| Validator public keys | Trusted bootstrap | Hard-coded in Verifier at deploy time |

The relayer is treated as an **untrusted courier** — it can omit events or submit stale proofs, but it cannot forge a valid proof without compromising ≥2/3 of the Stellar validator set.

**Testnet simplification:** For the testnet implementation, the Verifier accepts proofs signed by a single trusted relayer key (ECDSA). Production deployments should require a Stellar ledger header verified against the full SCP quorum.

---

## Proof Format

```
EventProof {
  ledgerSeq:    uint64     // Stellar ledger sequence
  txHash:       bytes32    // Transaction hash on Stellar
  eventIndex:   uint32     // Index within the transaction's events
  eventHash:    bytes32    // keccak256(abi.encode(eventData))
  signature:    bytes65    // ECDSA signature over keccak256(ledgerSeq ++ txHash ++ eventHash)
}
```

---

## Security Considerations

1. **Replay protection** — `verifiedEvents[eventHash]` mapping prevents the same event from being verified twice.
2. **Validator key rotation** — Verifier exposes `updateTrustedSigner(address)` restricted to `owner`.
3. **Proof staleness** — Verifier rejects proofs where `ledgerSeq` is more than `MAX_LEDGER_AGE` (default 1000 ledgers ≈ 80 min) behind the latest accepted ledger.
4. **Gas efficiency** — A single `verifyEvent` call costs ~50k gas (one ECDSA recovery + two SSTORE for new events).
5. **Event ordering** — Cross-chain ordering is not guaranteed; consumers should use `ledgerSeq + eventIndex` as the canonical sort key.

---

## Sequence: Happy Path

```
1.  Event logged on Stellar  →  AuditLedger emits event at index N
2.  Relayer polls Stellar RPC →  fetches ledger close meta
3.  Relayer builds proof      →  signs EventProof with relayer ECDSA key
4.  Relayer calls Verifier    →  verifyEvent(proof, eventData)
5.  Verifier recovers signer  →  checks against trustedSigner
6.  Verifier stores result    →  verifiedEvents[eventHash] = true
7.  EVM consumers query       →  Verifier.isVerified(eventHash) → true
```

## Sequence: Tampered Proof Rejected

```
1.  Attacker forges proof     →  changes eventHash or signature
2.  Verifier recovers signer  →  address ≠ trustedSigner
3.  Verifier reverts          →  InvalidProof()
```

---

## Gas Cost Estimates

| Operation | Gas |
|-----------|-----|
| `verifyEvent` (new event) | ~50,000 |
| `verifyEvent` (already verified) | ~25,000 |
| `isVerified` (view) | ~2,000 |

---

## Future Work

- Replace single-signer model with a threshold signature scheme (e.g., BLS aggregation) verifying the full Stellar validator quorum.
- Add a Solana program verifier for Stellar ↔ Solana bridge.
- Implement a Merkle audit proof (not just ECDSA) for complete trustlessness.
