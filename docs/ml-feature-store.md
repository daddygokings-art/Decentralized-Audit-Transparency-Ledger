# Contract Event ML Feature Store (#524)

This module implements an enterprise-grade Machine Learning Feature Store for computing, storing, versioning, and serving event-derived features for fraud detection, risk scoring, and anomaly identification models.

---

## Core Components

1. **Feature Computation Engine (`feature_store/core/`)**:
   - Computes real-time streaming and batch sliding-window aggregations (`1h`, `24h`, `7d`).
   - Generates velocity metrics, burst ratios, and entropy scores.
   - Declarative `FeatureView` and `FeatureRegistry` abstractions with semantic versioning.

2. **Dual-Store Architecture**:
   - **Online Store (`feature_store/online_store/`)**: Low-latency (< 5ms) Redis-backed serving layer for real-time model inference via HTTP/gRPC.
   - **Offline Store (`feature_store/offline_store/`)**: Parquet/Delta Lake store supporting point-in-time correct (AS-OF) joins to prevent target leakage during model training.

3. **Feature Monitoring & Drift Detection (`feature_store/monitoring/`)**:
   - Automated Population Stability Index (PSI) and Kolmogorov-Smirnov statistical tests.
   - Missing value, outlier, and freshness monitoring.

4. **Soroban On-Chain Attestation (`src/ml_feature_store.rs`)**:
   - `register_feature_view`: Registers feature schemas and version commitments.
   - `record_feature_attestation`: Stores cryptographic feature hashes on-chain for tamper-evident AI audits.
   - `record_drift_alert`: Emits on-chain alerts when feature distributions breach safety thresholds.
