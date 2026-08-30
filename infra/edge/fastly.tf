# Fastly Compute@Edge Infrastructure (#521)

terraform {
  required_providers {
    fastly = {
      source  = "fastly/fastly"
      version = "~> 5.0"
    }
  }
}

resource "fastly_service_compute" "edge_service" {
  name = "audit-ledger-fastly-edge"

  domain {
    name    = "fastly-edge.audit-ledger.network"
    comment = "Fastly Compute Edge Domain"
  }

  backend {
    name    = "audit_api"
    address = "api.audit-ledger.network"
    port    = 443
  }

  package {
    filename         = "${path.module}/../../edge/fastly-compute-edge/pkg/package.tar.gz"
    source_code_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  }

  force_destroy = true
}
