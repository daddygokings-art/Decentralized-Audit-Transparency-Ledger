# Reproducible WASM Builds

This document describes how to verify that a deployed Soroban contract WASM binary matches the source code in this repository.

## Trust Model

Reproducible builds ensure that:
1. The deployed WASM was built from the exact source code in the repository
2. No intermediate modifications occurred during compilation
3. Auditors can independently verify contract integrity

## Prerequisites

- Rust toolchain (version specified below)
- Docker (recommended for bit-by-bit reproducibility)

## Build Environment

To ensure reproducible builds, we pin exact versions:

```bash
rustup install 1.80.0
rustup target add wasm32-unknown-unknown
```

## Build Instructions

### Local Build (Native)

```bash
# Clean any previous builds
cargo clean

# Build with exact settings
cargo build --target wasm32-unknown-unknown --release \
  --locked
```

### Docker Build (Recommended)

Using Docker ensures bit-by-bit reproducibility across systems:

```bash
# Build using the provided Dockerfile
docker build -f docker/wasm.Dockerfile -t audit-ledger:build .

# Extract the WASM from the image
docker run --rm -v $(pwd)/target:/out audit-ledger:build \
  cp target/wasm32-unknown-unknown/release/audit_ledger.wasm /out/
```

## Verify WASM Binary

### Compute SHA-256

After building, compute the SHA-256 hash of the WASM binary:

```bash
sha256sum target/wasm32-unknown-unknown/release/audit_ledger.wasm
```

This produces output like:
```
abc123def456... target/wasm32-unknown-unknown/release/audit_ledger.wasm
```

### Compare Against Published Hash

Find the published hash for your release in one of these places:

1. **GitHub Releases** — Check the release notes for the associated commit tag
2. **CI Artifacts** — View GitHub Actions workflow logs
3. **Contract Deployment Record** — Check the deployment logs for the network (testnet/mainnet)

```bash
# Example: verify against published hash
PUBLISHED_HASH="abc123def456..."
COMPUTED_HASH=$(sha256sum target/wasm32-unknown-unknown/release/audit_ledger.wasm | cut -d' ' -f1)

if [ "$PUBLISHED_HASH" = "$COMPUTED_HASH" ]; then
  echo "✓ WASM binary matches published hash"
else
  echo "✗ WASM binary DOES NOT match"
  echo "Published: $PUBLISHED_HASH"
  echo "Computed:  $COMPUTED_HASH"
  exit 1
fi
```

## CI Integration

The CI/CD pipeline automatically:

1. Builds the WASM binary in a Docker container
2. Computes the SHA-256 hash
3. Publishes the hash as a build artifact

### View Published Hashes

```bash
# Download artifacts from GitHub Actions
gh run list --repo daddygokings-art/Decentralized-Audit-Transparency-Ledger
gh run download <RUN_ID> -n wasm-sha256
cat wasm-sha256.txt
```

## Troubleshooting

### Hash Mismatch

If your computed hash doesn't match the published hash:

1. **Verify Rust version**
   ```bash
   rustc --version
   cargo --version
   ```

2. **Check for uncommitted changes**
   ```bash
   git status
   ```

3. **Ensure exact dependencies**
   ```bash
   cargo update --frozen
   ```

4. **Rebuild in Docker** (most reliable)
   ```bash
   docker build -f docker/wasm.Dockerfile -t audit-ledger:build .
   ```

### Binary Size Differs

If the WASM binary size differs significantly:
- Check the Rust version (affects codegen)
- Verify compiler flags in `Cargo.toml`
- Ensure LTO and optimization settings match

## Security Considerations

1. **Always verify hashes** — Never trust a deployment without verifying the WASM hash
2. **Use multiple sources** — Cross-check published hashes from GitHub, CI, and deployment records
3. **Audit the source** — Review the source code corresponding to the tag/commit
4. **Independent verification** — If possible, rebuild locally and compare

## References

- [Soroban Documentation](https://soroban.stellar.org/)
- [Reproducible Builds Project](https://reproducible-builds.org/)
- [SHA-256 Verification](https://en.wikipedia.org/wiki/SHA-2)
