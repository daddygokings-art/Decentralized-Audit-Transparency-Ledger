# Vault policy: API key rotation via the KV v2 secrets engine, versioned
# so the previous key remains readable for rollback until explicitly
# destroyed.
#
# Apply with:
#   vault policy write audit-ledger-api-key-rotator infra/k8s/vault/policies/api-keys.hcl

path "secret/data/audit-ledger/api-keys/*" {
  capabilities = ["create", "read", "update"]
}

path "secret/metadata/audit-ledger/api-keys/*" {
  capabilities = ["read", "list"]
}

# Rollback needs the previous version, not destroy — keep at least 5
# versions (configured on the mount, see rotate-api-keys.sh) and only
# allow soft delete of the current version, never hard destroy, from
# this policy.
path "secret/delete/audit-ledger/api-keys/*" {
  capabilities = ["update"]
}
