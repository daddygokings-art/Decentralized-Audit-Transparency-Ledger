import Nav from "@/components/Nav";
import ExplorerClient from "./ExplorerClient";

export default function ExplorerPage() {
  return (
    <>
      <Nav />
      <main className="container" style={{ padding: "32px 24px" }}>
        <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>
          Event Explorer
        </h1>
        <ExplorerClient />
      </main>
    </>
  );
}
