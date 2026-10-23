# Formal Verification & State Machine Invariants (`src/confidential_metadata.rs`)

This specification defines the formal state machine, safety invariants, and fault-recovery
properties for the confidential event metadata module (`src/confidential_metadata.rs`, Issues #943, #922, #916, #914).

## 1. State Space Definition

Let the contract state of `ConfidentialEventLedger` be a tuple:

$$\Sigma = \langle A, K, E, s, N, T \rangle$$

where:
- $A \in \text{Address}$ is the administrative authority.
- $K: \text{Address} \rightharpoonup \text{EncryptionKeyRecord}$ maps participant addresses to registered asymmetric public keys and their lifecycle status (`active`, `registered_at`, `rotated_at`).
- $E: \mathbb{B}^{32} \rightharpoonup \text{ConfidentialMetadata}$ maps 32-byte event IDs to stored confidential envelopes (`submitter`, `event_type`, `encrypted_payload`, `policy`, `zk_proof_hash`, `key_id`, `created_at`).
- $s \in \mathbb{N}$ is the monotonic sequence counter `NextSeq` ($s \ge 1$).
- $N \in \mathbb{N}$ is the cumulative total events counter `TotalEvents`.
- $T \in \mathbb{N}$ is the current ledger timestamp.

---

## 2. Safety & Consistency Invariants

### `INV-CONF-SEQ` — Monotonic Sequence & Unique Event ID Invariant
$$\forall e_1, e_2 \in \text{dom}(E): e_1 \neq e_2 \implies \text{seq}(e_1) \neq \text{seq}(e_2)$$
$$N = |E| \land s = N + 1$$
For every newly accepted confidential event at sequence $s$ and timestamp $T$:
$$e_{\text{id}} = \text{SHA-256}(\text{submitter}.\text{to\_xdr}() \parallel \text{to\_be\_bytes}(s) \parallel \text{to\_be\_bytes}(T))$$
$$s' = s + 1 \land N' = N + 1 \land E' = E \cup \{ e_{\text{id}} \mapsto \text{record} \}$$

### `INV-CONF-KEY` — Key Registration & Lifecycle State Machine Invariant
Key records obey deterministic state transitions:
$$\text{State}(K(u)) \in \{ \text{Unregistered}, \text{Active}, \text{Rotated}, \text{Revoked} \}$$
- Transition $\text{Unregistered} \xrightarrow{\text{register}} \text{Active}$: $\text{registered\_at} = T \land \text{rotated\_at} = 0 \land \text{active} = \text{true}$.
- Transition $\text{Active} \lor \text{Rotated} \xrightarrow{\text{rotate}} \text{Rotated}$: $\text{rotated\_at} = T \land \text{active} = \text{true} \land \text{public\_key} = \text{new\_key}$.
- Transition $\text{Active} \lor \text{Rotated} \xrightarrow{\text{revoke}} \text{Revoked}$: $\text{active} = \text{false}$.
- A revoked key cannot be rotated or un-revoked without re-registration.

### `INV-CONF-POLICY` — Access Control Policy Soundness
For any query $\text{check\_access}(e_{\text{id}}, r)$ or $\text{decrypt\_event\_metadata}(e_{\text{id}}, r)$:
$$\text{access\_granted}(E(e_{\text{id}}), r) \iff \text{eval}(E(e_{\text{id}}).\text{policy}, r)$$
where:
$$\text{eval}(\text{Public}, r) = \text{true}$$
$$\text{eval}(\text{SubmitterOnly}, r) = (r = E(e_{\text{id}}).\text{submitter})$$
$$\text{eval}(\text{RoleBased}(\text{role}), r) = (r = E(e_{\text{id}}).\text{submitter} \lor r = A \lor \text{has\_role}(r, \text{role}))$$
$$\text{eval}(\text{CustomAllowlist}(L), r) = (r \in L \lor r = E(e_{\text{id}}).\text{submitter})$$
$$\text{eval}(\text{ThresholdDecryption}(k, P), r) = (r \in P \lor r = E(e_{\text{id}}).\text{submitter})$$

Unauthorized decryption attempts revert with `ConfidentialError::AccessDenied` (code 3) and emit an audit trail event tagged `conf_audit` with `outcome = "denied"`.

### `INV-CONF-BOUNDS` — Rent & Payload Resource Limits Invariant
To prevent unbounded storage rent consumption and CPU instruction exhaustion:
- $|E(e).\text{encrypted\_payload}| \le \text{MAX\_ENCRYPTED\_PAYLOAD\_BYTES} = 65,536$
- $16 \le |\text{zk\_proof}| \le \text{MAX\_ZK\_PROOF\_BYTES} = 1,024$
- $|L| \le \text{MAX\_POLICY\_PARTIES} = 128$ for $\text{CustomAllowlist}$
- $0 < k \le |P| \le \text{MAX\_POLICY\_PARTIES} = 128$ for $\text{ThresholdDecryption}$
- Submitted proof bytes are verified on-chain, but only the 32-byte digest $\text{SHA-256}(\text{zk\_proof})$ is archived in storage, bounding perpetual rent footprint.

Any violation immediately reverts with `PayloadTooLarge`, `InvalidPolicy`, `PolicyTooLarge`, or `InvalidZKProof`, ensuring $\Sigma' = \Sigma$.

### `INV-CONF-AGGREGATION` — Privacy-Preserving Analytics Invariance
Given a batch of event IDs $B = [id_0, id_1, \dots, id_{m-1}]$ with $m \le \text{MAX\_AGGREGATE\_BATCH} = 256$:
$$\text{aggregate\_confidential\_events}(B).\text{total\_events} = m$$
$$\text{encrypted\_accumulator} = \bigoplus_{i=0}^{m-1} E(id_i).\text{encrypted\_payload}$$
$$\text{zk\_aggregation\_proof} = \text{SHA-256}(\text{encrypted\_accumulator})$$
$$\text{aggregated\_at} = T$$
No individual event plaintexts or decryption keys are exposed during accumulation.
