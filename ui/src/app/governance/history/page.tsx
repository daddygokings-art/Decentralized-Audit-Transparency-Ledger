import Nav from "@/components/Nav";
import Link from "next/link";
import GovernanceHistoryClient from "./GovernanceHistoryClient";

export default function GovernanceHistoryPage() {
  return (
    <>
      <Nav />
      <main className="container" style={{ padding: "32px 24px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 8 }}>
          <h1 style={{ fontSize: 24, fontWeight: 700 }}>Governance History</h1>
          <Link href="/governance" style={{ fontSize: 13, color: "var(--text-muted)" }}>
            ← Back to Governance
          </Link>
        </div>
        <p className="text-muted mb-6">
          On-chain record of ownership transfers, cap changes, and pause events.
        </p>
        <GovernanceHistoryClient />
      </main>
    </>
  );
}
