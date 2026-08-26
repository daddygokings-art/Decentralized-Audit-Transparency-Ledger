/**
 * Event Export Module (#201)
 *
 * Provides export_events with time range filtering, CSV and JSON formats,
 * streaming export for large datasets, and integrity proofs in exports.
 */

import { resolvers } from "../../graphql/src/resolvers";
import { createHash } from "crypto";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface ExportFilter {
  /** Epoch seconds — only events at or after this time are included. */
  startTime?: number;
  /** Epoch seconds — only events at or before this time are included. */
  endTime?: number;
  /** Only events of this type are included. */
  type?: string;
  /** Only events from this submitter are included. */
  submitter?: string;
}

export interface ExportOptions {
  format: "csv" | "json";
  filter?: ExportFilter;
  limit?: number;
  offset?: number;
  /** Subset of fields to include in the output. Defaults to DEFAULT_FIELDS. */
  fields?: string[];
  /** When true, return a streaming async generator rather than buffered output. */
  stream?: boolean;
  /** When true, append an integrity proof block to the export. */
  includeProof?: boolean;
}

export interface IntegrityProof {
  /** Total events included in this export. */
  eventCount: number;
  /** SHA-256 of the concatenated event_hash values in export order. */
  exportHash: string;
  /** Unix ms timestamp when the proof was generated. */
  generatedAt: number;
  /** Filters applied, for auditor reference. */
  appliedFilter?: ExportFilter;
}

export interface ExportProgress {
  total: number;
  exported: number;
  percentage: number;
  status: "running" | "completed" | "failed";
  startedAt: number;
  completedAt?: number;
  error?: string;
}

export interface ExportResult {
  data: string;
  contentType: string;
  filename: string;
  progress: ExportProgress;
  /** Present when options.includeProof is true. */
  proof?: IntegrityProof;
}

export const DEFAULT_FIELDS = [
  "index",
  "timestamp",
  "event_type",
  "submitter",
  "metadata",
  "event_hash",
  "prev_hash",
];

const BATCH_SIZE = 500;

// ── Internal helpers ──────────────────────────────────────────────────────────

