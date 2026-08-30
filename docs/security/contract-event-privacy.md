# Contract Event Privacy

The `contract_event_privacy` module provides auditable control-plane records for privacy-sensitive event processing. It does not put plaintext event payloads on-chain and it does not treat a registration as proof that an external cryptographic system is sound.

## Breach notification workflow

1. The detector calls `detect_breach`, which records the affected event IDs and calculates a 72-hour `notification_deadline` from the ledger timestamp.
2. An authorized owner records the investigation with `assess_breach`.
3. The notification worker sends the configured authority notice and records the delivery time with `notify_authority`.
4. The worker communicates with affected subjects through the configured out-of-band channel and records completion with `notify_subjects`.
5. After containment, the owner records corrective actions and lessons learned with `complete_review`.

The contract is the immutable workflow evidence. Delivery itself remains an off-chain responsibility and should use an idempotent queue, durable templates, retry policy, and an escalation alert when the deadline is approaching. The timestamp fields make late notifications and incomplete workflows queryable.

## Privacy-enhancing technologies

`register_pet` records the selected technique, intended use case, and benchmark evidence. Recommended evaluation dimensions are:

| Technique | Suitable event use case | Benchmark evidence |
| --- | --- | --- |
| Differential privacy | Aggregate dashboards | Privacy budget, error, and utility |
| Federated learning | Shared anomaly models | Accuracy, communication, and poisoning resistance |
| Homomorphic encryption | Computation over encrypted fields | Runtime, ciphertext expansion, and supported operations |
| Secure MPC | Joint checks without a shared plaintext | Participant count, rounds, and failure recovery |
| Zero-knowledge proofs | Prove predicates or compliance | Proving and verification time, proof size |
| Synthetic data | Development and test fixtures | Disclosure risk and distribution similarity |

PET metadata is descriptive evidence. Keys, circuits, model artifacts, and encrypted data must be managed by the deployment and must never be placed in contract storage.

## Confidential computing

`record_tee_attestation` binds an event to a platform name, enclave measurement, attestation evidence, and sealed-output hash. The verifier must validate the vendor quote or document, measurement policy, freshness, signer, and revocation status before recording it. Supported deployment profiles include Intel SGX, AMD SEV, and AWS Nitro; the contract intentionally accepts the platform as metadata so deployments can use their own verifier.

Sealed output hashes provide integrity for exported results, not confidentiality by themselves. Operators must keep sealing keys inside the TEE or an approved key-management service and rotate them according to the deployment policy.

## Selective disclosure

`set_disclosure_policy` stores a commitment, an allow-list of fields, an authorized verifier, and an expiry. A disclosure service should produce a proof against the commitment, reject fields outside the allow-list, check the policy expiry and verifier identity, and retain the proof and verification result in its audit system. Zero-knowledge proof verification belongs in the chosen proof-system verifier; this module records the policy boundary and does not claim to implement a zk-SNARK or zk-STARK verifier.

## Security expectations

- Store opaque identifiers and hashes on-chain; encrypt sensitive descriptions before submission.
- Restrict owner and verifier keys with separate roles and monitored signing policies.
- Alert on unassessed breaches, missed deadlines, repeated notification attempts, and expired disclosure policies.
- Include the four workflow stages and post-incident review in compliance exports.