"use client";

import React, { useState } from "react";
import { CheckCircle2, AlertTriangle, ShieldCheck, Clock, Award, Send, RefreshCw, Lock } from "lucide-react";

interface OracleRequest {
  id: string;
  title: string;
  feedType: "ComplianceAudit" | "BridgeLatency" | "EventVolume" | "CustomFeed";
  requiredBond: number;
  rewardPool: number;
  status: "Open" | "InChallenge" | "Resolved";
  deadlineLedger: number;
  currentResolution?: string;
}

interface UserSubmission {
  id: string;
  requestId: string;
  marketTitle: string;
  proposedValue: string;
  bondLocked: number;
  challengeWindowEnds: string;
  status: "PendingChallenge" | "Finalized" | "Challenged";
  timestamp: string;
}

const INITIAL_REQUESTS: OracleRequest[] = [
  {
    id: "req-101",
    title: "SOC 2 Type II Annual Compliance Audit Verification",
    feedType: "ComplianceAudit",
    requiredBond: 1000,
    rewardPool: 250,
    status: "Open",
    deadlineLedger: 489200,
  },
  {
    id: "req-102",
    title: "Stellar-Ethereum Bridge Latency Under 1500ms Threshold",
    feedType: "BridgeLatency",
    requiredBond: 1500,
    rewardPool: 320,
    status: "Open",
    deadlineLedger: 489500,
  },
  {
    id: "req-103",
    title: "Carbon Credit Retirement Event Volume Verification",
    feedType: "EventVolume",
    requiredBond: 2000,
    rewardPool: 500,
    status: "InChallenge",
    deadlineLedger: 490100,
    currentResolution: "Exceeded (125,000 credits)",
  },
];

const INITIAL_SUBMISSIONS: UserSubmission[] = [
  {
    id: "sub-901",
    requestId: "req-098",
    marketTitle: "ISO 27001 Re-certification Ledger Proof",
    proposedValue: "Passed (Audit Hash: 0x8a9f...)",
    bondLocked: 1000,
    challengeWindowEnds: "42 hours remaining",
    status: "PendingChallenge",
    timestamp: "2026-10-21 04:15 UTC",
  },
  {
    id: "sub-889",
    requestId: "req-085",
    marketTitle: "Bridge Relayer Uptime Q3 Invariant Check",
    proposedValue: "99.98% SLA Met",
    bondLocked: 1200,
    challengeWindowEnds: "Finalized",
    status: "Finalized",
    timestamp: "2026-10-18 12:00 UTC",
  },
];

