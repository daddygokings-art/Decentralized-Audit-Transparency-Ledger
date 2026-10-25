import Nav from "@/components/Nav";
import OracleClient from "./OracleClient";

export default function OraclePage() {
  return (
    <>
      <Nav />
      <main id="main-content" className="container" style={{ padding: "32px 24px" }}>
        <div style={{ marginBottom: 24 }}>
          <h1 style={{ fontSize: 26, fontWeight: 700, marginBottom: 8 }}>Oracle Submissions & Bonding Portal</h1>
          <p className="text-muted" style={{ maxWidth: 800 }}>
            Participate as a decentralized oracle submitter. Stake collateral bonds, propose verified real-world outcomes
            (compliance audits, price feeds, bridge latencies), monitor active challenge windows, and claim protocol rewards.
          </p>
        </div>
        <OracleClient />
      </main>
    </>
  );
}
