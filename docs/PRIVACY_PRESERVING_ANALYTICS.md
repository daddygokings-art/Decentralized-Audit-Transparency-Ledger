# Contract Event Privacy-Preserving Analytics

## Overview

This module provides privacy-enhancing technologies (PETs) for sensitive contract events on the Decentralized Audit Transparency Ledger, combining:
- **Differential Privacy (DP)**
- **Federated Learning (FL)**
- **Secure Multi-Party Computation (SMPC)**
- **Homomorphic Encryption (HE)**

## Architectural Pillars

### 1. Differential Privacy (DP)
- **Laplace & Gaussian Mechanisms**: Injects calibrated noise ($Lap(\Delta f / \epsilon)$) scaled to query global sensitivity.
- **Privacy Loss Accounting**: Strict $(\epsilon, \delta)$ budget tracking anchored on the Soroban smart contract (`src/privacy_preserving_analytics.rs`) preventing reconstruction attacks.

### 2. Federated Learning (FL)
- **Decentralized Model Training**: Relayer nodes compute gradients locally on private audit event datasets.
- **FedAvg / FedProx Aggregation**: On-chain gradient hash commitments with weighted federated averaging.

### 3. Secure Multi-Party Computation (SMPC)
- **Shamir's Secret Sharing**: $(k, n)$-threshold polynomial sharing over finite field $\mathbb{F}_{2^{61}-1}$.
- **Additive Secret Sharing**: Enables distributed summation of audit metrics across nodes without exposing individual participants' private data.

### 4. Homomorphic Encryption (HE)
- **Paillier Cryptosystem**: Additively homomorphic ciphertexts where $E(m_1) \cdot E(m_2) \pmod{n^2} = E(m_1 + m_2 \pmod n)$.
- **Scalar Multiplication**: $E(m)^k \pmod{n^2} = E(k \cdot m \pmod n)$ for privacy-preserving volume and weighted score calculations.

## API Endpoints

- `POST /api/v1/privacy/dp/query`: Execute differential privacy count/sum queries.
- `GET /api/v1/privacy/dp/budget`: Check remaining epsilon/delta budget.
- `POST /api/v1/privacy/fl/rounds`: Initialize federated learning rounds.
- `POST /api/v1/privacy/smpc/split`: Split secret into additive shares.
- `POST /api/v1/privacy/he/aggregate`: Perform homomorphic addition on ciphertexts.
- `GET /api/v1/privacy/health`: Subsystem health check.
