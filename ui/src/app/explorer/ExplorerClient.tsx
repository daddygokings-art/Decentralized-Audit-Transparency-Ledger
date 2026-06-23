"use client";
import { useEffect, useState, useCallback } from "react";
import { fetchTotalEvents, fetchEventPage } from "@/lib/contract";
import type { AuditEvent } from "@/types";

const PAGE_SIZE = 20;

type SortKey = keyof Pick<AuditEvent, "index" | "timestamp" | "event_type" | "submitter">;

function exportAs(events: AuditEvent[], format: "csv" | "json") {
  let content: string;
  let mime: string;
  let filename: string;
  if (format === "json") {
    content = JSON.stringify(events, null, 2);
    mime = "application/json";
    filename = "events.json";
  } else {
    const header = "index,timestamp,event_type,submitter,metadata,event_hash\n";
    const rows = events
      .map(
        (e) =>
          `${e.index},${e.timestamp},${e.event_type},${e.submitter},${e.metadata},${e.event_hash}`
      )
      .join("\n");
    content = header + rows;
    mime = "text/csv";
    filename = "events.csv";
  }
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

function tryDecodeMetadata(hex: string): string {
  try {
    return Buffer.from(hex, "hex").toString("utf8");
  } catch {
    return hex;
  }
}

export default function ExplorerClient() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(0);
  const [sortKey, setSortKey] = useState<SortKey>("index");
  const [sortAsc, setSortAsc] = useState(false);
  const [selected, setSelected] = useState<AuditEvent | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const t = await fetchTotalEvents();
      setTotal(t);
      const evts = await fetchEventPage(page, PAGE_SIZE);
      setEvents(evts);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [page]);

  useEffect(() => { load(); }, [load]);

  const sorted = [...events].sort((a, b) => {
    const av = a[sortKey];
    const bv = b[sortKey];
    const cmp = av < bv ? -1 : av > bv ? 1 : 0;
    return sortAsc ? cmp : -cmp;
  });

  const totalPages = Math.ceil(total / PAGE_SIZE);

  function toggleSort(key: SortKey) {
    if (sortKey === key) setSortAsc((p) => !p);
    else { setSortKey(key); setSortAsc(true); }
  }

  function SortIcon({ k }: { k: SortKey }) {
    if (sortKey !== k) return <span style={{ opacity: 0.3 }}> ↕</span>;
    return <span>{sortAsc ? " ↑" : " ↓"}</span>;
  }

  if (error)
    return (
      <p style={{ color: "var(--error)" }}>Error loading events: {error}</p>
    );

  return (
    <div>
      {/* Toolbar */}
      <div className="flex-between mb-4">
        <p className="text-muted">
          {total} total events · Page {page + 1} of {Math.max(totalPages, 1)}
        </p>
        <div className="flex gap-2">
          <button className="secondary" onClick={() => exportAs(events, "csv")}>
            Export CSV
          </button>
          <button className="secondary" onClick={() => exportAs(events, "json")}>
            Export JSON
          </button>
        </div>
      </div>

      {/* Table */}
      <div className="card" style={{ padding: 0, overflow: "hidden" }}>
        <table>
          <thead>
            <tr>
              <th style={{ cursor: "pointer" }} onClick={() => toggleSort("index")}>
                # <SortIcon k="index" />
              </th>
              <th style={{ cursor: "pointer" }} onClick={() => toggleSort("timestamp")}>
                Timestamp <SortIcon k="timestamp" />
              </th>
              <th style={{ cursor: "pointer" }} onClick={() => toggleSort("event_type")}>
                Type <SortIcon k="event_type" />
              </th>
              <th style={{ cursor: "pointer" }} onClick={() => toggleSort("submitter")}>
                Submitter <SortIcon k="submitter" />
              </th>
              <th>Metadata</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={5} className="text-muted" style={{ textAlign: "center", padding: 32 }}>
                  Loading…
                </td>
              </tr>
            ) : sorted.length === 0 ? (
              <tr>
                <td colSpan={5} className="text-muted" style={{ textAlign: "center", padding: 32 }}>
                  No events on this page.
                </td>
              </tr>
            ) : (
              sorted.map((evt) => (
                <tr
                  key={evt.index}
                  style={{ cursor: "pointer" }}
                  onClick={() => setSelected(evt)}
                >
                  <td>{evt.index}</td>
                  <td>{new Date(evt.timestamp * 1000).toLocaleString()}</td>
                  <td>
                    <span className="badge">{evt.event_type}</span>
                  </td>
                  <td className="mono">{evt.submitter.slice(0, 16)}…</td>
                  <td className="mono">{tryDecodeMetadata(evt.metadata).slice(0, 30)}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="flex-between" style={{ marginTop: 16 }}>
        <button
          className="secondary"
          disabled={page === 0}
          onClick={() => setPage((p) => p - 1)}
        >
          ← Previous
        </button>
        <span className="text-muted">
          {page * PAGE_SIZE + 1}–{Math.min((page + 1) * PAGE_SIZE, total)} of{" "}
          {total}
        </span>
        <button
          className="secondary"
          disabled={page >= totalPages - 1}
          onClick={() => setPage((p) => p + 1)}
        >
          Next →
        </button>
      </div>

      {/* Event detail modal */}
      {selected && (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0,0,0,0.7)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 50,
          }}
          onClick={() => setSelected(null)}
        >
          <div
            className="card"
            style={{ width: 600, maxWidth: "90vw", maxHeight: "80vh", overflowY: "auto" }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex-between mb-4">
              <h2 style={{ fontSize: 18, fontWeight: 700 }}>Event #{selected.index}</h2>
              <button className="secondary" onClick={() => setSelected(null)}>✕</button>
            </div>
            <dl style={{ display: "grid", gridTemplateColumns: "140px 1fr", gap: "8px 16px" }}>
              {(
                [
                  ["Index", String(selected.index)],
                  ["Type", selected.event_type],
                  ["Submitter", selected.submitter],
                  ["Timestamp", new Date(selected.timestamp * 1000).toISOString()],
                  ["Metadata (hex)", selected.metadata],
                  ["Metadata (UTF-8)", tryDecodeMetadata(selected.metadata)],
                  ["Event Hash", selected.event_hash],
                  ["Prev Hash", selected.prev_hash],
                ] as [string, string][]
              ).map(([label, value]) => (
                <>
                  <dt key={`dt-${label}`} className="text-muted text-sm" style={{ alignSelf: "start" }}>
                    {label}
                  </dt>
                  <dd key={`dd-${label}`} className="mono" style={{ wordBreak: "break-all" }}>
                    {value}
                  </dd>
                </>
              ))}
            </dl>
          </div>
        </div>
      )}
    </div>
  );
}
