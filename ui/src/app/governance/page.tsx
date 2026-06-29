import Nav from "@/components/Nav";
import Link from "next/link";
import GovernanceClient from "./GovernanceClient";

export default function GovernancePage() {
  return (
    <>
      <Nav />
      <main className="container" style={{ padding: "32px 24px" }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
          <h1 style={{ fontSize: 24, fontWeight: 700 }}>Governance</h1>
          <Link
            href="/governance/history"
            style={{ fontSize: 13, color: "var(--accent)" }}
          >
            View History →
          </Link>
        </div>
        <p className="text-muted mb-6">
          Owner-only actions. Connect a wallet to sign transactions.
        </p>
        <GovernanceClient />
      </main>
    </>
  );
}
