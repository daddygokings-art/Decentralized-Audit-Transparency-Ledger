"use client";
import { useEffect, useState } from "react";

const GRAPHQL_URL =
  process.env.NEXT_PUBLIC_GRAPHQL_URL ?? "http://localhost:4000/graphql";

const GOVERNANCE_TYPES = [
  "transfer_ownership",
  "set_global_max_logs",
  "set_event_max_logs",
  "remove_event_cap",
  "contract_paused",
  "contract_unpaused",
];

interface GovernanceEvent {
  action: string;
  caller: string;
  oldValue?: string;
  newValue?: string;
  timestamp: number;
}

const QUERY = `
  query GovernanceHistory($types: [String!], $limit: Int, $offset: Int) {
    governanceHistory(types: $types, limit: $limit, offset: $offset) {
      action
      caller
      oldValue
      newValue
      timestamp
    }
  }
`;

async function fetchHistory(
  types: string[],
  limit: number,
  offset: number
): Promise<GovernanceEvent[]> {
  const res = await fetch(GRAPHQL_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      query: QUERY,
      variables: { types: types.length ? types : null, limit, offset },
    }),
  });
  const json = await res.json();
  return json.data?.governanceHistory ?? [];
}

function formatTs(ts: number) {
  return new Date(ts * 1000).toLocaleString();
}

const ACTION_LABELS: Record<string, string> = {
  transfer_ownership: "Ownership Transfer",
  set_global_max_logs: "Set Global Max",
  set_event_max_logs: "Set Event Max",
  remove_event_cap: "Remove Event Cap",
  contract_paused: "Contract Paused",
  contract_unpaused: "Contract Unpaused",
};

export default function GovernanceHistoryClient() {
  const [events, setEvents] = useState<GovernanceEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filterTypes, setFilterTypes] = useState<string[]>([]);
  const [page, setPage] = useState(0);
  const PAGE_SIZE = 20;

  useEffect(() => {
    setLoading(true);
    setError(null);
    fetchHistory(filterTypes, PAGE_SIZE, page * PAGE_SIZE)
      .then(setEvents)
      .catch((e) => setError(e.message))
      .finally(() => setLoading(false));
  }, [filterTypes, page]);

  function toggleType(type: string) {
    setFilterTypes((prev) =>
      prev.includes(type) ? prev.filter((t) => t !== type) : [...prev, type]
    );
    setPage(0);
  }

  return (
    <div>
      {/* Filter bar */}
      <div className="card mb-6" style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
        {GOVERNANCE_TYPES.map((t) => (
          <button
            key={t}
            onClick={() => toggleType(t)}
            style={{
              background: filterTypes.includes(t) ? "var(--accent)" : "var(--surface-raised)",
              color: filterTypes.includes(t) ? "#fff" : "var(--text-muted)",
              fontSize: 12,
              padding: "4px 10px",
            }}
          >
            {ACTION_LABELS[t] ?? t}
          </button>
        ))}
        {filterTypes.length > 0 && (
          <button
            onClick={() => { setFilterTypes([]); setPage(0); }}
            style={{ fontSize: 12, padding: "4px 10px" }}
          >
            Clear
          </button>
        )}
      </div>

      {/* Table */}
      {loading ? (
        <p className="text-muted">Loading…</p>
      ) : error ? (
        <p style={{ color: "var(--error)" }}>{error}</p>
      ) : events.length === 0 ? (
        <p className="text-muted">No governance events found.</p>
      ) : (
        <div className="card" style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left" }}>
                <th style={{ padding: "8px 12px" }}>Action</th>
                <th style={{ padding: "8px 12px" }}>Caller</th>
                <th style={{ padding: "8px 12px" }}>Old Value</th>
                <th style={{ padding: "8px 12px" }}>New Value</th>
                <th style={{ padding: "8px 12px" }}>Timestamp</th>
              </tr>
            </thead>
            <tbody>
              {events.map((ev, i) => (
                <tr
                  key={i}
                  style={{ borderBottom: "1px solid var(--border)", verticalAlign: "top" }}
                >
                  <td style={{ padding: "8px 12px", fontWeight: 600 }}>
                    {ACTION_LABELS[ev.action] ?? ev.action}
                  </td>
                  <td
                    style={{
                      padding: "8px 12px",
                      fontFamily: "monospace",
                      fontSize: 11,
                      wordBreak: "break-all",
                      maxWidth: 200,
                    }}
                  >
                    {ev.caller}
                  </td>
                  <td style={{ padding: "8px 12px", color: "var(--text-muted)" }}>
                    {ev.oldValue ?? "—"}
                  </td>
                  <td style={{ padding: "8px 12px" }}>{ev.newValue ?? "—"}</td>
                  <td style={{ padding: "8px 12px", color: "var(--text-muted)", whiteSpace: "nowrap" }}>
                    {formatTs(ev.timestamp)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Pagination */}
      <div style={{ display: "flex", gap: 12, marginTop: 16 }}>
        <button onClick={() => setPage((p) => Math.max(0, p - 1))} disabled={page === 0}>
          ← Prev
        </button>
        <span style={{ alignSelf: "center", fontSize: 13 }}>Page {page + 1}</span>
        <button
          onClick={() => setPage((p) => p + 1)}
          disabled={events.length < PAGE_SIZE}
        >
          Next →
        </button>
      </div>
    </div>
  );
}
