# AuditLedger Backup & Restore Tools

Scripts for exporting, restoring, and verifying AuditLedger contract event data.

## Prerequisites

- [Soroban CLI](https://soroban.stellar.org/docs/setup/install)
- `jq` (JSON processor)
- `xxd` (hex encoding, included in `vim-common`)
- `aws` CLI (optional, for S3 uploads)

## Configuration

Edit `config.json` with your contract details:

```json
{
  "rpc_url": "https://soroban-testnet.stellar.org",
  "network_passphrase": "Test SDF Network ; September 2015",
  "contract_id": "CCXMTP7...",
  "backup_dir": "./backups",
  "max_batch_size": 100
}
```

## Usage

### Backup

Export all events to a JSON file:

```bash
./backup.sh --config config.json --output ./backups/my-backup.json
```

Upload to S3 after backup:

```bash
./backup.sh --s3-upload
```

### Restore

Replay events from a backup onto a contract:

```bash
./restore.sh --backup ./backups/my-backup.json --config config.json
```

Preview without making changes:

```bash
./restore.sh --backup ./backups/my-backup.json --dry-run
```

### Verify

Compare on-chain events against a backup:

```bash
./verify.sh --backup ./backups/my-backup.json --config config.json
```

## Automating with Cron

Run backups daily:

```cron
0 2 * * * /path/to/tools/backup/backup.sh --config /path/to/config.json 2>&1 | logger -t audit-ledger-backup
```

## Backup Format

```json
{
  "backup_timestamp": "2025-06-23T02:00:00Z",
  "contract_id": "CCXMTP7...",
  "block_height": 123456,
  "total_events": 42,
  "events": [
    {
      "index": 0,
      "timestamp": 1719000000,
      "event_type": "payment",
      "submitter": "GB...",
      "metadata": "dHgx"
    }
  ]
}
```
