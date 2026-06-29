"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { Sun, Moon } from "lucide-react";
import { useTheme } from "@/components/ThemeProvider";

const NAV = [
  { href: "/", label: "Dashboard" },
  { href: "/explorer", label: "Event Explorer" },
  { href: "/search", label: "Search" },
  { href: "/governance", label: "Governance" },
];

export default function Nav() {
  const path = usePathname();
  const { theme, toggle } = useTheme();
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
      <button
        onClick={toggle}
        aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
        className="secondary"
        style={{ marginLeft: "auto", padding: "6px 10px", display: "flex", alignItems: "center" }}
      >
        {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
      </button>
    </nav>
  );
}
