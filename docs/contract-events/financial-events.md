# Financial & Asset Lifecycle Events

This document details all financial regulation, Real-World Asset (RWA) tokenization, stablecoin reserve backing, and CBDC events.

---

## 1. Stablecoin Reserve Attestations (`src/stablecoin_reserves.rs`)

### `reserve_attestation`
- **Topic**: `Symbol("stablecoin_reserves")`, `Symbol("reserve_attestation")`
- **Regulatory Framework**: EU MiCA Article 36(1)
- **Payload Schema**:
  ```json
  {
    "asset_id": "EURC_VAULT",
    "reserve_ratio": 1.05,
    "total_supply": "1000000000000",
    "total_reserves": "1050000000000",
    "auditor_signature": "0xsig_verified_pki"
  }
  ```

---

## 2. Real-World Asset (RWA) Operations (`src/rwa_asset.rs`)

### `rwa_transferred` (FATF Travel Rule / FinCEN CTR)
- **Topic**: `Symbol("rwa_asset")`, `Symbol("transfer_settled")`
- **Payload Schema**:
  ```json
  {
    "asset_id": "RWA-REAL-ESTATE-99",
    "amount_usd": 125000,
    "originator": "GAK4K6K4Z67A5M7X5SLLM",
    "beneficiary": "GB2X99KKLM...",
    "travel_rule_compliant": true,
    "ctr_reported": true
  }
  ```

---

## 3. CBDC Logging & Interoperability (`src/cbdc_logging.rs`)

### `settlement_finalized`
- **Topic**: `Symbol("cbdc_logging")`, `Symbol("settlement_finalized")`
- **Payload Schema**:
  ```json
  {
    "batch_id": "CBDC-BATCH-4412",
    "currency_code": "EUR",
    "total_volume": "45000000.00",
    "merkle_root": "0x5b3a1..."
  }
  ```
