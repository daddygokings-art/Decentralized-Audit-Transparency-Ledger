# Contract Event Edge Computing Integration (#521)

This module implements high-performance, low-latency event ingestion and query caching across global edge computing platforms:
- **Cloudflare Workers** (KV, Cache API, global Anycast network)
- **AWS Lambda@Edge** (CloudFront viewer-request & origin-response hooks)
- **Fastly Compute@Edge** (Viceroy, Lucet WebAssembly runtime, surrogate key purging)

---

## Key Features

1. **Low-Latency Edge Ingestion**:
   - Ingests events at the nearest edge POP (< 20ms TTFB).
   - Generates SHA-256 batch Merkle root hashes directly in V8/WASM edge runtimes.
   - Attests batch roots on Stellar Soroban smart contract (`src/edge_computing.rs`).

2. **Multi-Tier Edge Query Caching**:
   - Implements Stale-While-Revalidate (SWR) for high cache hit rates (> 95%).
   - Cryptographic query hashing and deterministic ETag generation.
   - Instant surrogate-key / tag-based cache purging.

3. **Multi-Cloud Edge Deployment**:
   - Native Cloudflare Worker (`edge/cloudflare/worker.ts`).
   - Lambda@Edge viewer-request and origin-response handlers (`edge/aws-lambda-edge/`).
   - Fastly Compute@Edge Rust/TypeScript worker (`edge/fastly-compute-edge/`).
   - Terraform infrastructure manifests for automated provisioning (`infra/edge/`).

4. **Soroban On-Chain Node Registry & Attestation**:
   - `register_edge_node`: Registers verified edge gateway nodes.
   - `record_cache_attestation`: Logs cryptographic proofs of edge cached query responses.
   - `record_edge_batch`: On-chain receipt for edge-ingested event batches.
   - `set_edge_cache_policy`: Configures TTL and invalidation policies per event type.

---

## Directory Structure

```
edge/
├── common/
│   ├── cache-manager.ts       # Multi-tier edge caching engine with SWR
│   ├── geo-router.ts          # GeoDNS and latency routing
│   ├── signature-verifier.ts  # Edge cryptographic validator
│   └── types.ts               # Edge data models and interfaces
├── cloudflare/
│   ├── package.json
│   ├── worker.ts              # Cloudflare Workers implementation
│   └── wrangler.toml          # Cloudflare configuration
├── aws-lambda-edge/
│   ├── originResponse.ts      # CloudFront origin response handler
│   ├── serverless.yml         # Serverless framework configuration
│   └── viewerRequest.ts       # CloudFront viewer request handler
└── fastly-compute-edge/
    ├── fastly.toml            # Fastly configuration
    └── src/index.ts           # Fastly Compute@Edge worker

infra/edge/
├── cloudflare.tf              # Terraform Cloudflare definitions
├── cloudfront.tf              # Terraform AWS CloudFront + Lambda@Edge
└── fastly.tf                  # Terraform Fastly Compute@Edge definitions

monitoring/grafana/dashboards/
└── edge-computing.json        # Grafana dashboard for edge metrics
```
