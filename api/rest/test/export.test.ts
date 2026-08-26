/**
 * Tests for the event export module (#201).
 *
 * Covers:
 *  - export_events / exportJson / exportCsv with time range filters
 *  - CSV and JSON format correctness
 *  - Streaming export via createStreamingExporter
 *  - Integrity proof generation and structure
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { resetEvents, resolvers } from "../../graphql/src/resolvers";
import {
  export_events,
  exportJson,
  exportCsv,
  createStreamingExporter,
  DEFAULT_FIELDS,
  ExportOptions,
  ExportFilter,
  IntegrityProof,
} from "../src/export";

// ── Test helpers ──────────────────────────────────────────────────────────────

/** Seed the in-memory store via the GraphQL mutation (bypasses auth for tests). */
function seedEvent(
  index: number,
  eventType: string,
  submitter: string,
  timestamp: number,
  metadata = "deadbeef",
) {
  // Directly push into the resolver store via the public logEvent mutation
  // by calling the internal resolver with a fake ctx that bypasses auth.
  (resolvers.Mutation as any).logEvent(
    null,
    { submitter, eventType, metadata },
    { apiKey: process.env.API_KEY ?? "test-key" },
  );
  // Override timestamp on the last entry so time-range tests are deterministic.
  const events = (resolvers.Query as any).events(null, { limit: 1000, offset: 0, filter: null }, null);
  const last = events[events.length - 1];
  if (last) last.timestamp = timestamp;
}

function allEvents() {
  return (resolvers.Query as any).events(null, { limit: 1000, offset: 0, filter: null }, null);
}

// ── Setup / Teardown ──────────────────────────────────────────────────────────

beforeEach(() => {
  // Reset the in-memory store before each test.
  resetEvents();
  process.env.API_KEY = "test-key";
});

afterEach(() => {
  resetEvents();
});

// ── export_events ─────────────────────────────────────────────────────────────

