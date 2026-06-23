import Nav from "@/components/Nav";
import DashboardClient from "./DashboardClient";

export default function DashboardPage() {
  return (
    <>
      <Nav />
      <main className="container" style={{ padding: "32px 24px" }}>
        <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>
          Dashboard
        </h1>
        <DashboardClient />
      </main>
    </>
  );
}
