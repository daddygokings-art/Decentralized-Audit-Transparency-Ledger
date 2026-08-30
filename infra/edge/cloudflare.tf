# Cloudflare Workers & KV Infrastructure for Audit Ledger Edge Computing (#521)

terraform {
  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
  }
}

variable "cloudflare_zone_id" {
  type        = string
  description = "Cloudflare DNS Zone ID"
  default     = "023e105f4ecef8ad9ca31a8372d0c353"
}

variable "cloudflare_account_id" {
  type        = string
  description = "Cloudflare Account ID"
  default     = "bc82beaf9a3201a3cf1887d485524e7b"
}

resource "cloudflare_workers_kv_namespace" "event_cache" {
  account_id = var.cloudflare_account_id
  title      = "audit_ledger_event_cache"
}

resource "cloudflare_workers_kv_namespace" "rate_limit" {
  account_id = var.cloudflare_account_id
  title      = "audit_ledger_rate_limit"
}

resource "cloudflare_worker_script" "edge_worker" {
  account_id = var.cloudflare_account_id
  name       = "audit-ledger-edge-router"
  content    = file("${path.module}/../../edge/cloudflare/worker.ts")
  module     = true

  kv_namespace_binding {
    name         = "EVENT_CACHE_KV"
    namespace_id = cloudflare_workers_kv_namespace.event_cache.id
  }

  kv_namespace_binding {
    name         = "RATE_LIMIT_KV"
    namespace_id = cloudflare_workers_kv_namespace.rate_limit.id
  }

  plain_text_binding {
    name = "ENVIRONMENT"
    text = "production"
  }

  plain_text_binding {
    name = "DEFAULT_TTL_SECONDS"
    text = "60"
  }
}

resource "cloudflare_worker_route" "edge_route" {
  zone_id     = var.cloudflare_zone_id
  pattern     = "edge.audit-ledger.network/*"
  script_name = cloudflare_worker_script.edge_worker.name
}