describe("export_events", () => {
  it("returns JSON result by default", () => {
    seedEvent(0, "payment", "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 1000);
    const result = export_events({ format: "json" });
    expect(result.contentType).toContain("application/json");
    expect(result.progress.status).toBe("completed");
  });

  it("returns CSV result when format is csv", () => {
    seedEvent(0, "payment", "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", 1000);
    const result = export_events({ format: "csv" });
    expect(result.contentType).toContain("text/csv");
    expect(result.progress.status).toBe("completed");
  });

  it("returns streaming metadata when stream=true", () => {
    const result = export_events({ format: "json", stream: true });
    expect(result.data).toBe("");
    expect(result.progress.status).toBe("running");
  });
});

// ── exportJson ────────────────────────────────────────────────────────────────

describe("exportJson", () => {
  beforeEach(() => {
    const submitter = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    seedEvent(0, "payment", submitter, 1000);
    seedEvent(1, "transfer", submitter, 2000);
    seedEvent(2, "payment", submitter, 3000);
  });

  it("exports all events when no filter applied", () => {
    const result = exportJson({ format: "json" });
    const payload = JSON.parse(result.data);
    expect(payload.total).toBe(3);
    expect(payload.data).toHaveLength(3);
  });

  it("filters events by startTime", () => {
    const result = exportJson({ format: "json", filter: { startTime: 2000 } });
    const payload = JSON.parse(result.data);
    expect(payload.total).toBe(2);
    payload.data.forEach((e: any) => expect(e.timestamp).toBeGreaterThanOrEqual(2000));
  });

  it("filters events by endTime", () => {
    const result = exportJson({ format: "json", filter: { endTime: 2000 } });
    const payload = JSON.parse(result.data);
    expect(payload.total).toBe(2);
    payload.data.forEach((e: any) => expect(e.timestamp).toBeLessThanOrEqual(2000));
  });

  it("filters events by startTime and endTime range", () => {
    const result = exportJson({ format: "json", filter: { startTime: 1500, endTime: 2500 } });
    const payload = JSON.parse(result.data);
    expect(payload.total).toBe(1);
    expect(payload.data[0].timestamp).toBe(2000);
  });

  it("filters events by type", () => {
    const result = exportJson({ format: "json", filter: { type: "payment" } });
    const payload = JSON.parse(result.data);
    expect(payload.total).toBe(2);
    payload.data.forEach((e: any) => expect(e.event_type).toBe("payment"));
  });

  it("respects limit and offset", () => {
    const result = exportJson({ format: "json", limit: 2, offset: 1 });
    const payload = JSON.parse(result.data);
    expect(payload.total).toBe(2);
    expect(payload.data).toHaveLength(2);
  });

  it("only includes specified fields", () => {
    const result = exportJson({ format: "json", fields: ["index", "event_type"] });
    const payload = JSON.parse(result.data);
    payload.data.forEach((e: any) => {
      expect(Object.keys(e)).toEqual(["index", "event_type"]);
    });
  });

  it("includes default fields when fields not specified", () => {
    const result = exportJson({ format: "json" });
    const payload = JSON.parse(result.data);
    payload.data.forEach((e: any) => {
      for (const field of DEFAULT_FIELDS) {
        expect(Object.keys(e)).toContain(field);
      }
    });
  });

  it("includes integrity proof when includeProof is true", () => {
    const result = exportJson({ format: "json", includeProof: true });
    expect(result.proof).toBeDefined();
    expect(result.proof!.eventCount).toBe(3);
    expect(result.proof!.exportHash).toMatch(/^[0-9a-f]{64}$/);
    expect(result.proof!.generatedAt).toBeGreaterThan(0);
    const payload = JSON.parse(result.data);
    expect(payload.integrity_proof).toBeDefined();
  });

  it("integrity proof changes when filter changes event set", () => {
    const r1 = exportJson({ format: "json", includeProof: true });
    const r2 = exportJson({ format: "json", includeProof: true, filter: { type: "payment" } });
    expect(r1.proof!.exportHash).not.toBe(r2.proof!.exportHash);
  });

  it("proof records the applied filter", () => {
    const filter: ExportFilter = { startTime: 1500, type: "transfer" };
    const result = exportJson({ format: "json", includeProof: true, filter });
    expect(result.proof!.appliedFilter).toEqual(filter);
  });

  it("does not include integrity proof when includeProof is false", () => {
    const result = exportJson({ format: "json", includeProof: false });
    expect(result.proof).toBeUndefined();
    const payload = JSON.parse(result.data);
    expect(payload.integrity_proof).toBeUndefined();
  });

  it("returns correct content type", () => {
    const result = exportJson({ format: "json" });
    expect(result.contentType).toBe("application/json; charset=utf-8");
  });

  it("filename includes .json extension", () => {
    const result = exportJson({ format: "json" });
    expect(result.filename).toMatch(/\.json$/);
  });

  it("progress is completed on success", () => {
    const result = exportJson({ format: "json" });
    expect(result.progress.status).toBe("completed");
    expect(result.progress.exported).toBe(3);
    expect(result.progress.percentage).toBe(100);
  });

  it("empty export produces empty data array", () => {
    resetEvents();
    const result = exportJson({ format: "json" });
    const payload = JSON.parse(result.data);
    expect(payload.total).toBe(0);
    expect(payload.data).toHaveLength(0);
  });
});

// ── exportCsv ─────────────────────────────────────────────────────────────────

describe("exportCsv", () => {
  const submitter = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

  beforeEach(() => {
    seedEvent(0, "payment", submitter, 1000);
    seedEvent(1, "transfer", submitter, 2000);
    seedEvent(2, "payment", submitter, 3000);
  });

  it("exports all events as CSV rows", () => {
    const result = exportCsv({ format: "csv" });
    const lines = result.data.split("\n").filter((l) => l && !l.startsWith("#"));
    // header + 3 data rows
    expect(lines).toHaveLength(4);
  });

  it("first line is a header row with default field names", () => {
    const result = exportCsv({ format: "csv" });
    const [header] = result.data.split("\n");
    expect(header).toBe(DEFAULT_FIELDS.join(","));
  });

  it("filters by startTime and endTime", () => {
    const result = exportCsv({ format: "csv", filter: { startTime: 2000, endTime: 2000 } });
    const lines = result.data.split("\n").filter((l) => l && !l.startsWith("#"));
    // header + 1 matching row
    expect(lines).toHaveLength(2);
  });

  it("filters by type", () => {
    const result = exportCsv({ format: "csv", filter: { type: "payment" } });
    const lines = result.data.split("\n").filter((l) => l && !l.startsWith("#"));
    expect(lines).toHaveLength(3); // header + 2 payment rows
  });

  it("respects custom fields", () => {
    const result = exportCsv({ format: "csv", fields: ["index", "event_type"] });
    const [header] = result.data.split("\n");
    expect(header).toBe("index,event_type");
  });

  it("escapes commas in field values", () => {
    // metadata with comma — seed directly
    resetEvents();
    (resolvers.Mutation as any).logEvent(null, {
      submitter,
      eventType: "payment",
      metadata: "aabb",
    }, { apiKey: "test-key" });
    // Override metadata value on the event record
    const events = allEvents();
    events[0].metadata = "value,with,comma";

    const result = exportCsv({ format: "csv", fields: ["metadata"] });
    const lines = result.data.split("\n");
    // The data row should wrap the field in double-quotes
    expect(lines[1]).toMatch(/^"/);
  });

  it("appends integrity proof comment when includeProof is true", () => {
    const result = exportCsv({ format: "csv", includeProof: true });
    expect(result.data).toContain("# integrity_proof,");
    expect(result.proof).toBeDefined();
    expect(result.proof!.eventCount).toBe(3);
    expect(result.proof!.exportHash).toMatch(/^[0-9a-f]{64}$/);
  });

  it("does not append integrity proof when includeProof is false", () => {
    const result = exportCsv({ format: "csv", includeProof: false });
    expect(result.data).not.toContain("# integrity_proof");
    expect(result.proof).toBeUndefined();
  });

  it("returns correct content type", () => {
    const result = exportCsv({ format: "csv" });
    expect(result.contentType).toBe("text/csv; charset=utf-8");
  });

  it("filename includes .csv extension", () => {
    const result = exportCsv({ format: "csv" });
    expect(result.filename).toMatch(/\.csv$/);
  });

  it("progress is completed on success", () => {
    const result = exportCsv({ format: "csv" });
    expect(result.progress.status).toBe("completed");
    expect(result.progress.percentage).toBe(100);
  });
});

// ── createStreamingExporter ───────────────────────────────────────────────────

describe("createStreamingExporter", () => {
  const submitter = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

  beforeEach(() => {
    for (let i = 0; i < 5; i++) {
      seedEvent(i, "payment", submitter, 1000 + i * 1000);
    }
  });

  it("streams all events as JSON", async () => {
    const exporter = createStreamingExporter({ format: "json" });
    const chunks: string[] = [];
    for await (const chunk of exporter.generate()) {
      chunks.push(chunk);
    }
    const full = chunks.join("");
    const parsed = JSON.parse(full);
    expect(parsed.total).toBe(5);
    expect(parsed.data).toHaveLength(5);
  });

  it("streams all events as CSV", async () => {
    const exporter = createStreamingExporter({ format: "csv" });
    const chunks: string[] = [];
    for await (const chunk of exporter.generate()) {
      chunks.push(chunk);
    }
    const full = chunks.join("");
    const lines = full.split("\n").filter((l) => l && !l.startsWith("#"));
    // header + 5 data rows
    expect(lines).toHaveLength(6);
  });

  it("tracks progress to 100% on completion", async () => {
    const exporter = createStreamingExporter({ format: "json" });
    // drain the generator
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    for await (const _ of exporter.generate()) { /* noop */ }
    expect(exporter.progress.status).toBe("completed");
    expect(exporter.progress.percentage).toBe(100);
    expect(exporter.progress.exported).toBe(5);
  });

  it("filters events by time range in streaming mode", async () => {
    const exporter = createStreamingExporter({
      format: "json",
      filter: { startTime: 3000, endTime: 4000 },
    });
    const chunks: string[] = [];
    for await (const chunk of exporter.generate()) {
      chunks.push(chunk);
    }
    const parsed = JSON.parse(chunks.join(""));
    expect(parsed.total).toBe(2);
    parsed.data.forEach((e: any) => {
      expect(e.timestamp).toBeGreaterThanOrEqual(3000);
      expect(e.timestamp).toBeLessThanOrEqual(4000);
    });
  });

  it("appends integrity proof in JSON streaming mode", async () => {
    const exporter = createStreamingExporter({ format: "json", includeProof: true });
    const chunks: string[] = [];
    for await (const chunk of exporter.generate()) {
      chunks.push(chunk);
    }
    const parsed = JSON.parse(chunks.join(""));
    expect(parsed.integrity_proof).toBeDefined();
    expect(parsed.integrity_proof.eventCount).toBe(5);
    expect(parsed.integrity_proof.exportHash).toMatch(/^[0-9a-f]{64}$/);
  });

  it("appends integrity proof comment in CSV streaming mode", async () => {
    const exporter = createStreamingExporter({ format: "csv", includeProof: true });
    const chunks: string[] = [];
    for await (const chunk of exporter.generate()) {
      chunks.push(chunk);
    }
    const full = chunks.join("");
    expect(full).toContain("# integrity_proof,");
  });

  it("respects hard limit in streaming mode", async () => {
    const exporter = createStreamingExporter({ format: "json", limit: 3 });
    const chunks: string[] = [];
    for await (const chunk of exporter.generate()) {
      chunks.push(chunk);
    }
    const parsed = JSON.parse(chunks.join(""));
    expect(parsed.total).toBe(3);
  });

  it("progress starts as running", () => {
    const exporter = createStreamingExporter({ format: "json" });
    expect(exporter.progress.status).toBe("running");
  });

  it("completedAt is set after generation finishes", async () => {
    const exporter = createStreamingExporter({ format: "json" });
    for await (const _ of exporter.generate()) { /* noop */ }
    expect(exporter.progress.completedAt).toBeGreaterThan(0);
  });
});

// ── Integrity proof determinism ───────────────────────────────────────────────

describe("integrity proof determinism", () => {
  it("same events produce the same exportHash", () => {
    resetEvents();
    const submitter = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    seedEvent(0, "payment", submitter, 1000);
    seedEvent(1, "transfer", submitter, 2000);

    const r1 = exportJson({ format: "json", includeProof: true });
    const r2 = exportJson({ format: "json", includeProof: true });
    expect(r1.proof!.exportHash).toBe(r2.proof!.exportHash);
  });

  it("CSV and JSON produce the same exportHash for the same event set", () => {
    resetEvents();
    const submitter = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    seedEvent(0, "payment", submitter, 1000);
    seedEvent(1, "transfer", submitter, 2000);

    const jsonResult = exportJson({ format: "json", includeProof: true });
    const csvResult = exportCsv({ format: "csv", includeProof: true });
    expect(jsonResult.proof!.exportHash).toBe(csvResult.proof!.exportHash);
  });
});
