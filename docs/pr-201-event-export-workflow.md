# PR: Add Event Export Workflow

## Summary

Implements full event export functionality for off-chain analysis as described in issue #201.

## Changes

### `api/rest/src/export.ts` (rewritten)
- Added `export_events(options)` — the primary entry point satisfying the issue requirement
- Added `ExportFilter` type with `startTime`, `endTime`, `type`, and `submitter` fields for time-range and field-level filtering
- `exportJson(options)` — buffered JSON export, returns `{ data, total, integrity_proof? }`
- `exportCsv(options)` — buffered CSV export with optional integrity proof appended as a comment line
- `createStreamingExporter(options)` — async generator for large datasets; yields chunks without loading all events into memory; supports both CSV and JSON output formats
- `IntegrityProof` type: SHA-256 of all `event_hash` values in export order, event count, generation timestamp, and applied filter — gives auditors a tamper-evident fingerprint of every export

### `api/rest/src/server.ts`
- Updated export route imports to use the new `export_events` / `exportJson` / `exportCsv` / `createStreamingExporter` API
- `GET /v1/export/events.json` — JSON download with optional `startTime`, `endTime`, `type`, `submitter`, `limit`, `offset`, `fields` query params; integrity proof included by default (`proof=false` to disable); sets `X-Export-Hash` and `X-Export-Event-Count` response headers
- `GET /v1/export/events.csv` — CSV download with same filter params and proof headers
- `GET /v1/export/events/stream` — streaming chunked export, supports `format=csv` or `format=json`; tracks progress in `X-Export-Status` / `X-Export-Event-Count` headers
- `GET /v1/export/progress` — informational endpoint describing how to track streaming progress
- Added `parseExportFilter` helper that reads both flat query params (`startTime`, `endTime`) and a legacy JSON `filter` param for backwards compatibility

### `api/rest/package.json`
- Added `zod` (used by `validation.ts`) and `js-yaml` (used by `server.ts`) to `dependencies`
- Added `@types/js-yaml` to `devDependencies`

### `api/rest/tsconfig.json`
- Removed `rootDir` restriction so test files under `test/` are included by the compiler

### `api/rest/test/export.test.ts` (new)
- 30+ unit tests covering:
  - `export_events` format dispatch and streaming flag
  - `exportJson`: time range filter, type filter, limit/offset, field projection, integrity proof structure and determinism, empty dataset
  - `exportCsv`: row count, header row, time range filter, type filter, custom fields, CSV escaping, integrity proof comment
  - `createStreamingExporter`: JSON/CSV streaming, progress tracking, time range filter, hard limit, proof in both formats
  - Cross-format proof determinism (same event set → same `exportHash` in CSV and JSON)

## Acceptance Criteria

| Criterion | Met |
|-----------|-----|
| Events can be exported by time range | ✅ `startTime` / `endTime` filter in all export modes |
| Multiple format support (CSV + JSON) | ✅ `format: "csv" \| "json"` in every export function |
| Exports include verification data | ✅ `IntegrityProof` with SHA-256 `exportHash`, event count, timestamp, applied filter |
| Streaming export for large datasets | ✅ `createStreamingExporter` yields batched chunks via async generator |

closes #201
