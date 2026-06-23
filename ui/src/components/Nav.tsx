"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";

const NAV = [
  { href: "/", label: "Dashboard" },
  { href: "/explorer", label: "Event Explorer" },
  { href: "/search", label: "Search" },
  { href: "/governance", label: "Governance" },
];

export default function Nav() {
  const path = usePathname();
  return (
    <nav
      style={{
        background: "var(--surface)",
        borderBottom: "1px solid var(--border)",
        padding: "0 24px",
        display: "flex",
        alignItems: "center",
        gap: 32,
        height: 56,
      }}
    >
      <span style={{ fontWeight: 700, color: "var(--accent)", fontSize: 16 }}>
        🔍 AuditLedger
      </span>
      {NAV.map(({ href, label }) => (
        <Link
          key={href}
          href={href}
          style={{
            color: path === href ? "var(--accent)" : "var(--text-muted)",
            fontWeight: path === href ? 600 : 400,
            fontSize: 14,
          }}
        >
          {label}
        </Link>
      ))}
    </nav>
  );
}
