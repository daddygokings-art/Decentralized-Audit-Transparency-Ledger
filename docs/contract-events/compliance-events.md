# Compliance & Regulatory Events

This document details all regulatory compliance events emitted across Anti-Corruption, Export Controls, Trade Compliance, and Data Retention modules.

---

## 1. Anti-Corruption Events (`src/anti_corruption.rs`)

### `incident_reported`
- **Topic**: `Symbol("anti_corruption")`, `Symbol("incident_reported")`
- **Payload Schema**:
  ```json
  {
    "incident_id": "INC-2026-004",
    "reporter_hash": "0x892a1...",
    "severity": "CRITICAL",
    "assigned_officer": "GAK4K6K4Z67A5M7X5SLLM",
    "timestamp": 1756483200
  }
  ```

### `whistleblower_submitted`
- **Topic**: `Symbol("anti_corruption")`, `Symbol("whistleblower_submitted")`
- **Payload Schema**:
  ```json
  {
    "report_id": "WB-8812",
    "encrypted_identity": "0xenc_rsa4096_...",
    "is_anonymous": true,
    "evidence_hash": "0xb3a812..."
  }
  ```

---

## 2. Export Controls Events (`src/export_controls.rs`)

### `customs_declaration`
- **Topic**: `Symbol("export_controls")`, `Symbol("customs_declaration")`
- **Payload Schema**:
  ```json
  {
    "declaration_id": "DEC-US-7712",
    "screening_status": "CLEAR",
    "is_dual_use": true,
    "license_number": "DOC-BIS-2026-9812",
    "destination_country": "DE"
  }
  ```

### `shipment_authorized`
- **Topic**: `Symbol("export_controls")`, `Symbol("shipment_authorized")`
- **Payload Schema**:
  ```json
  {
    "shipment_id": "SHP-9921",
    "consignee": "GDKL2...",
    "screening_status": "CLEAR"
  }
  ```

---

## 3. Trade Compliance Events (`src/trade_compliance.rs`)

### `shipment_dispatched`
- **Topic**: `Symbol("trade_compliance")`, `Symbol("shipment_dispatched")`
- **Payload Schema**:
  ```json
  {
    "tracking_id": "TRK-00129",
    "certificate_of_origin_hash": "0x7f83b1657ff1fc53...",
    "origin_verified": true,
    "hs_code": "84713000"
  }
  ```

---

## 4. Data Retention Events (`src/data_retention.rs`)

### `erasure_requested` (GDPR Article 17)
- **Topic**: `Symbol("data_retention")`, `Symbol("erasure_requested")`
- **Payload Schema**:
  ```json
  {
    "request_id": "GDPR-DEL-912",
    "data_subject_hash": "0xanon_sub_123",
    "pending_days": 14,
    "erasure_completed": true,
    "legal_hold": false
  }
  ```
