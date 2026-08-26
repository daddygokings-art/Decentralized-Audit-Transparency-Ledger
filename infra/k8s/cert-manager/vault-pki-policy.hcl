# Vault policy for cert-manager's Vault issuer (see
# cluster-issuer-internal-ca.yaml). Scoped to signing only — cert-manager
# never needs to manage the PKI mount itself.
#
# Apply with:
#   vault policy write cert-manager-pki infra/k8s/cert-manager/vault-pki-policy.hcl

path "pki/sign/internal-mtls" {
  capabilities = ["create", "update"]
}

path "pki/cert/ca" {
  capabilities = ["read"]
}
