# ADR-016: Contract Event Feature Flags and Progressive Delivery

## Status
Accepted

## Context
Deploying new contract event schemas, topics, or indexer pipelines directly to 100% of network traffic creates systemic risk of ledger congestion, deserialization crashes, or downstream breaking changes. Furthermore, operations teams require instantaneous mitigation controls when zero-day vulnerabilities or misbehaving event types are discovered.

## Decision
We implement a unified Feature Flagging, Progressive Delivery, Experimentation, and Emergency Kill Switch architecture:

1. **On-Chain Feature Flag Registry (`src/event_feature_flags.rs`)**:
   - Stores flag metadata, types (`Boolean`, `PercentageRollout`, `Multivariate`, `KillSwitch`), and statuses (`Active`, `Inactive`, `Killed`, `Graduated`).
   - Supports deterministic, hash-based percentage bucketing for canary rollouts and A/B/n experiments.
   - Emits tamper-evident audit events (`flag_created`, `canary_advanced`, `kill_switch_triggered`).

2. **Progressive Delivery & Canary Engine (`packages/feature-flags`)**:
   - Provides OpenFeature & LaunchDarkly compatible SDK providers.
   - Implements automated canary progression (`current_percentage + step_percentage`) governed by error-budget and latency SLAs.
   - Executes automatic rollbacks if error thresholds exceed configured basis points (`error_threshold_bps`).

3. **Emergency Kill Switches**:
   - Instantaneous, single-transaction shutdown of specific event emissions or contract features.
   - Overrides all progressive rollouts and routes traffic immediately to safe fallbacks.
   - Triggers automated alerts and logs the initiator and reason on-chain.

4. **Multi-Variate Experimentation**:
   - Deterministic variant resolution based on user ID / caller address.
   - Conversion tracking and statistical winner declaration.

## Consequences

### Positive
- Safely test and rollout new event versions on live networks with granular percentages (e.g. 5% -> 25% -> 50% -> 100%).
- Sub-second incident mitigation via emergency kill switches without contract redeployment.
- Compatible with LaunchDarkly, OpenFeature, Flipt, and Unleash ecosystems.

### Trade-offs
- Small gas overhead for on-chain flag evaluation when executed directly within smart contract invocations.
