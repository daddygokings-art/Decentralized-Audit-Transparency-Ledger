import Nav from "@/components/Nav";
import SearchClient from "./SearchClient";

export default function SearchPage() {
  return (
    <>
      <Nav />
      <main className="container" style={{ padding: "32px 24px" }}>
        <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>
          Search Events
        </h1>
        <SearchClient />
      </main>
    </>
  );
}
