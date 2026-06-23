import Nav from "@/components/Nav";
import GovernanceClient from "./GovernanceClient";

export default function GovernancePage() {
  return (
    <>
      <Nav />
      <main className="container" style={{ padding: "32px 24px" }}>
        <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 8 }}>
          Governance
        </h1>
        <p className="text-muted mb-6">
          Owner-only actions. Connect a wallet to sign transactions.
        </p>
        <GovernanceClient />
      </main>
    </>
  );
}