export default function OracleClient() {
  const [requests, setRequests] = useState<OracleRequest[]>(INITIAL_REQUESTS);
  const [submissions, setSubmissions] = useState<UserSubmission[]>(INITIAL_SUBMISSIONS);
  const [selectedReq, setSelectedReq] = useState<OracleRequest | null>(null);
  const [proposedValue, setProposedValue] = useState("");
  const [evidenceUri, setEvidenceUri] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Submitter Profile Stats
  const stakedCollateral = submissions.reduce((acc, s) => s.status === "PendingChallenge" ? acc + s.bondLocked : acc, 0);
  const earnedRewards = 820; // AUDIT
  const reputationScore = 98.5; // percentage

  const handleOpenSubmit = (req: OracleRequest) => {
    setSelectedReq(req);
    setProposedValue("");
    setEvidenceUri("");
    setSuccessMsg(null);
    setErrorMsg(null);
  };

  const handleCloseModal = () => {
    setSelectedReq(null);
  };

  const handleSubmitOutcome = (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedReq) return;
    if (!proposedValue.trim()) {
      setErrorMsg("Please provide a proposed outcome or value.");
      return;
    }

    setIsSubmitting(true);
    setErrorMsg(null);

    setTimeout(() => {
      const newSub: UserSubmission = {
        id: `sub-${Math.floor(1000 + Math.random() * 9000)}`,
        requestId: selectedReq.id,
        marketTitle: selectedReq.title,
        proposedValue: proposedValue.trim(),
        bondLocked: selectedReq.requiredBond,
        challengeWindowEnds: "72 hours remaining (43,200 ledgers)",
        status: "PendingChallenge",
        timestamp: "Just now",
      };

      setSubmissions([newSub, ...submissions]);
      setRequests(requests.map((r) => r.id === selectedReq.id ? { ...r, status: "InChallenge", currentResolution: proposedValue.trim() } : r));
      setIsSubmitting(false);
      setSelectedReq(null);
      setSuccessMsg(`Outcome successfully submitted for "${selectedReq.title}". Bond of ${selectedReq.requiredBond} AUDIT locked in escrow.`);
    }, 600);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
      {/* Notifications */}
      {successMsg && (
        <div style={{ padding: "12px 16px", borderRadius: 8, background: "rgba(34, 197, 94, 0.15)", border: "1px solid rgba(34, 197, 94, 0.4)", display: "flex", alignItems: "center", gap: 10 }}>
          <CheckCircle2 size={18} color="var(--success, #22c55e)" />
          <span>{successMsg}</span>
        </div>
      )}
      {errorMsg && (
        <div style={{ padding: "12px 16px", borderRadius: 8, background: "rgba(239, 68, 68, 0.15)", border: "1px solid rgba(239, 68, 68, 0.4)", display: "flex", alignItems: "center", gap: 10 }}>
          <AlertTriangle size={18} color="var(--danger, #ef4444)" />
          <span>{errorMsg}</span>
        </div>
      )}

      {/* Submitter Overview Stats */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: 16 }}>
        <div style={{ padding: 16, borderRadius: 8, background: "var(--bg-card, #1e293b)", border: "1px solid var(--border, #334155)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--text-muted, #94a3b8)", fontSize: 13, marginBottom: 4 }}>
            <Lock size={15} /> Locked Submitter Bond
          </div>
          <div style={{ fontSize: 22, fontWeight: 700 }}>{stakedCollateral.toLocaleString()} AUDIT</div>
        </div>

        <div style={{ padding: 16, borderRadius: 8, background: "var(--bg-card, #1e293b)", border: "1px solid var(--border, #334155)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--text-muted, #94a3b8)", fontSize: 13, marginBottom: 4 }}>
            <Award size={15} /> Total Rewards Earned
          </div>
          <div style={{ fontSize: 22, fontWeight: 700, color: "var(--accent, #38bdf8)" }}>{earnedRewards.toLocaleString()} AUDIT</div>
        </div>

        <div style={{ padding: 16, borderRadius: 8, background: "var(--bg-card, #1e293b)", border: "1px solid var(--border, #334155)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--text-muted, #94a3b8)", fontSize: 13, marginBottom: 4 }}>
            <ShieldCheck size={15} /> Submitter Reputation
          </div>
          <div style={{ fontSize: 22, fontWeight: 700, color: "var(--success, #22c55e)" }}>{reputationScore}%</div>
        </div>

        <div style={{ padding: 16, borderRadius: 8, background: "var(--bg-card, #1e293b)", border: "1px solid var(--border, #334155)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, color: "var(--text-muted, #94a3b8)", fontSize: 13, marginBottom: 4 }}>
            <Clock size={15} /> Active Submissions
          </div>
          <div style={{ fontSize: 22, fontWeight: 700 }}>{submissions.filter((s) => s.status === "PendingChallenge").length}</div>
        </div>
      </div>

      {/* Open Oracle Requests */}
      <section style={{ padding: 20, borderRadius: 8, background: "var(--bg-card, #1e293b)", border: "1px solid var(--border, #334155)" }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 12 }}>Open Oracle Resolution Requests</h2>
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 14 }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border, #334155)", textAlign: "left", color: "var(--text-muted, #94a3b8)" }}>
                <th style={{ padding: "8px 12px" }}>Feed ID / Title</th>
                <th style={{ padding: "8px 12px" }}>Type</th>
                <th style={{ padding: "8px 12px" }}>Required Bond</th>
                <th style={{ padding: "8px 12px" }}>Reward</th>
                <th style={{ padding: "8px 12px" }}>Status</th>
                <th style={{ padding: "8px 12px", textAlign: "right" }}>Action</th>
              </tr>
            </thead>
            <tbody>
              {requests.map((req) => (
                <tr key={req.id} style={{ borderBottom: "1px solid var(--border, #334155)" }}>
                  <td style={{ padding: "12px 12px", fontWeight: 500 }}>
                    <div>{req.title}</div>
                    <div style={{ fontSize: 12, color: "var(--text-muted, #94a3b8)" }}>ID: {req.id}</div>
                  </td>
                  <td style={{ padding: "12px 12px" }}>
                    <span style={{ padding: "2px 8px", borderRadius: 4, background: "rgba(56, 189, 248, 0.15)", color: "var(--accent, #38bdf8)", fontSize: 12 }}>
                      {req.feedType}
                    </span>
                  </td>
                  <td style={{ padding: "12px 12px" }}>{req.requiredBond} AUDIT</td>
                  <td style={{ padding: "12px 12px", color: "var(--success, #22c55e)", fontWeight: 600 }}>+{req.rewardPool} AUDIT</td>
                  <td style={{ padding: "12px 12px" }}>
                    {req.status === "Open" ? (
                      <span style={{ color: "var(--accent, #38bdf8)" }}>Awaiting Outcome</span>
                    ) : (
                      <span style={{ color: "#eab308" }}>Challenge Window Active</span>
                    )}
                  </td>
                  <td style={{ padding: "12px 12px", textAlign: "right" }}>
                    <button
                      onClick={() => handleOpenSubmit(req)}
                      disabled={req.status !== "Open"}
                      style={{
                        padding: "6px 14px",
                        borderRadius: 6,
                        background: req.status === "Open" ? "var(--accent, #38bdf8)" : "transparent",
                        color: req.status === "Open" ? "#000" : "var(--text-muted, #64748b)",
                        border: "1px solid var(--border, #334155)",
                        fontWeight: 600,
                        cursor: req.status === "Open" ? "pointer" : "not-allowed",
                      }}
                    >
                      Propose Outcome
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {/* Submitter History & Challenge Window Tracker */}
      <section style={{ padding: 20, borderRadius: 8, background: "var(--bg-card, #1e293b)", border: "1px solid var(--border, #334155)" }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 12 }}>Your Oracle Submissions & Challenge Windows</h2>
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 14 }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border, #334155)", textAlign: "left", color: "var(--text-muted, #94a3b8)" }}>
                <th style={{ padding: "8px 12px" }}>Market</th>
                <th style={{ padding: "8px 12px" }}>Proposed Value</th>
                <th style={{ padding: "8px 12px" }}>Locked Bond</th>
                <th style={{ padding: "8px 12px" }}>Challenge Window</th>
                <th style={{ padding: "8px 12px" }}>Status</th>
              </tr>
            </thead>
            <tbody>
              {submissions.map((sub) => (
                <tr key={sub.id} style={{ borderBottom: "1px solid var(--border, #334155)" }}>
                  <td style={{ padding: "12px 12px", fontWeight: 500 }}>
                    <div>{sub.marketTitle}</div>
                    <div style={{ fontSize: 12, color: "var(--text-muted, #94a3b8)" }}>{sub.timestamp}</div>
                  </td>
                  <td style={{ padding: "12px 12px" }}>{sub.proposedValue}</td>
                  <td style={{ padding: "12px 12px" }}>{sub.bondLocked} AUDIT</td>
                  <td style={{ padding: "12px 12px" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <Clock size={14} color="#eab308" />
                      <span>{sub.challengeWindowEnds}</span>
                    </div>
                  </td>
                  <td style={{ padding: "12px 12px" }}>
                    {sub.status === "PendingChallenge" ? (
                      <span style={{ padding: "2px 8px", borderRadius: 4, background: "rgba(234, 179, 8, 0.15)", color: "#eab308", fontSize: 12 }}>
                        Unchallenged
                      </span>
                    ) : (
                      <span style={{ padding: "2px 8px", borderRadius: 4, background: "rgba(34, 197, 94, 0.15)", color: "var(--success, #22c55e)", fontSize: 12 }}>
                        Settled & Rewarded
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {/* Propose Outcome Modal */}
      {selectedReq && (
        <div style={{
          position: "fixed",
          inset: 0,
          background: "rgba(0, 0, 0, 0.7)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          zIndex: 50,
          padding: 16,
        }}>
          <div style={{
            background: "var(--bg-card, #1e293b)",
            border: "1px solid var(--border, #334155)",
            borderRadius: 12,
            padding: 24,
            maxWidth: 520,
            width: "100%",
          }}>
            <h3 style={{ fontSize: 18, fontWeight: 700, marginBottom: 8 }}>Propose Oracle Outcome</h3>
            <p style={{ fontSize: 13, color: "var(--text-muted, #94a3b8)", marginBottom: 16 }}>
              {selectedReq.title}
            </p>

            <form onSubmit={handleSubmitOutcome} style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              <div>
                <label style={{ display: "block", fontSize: 13, fontWeight: 500, marginBottom: 4 }}>
                  Verified Outcome / Data Point
                </label>
                <input
                  type="text"
                  placeholder="e.g. Audit Passed (Hash: 0x...), 1240ms, etc."
                  value={proposedValue}
                  onChange={(e) => setProposedValue(e.target.value)}
                  style={{
                    width: "100%",
                    padding: "8px 12px",
                    borderRadius: 6,
                    background: "rgba(0, 0, 0, 0.2)",
                    border: "1px solid var(--border, #334155)",
                    color: "inherit",
                  }}
                  required
                />
              </div>

              <div>
                <label style={{ display: "block", fontSize: 13, fontWeight: 500, marginBottom: 4 }}>
                  Evidence URI / IPFS Hash
                </label>
                <input
                  type="text"
                  placeholder="ipfs://bafy... or https://audit.example.org/report.pdf"
                  value={evidenceUri}
                  onChange={(e) => setEvidenceUri(e.target.value)}
                  style={{
                    width: "100%",
                    padding: "8px 12px",
                    borderRadius: 6,
                    background: "rgba(0, 0, 0, 0.2)",
                    border: "1px solid var(--border, #334155)",
                    color: "inherit",
                  }}
                />
              </div>

              <div style={{
                padding: 12,
                borderRadius: 8,
                background: "rgba(56, 189, 248, 0.1)",
                border: "1px solid rgba(56, 189, 248, 0.3)",
                fontSize: 13,
              }}>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 4 }}>
                  <span>Required Submitter Bond:</span>
                  <strong>{selectedReq.requiredBond} AUDIT</strong>
                </div>
                <div style={{ display: "flex", justifyContent: "space-between", color: "var(--success, #22c55e)" }}>
                  <span>Eligible Reward upon Settlement:</span>
                  <strong>+{selectedReq.rewardPool} AUDIT</strong>
                </div>
                <div style={{ fontSize: 12, color: "var(--text-muted, #94a3b8)", marginTop: 6 }}>
                  * Bond is locked in escrow during the 72-hour challenge window. If no dispute is raised, bond + reward are automatically released.
                </div>
              </div>

              <div style={{ display: "flex", justifyContent: "flex-end", gap: 10, marginTop: 10 }}>
                <button
                  type="button"
                  onClick={handleCloseModal}
                  style={{
                    padding: "8px 16px",
                    borderRadius: 6,
                    background: "transparent",
                    border: "1px solid var(--border, #334155)",
                    color: "inherit",
                    cursor: "pointer",
                  }}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={isSubmitting}
                  style={{
                    padding: "8px 18px",
                    borderRadius: 6,
                    background: "var(--accent, #38bdf8)",
                    border: "none",
                    color: "#000",
                    fontWeight: 600,
                    cursor: isSubmitting ? "not-allowed" : "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                  }}
                >
                  {isSubmitting ? <RefreshCw size={14} className="animate-spin" /> : <Send size={14} />}
                  Deposit Bond & Submit
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
