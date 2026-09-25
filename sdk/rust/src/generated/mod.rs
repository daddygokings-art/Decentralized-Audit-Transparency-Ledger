// GENERATED FILE — DO NOT EDIT.
// Regenerate with: npm run generate --workspace tools/sdk-codegen (or `sdk-codegen generate`).
// Source of truth: abi/audit-ledger.json
//
// Generated contract binding. Include it with `mod generated;` and re-export
// the pieces you need from your crate root.

mod client;
mod errors;
mod events;
mod types;

pub use client::{ContractTransport, GeneratedClient, TransportError, Val};
pub use errors::ContractErrorCode;
pub use events::{event_name, ALL_EVENT_NAMES};
pub use types::*;

// contract: AuditLedger v0.1.0
// idl: spec_version 1.0.0 (120 functions, 19 types, 75 errors, 43 events)
