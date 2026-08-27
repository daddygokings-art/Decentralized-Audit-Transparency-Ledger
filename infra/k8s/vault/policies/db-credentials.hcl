# Vault policy: database credential rotation via the `database` secrets
# engine (dynamic credentials with a bounded lease — no long-lived DB
# passwords are ever stored).
#
# Apply with:
#   vault policy write audit-ledger-db-rotator infra/k8s/vault/policies/db-credentials.hcl

# Read dynamic, short-lived DB credentials.
path "database/creds/audit-ledger-app" {
  capabilities = ["read"]
}

# Force-rotate the root credential Vault uses to manage the database.
# Restricted to the rotation CronJob's own AppRole/K8s auth role.
path "database/rotate-root/audit-ledger-db" {
  capabilities = ["update"]
}

# Rotate the static role used by long-running connection pools that can't
# tolerate dynamic credential churn on every lease expiry.
path "database/rotate-role/audit-ledger-static" {
  capabilities = ["update"]
}

# Inspect lease metadata for validation/rollback tooling.
path "sys/leases/lookup" {
  capabilities = ["update"]
}

path "sys/leases/revoke" {
  capabilities = ["update"]
}
