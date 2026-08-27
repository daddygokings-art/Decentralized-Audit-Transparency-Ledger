# Vault policy: signing key rotation via the `transit` secrets engine.
# Transit keeps every prior key version so old signatures/tokens stay
# verifiable through their validity window while new material is signed
# with the latest version (`min_encryption_version` enforces this).
#
# Apply with:
#   vault policy write audit-ledger-signing-key-rotator infra/k8s/vault/policies/signing-keys.hcl

path "transit/keys/audit-ledger-event-signing" {
  capabilities = ["read"]
}

path "transit/keys/audit-ledger-event-signing/rotate" {
  capabilities = ["update"]
}

path "transit/keys/audit-ledger-event-signing/config" {
  capabilities = ["update"]
}

path "transit/sign/audit-ledger-event-signing" {
  capabilities = ["update"]
}

path "transit/verify/audit-ledger-event-signing" {
  capabilities = ["update"]
}

path "transit/keys/audit-ledger-jwt-signing" {
  capabilities = ["read"]
}

path "transit/keys/audit-ledger-jwt-signing/rotate" {
  capabilities = ["update"]
}

path "transit/keys/audit-ledger-jwt-signing/config" {
  capabilities = ["update"]
}