function escapeCsvField(value: unknown): string {
  const str = String(value ?? "");
  if (str.includes(",") || str.includes('"') || str.includes("\n")) {
    return '"' + str.replace(/"/g, '""') + '"';
  }
  return str;
}

function eventToCsvRow(
  event: Record<string, unknown>,
  fields: string[],
): string {
  return fields.map((f) => escapeCsvField(event[f])).join(",");
}

function projectFields(
  event: Record<string, unknown>,
  fields: string[],
): Record<string, unknown> {
  const obj: Record<string, unknown> = {};
  for (const f of fields) {
    obj[f] = event[f];
  }
  return obj;
}

function buildProof(
  events: Record<string, unknown>[],
  filter?: ExportFilter,
): IntegrityProof {
  const hasher = createHash("sha256");
  for (const e of events) {
    hasher.update(String(e["event_hash"] ?? ""));
  }
  return {
    eventCount: events.length,
    exportHash: hasher.digest("hex"),
    generatedAt: Date.now(),
    appliedFilter: filter,
  };
}

/**
 * Fetch all events that satisfy the given filter. The underlying resolver
 * already understands startTime / endTime via its filter parameter.
 */
function fetchEvents(
  filter?: ExportFilter,
  limit?: number,
  offset?: number,
): Record<string, unknown>[] {
  return resolvers.Query.events(
    null,
    { limit: limit ?? 1_000_000, offset: offset ?? 0, filter },
    null,
  ) as Record<string, unknown>[];
}

// ── Public API ────────────────────────────────────────────────────────────────

/**
 * Export events within an optional time range (and other filter criteria).
 *
 * Fulfils issue #201:
 *   - Time range filtering via filter.startTime / filter.endTime
 *   - CSV and JSON format support
 *   - Streaming via options.stream = true
 *   - Integrity proofs via options.includeProof = true
 */
export function export_events(options: ExportOptions): ExportResult {
  if (options.stream) {
    // For streaming we return metadata only; caller uses createStreamingExporter.
    const exporter = createStreamingExporter(options);
    return {
      data: "",
      contentType:
        options.format === "csv"
          ? "text/csv; charset=utf-8"
          : "application/json; charset=utf-8",
      filename: `audit-events-${Date.now()}.${options.format}`,
      progress: exporter.progress,
    };
  }

  return options.format === "csv"
    ? exportCsv(options)
    : exportJson(options);
}

export function exportCsv(options: ExportOptions): ExportResult {
  const fields = options.fields ?? DEFAULT_FIELDS;
  const events = fetchEvents(options.filter, options.limit, options.offset);
  const startedAt = Date.now();

  const header = fields.join(",");
  const rows = events.map((e) => eventToCsvRow(e, fields));

  let data = [header, ...rows].join("\n");

  let proof: IntegrityProof | undefined;
  if (options.includeProof) {
    proof = buildProof(events, options.filter);
    // Append proof as a comment block at the end of the CSV
    data +=
      "\n# integrity_proof," +
      escapeCsvField(JSON.stringify(proof));
  }

  return {
    data,
    contentType: "text/csv; charset=utf-8",
    filename: `audit-events-${Date.now()}.csv`,
    proof,
    progress: {
      total: events.length,
      exported: events.length,
      percentage: 100,
      status: "completed",
      startedAt,
      completedAt: Date.now(),
    },
  };
}

export function exportJson(options: ExportOptions): ExportResult {
  const fields = options.fields ?? DEFAULT_FIELDS;
  const events = fetchEvents(options.filter, options.limit, options.offset);
  const startedAt = Date.now();

  const mapped = events.map((e) => projectFields(e, fields));

  let proof: IntegrityProof | undefined;
  if (options.includeProof) {
    proof = buildProof(events, options.filter);
  }

  const payload: Record<string, unknown> = {
    data: mapped,
    total: mapped.length,
  };
  if (proof) {
    payload["integrity_proof"] = proof;
  }

  return {
    data: JSON.stringify(payload, null, 2),
    contentType: "application/json; charset=utf-8",
    filename: `audit-events-${Date.now()}.json`,
    proof,
    progress: {
      total: events.length,
      exported: events.length,
      percentage: 100,
      status: "completed",
      startedAt,
      completedAt: Date.now(),
    },
  };
}

/**
 * Create a streaming exporter for large datasets.
 * Returns a progress tracker and an async generator that yields chunks.
 */
export function createStreamingExporter(options: ExportOptions) {
  const fields = options.fields ?? DEFAULT_FIELDS;
  const progress: ExportProgress = {
    total: 0,
    exported: 0,
    percentage: 0,
    status: "running",
    startedAt: Date.now(),
  };

  return {
    progress,

    async *generate(): AsyncGenerator<string> {
      try {
        let offset = options.offset ?? 0;
        const hardLimit = options.limit ?? Number.MAX_SAFE_INTEGER;
        const hasher = options.includeProof ? createHash("sha256") : null;

        if (options.format === "csv") {
          yield fields.join(",") + "\n";
        } else {
          yield '{"data":[';
        }

        let isFirst = true;
        let totalExported = 0;

        while (totalExported < hardLimit) {
          const batchSize = Math.min(BATCH_SIZE, hardLimit - totalExported);
          const batch = fetchEvents(options.filter, batchSize, offset) as Record<string, unknown>[];
          if (batch.length === 0) break;

          progress.total += batch.length;

          for (const event of batch) {
            if (hasher) {
              hasher.update(String(event["event_hash"] ?? ""));
            }

            if (options.format === "csv") {
              yield eventToCsvRow(event, fields) + "\n";
            } else {
              if (!isFirst) yield ",";
              yield JSON.stringify(projectFields(event, fields));
              isFirst = false;
            }

            progress.exported++;
            totalExported++;
            progress.percentage =
              progress.total > 0
                ? Math.round((progress.exported / progress.total) * 100)
                : 0;
          }

          offset += batch.length;
          if (batch.length < batchSize) break;
        }

        if (options.format === "json") {
          let suffix = `],"total":${progress.exported}`;
          if (options.includeProof && hasher) {
            const proof: IntegrityProof = {
              eventCount: progress.exported,
              exportHash: hasher.digest("hex"),
              generatedAt: Date.now(),
              appliedFilter: options.filter,
            };
            suffix += `,"integrity_proof":${JSON.stringify(proof)}`;
          }
          suffix += "}";
          yield suffix;
        } else if (options.format === "csv" && options.includeProof && hasher) {
          const proof: IntegrityProof = {
            eventCount: progress.exported,
            exportHash: hasher.digest("hex"),
            generatedAt: Date.now(),
            appliedFilter: options.filter,
          };
          yield "\n# integrity_proof," + escapeCsvField(JSON.stringify(proof));
        }

        progress.status = "completed";
        progress.completedAt = Date.now();
        progress.percentage = 100;
      } catch (err) {
        progress.status = "failed";
        progress.error = err instanceof Error ? err.message : String(err);
        throw err;
      }
    },
  };
}
