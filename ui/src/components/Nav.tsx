"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { Sun, Moon, Monitor } from "lucide-react";
import { useTheme } from "@/components/ThemeProvider";
import { useState, useCallback, useEffect } from "react";

const NAV = [
  { href: "/", label: "Dashboard" },
  { href: "/explorer", label: "Event Explorer" },
  { href: "/search", label: "Search" },
  { href: "/export", label: "Export" },
  { href: "/compliance", label: "Compliance" },
  { href: "/governance", label: "Governance" },
  { href: "/oracle", label: "Oracle Portal" },
  { href: "/webhooks", label: "Webhooks" },
  { href: "/developer", label: "Developer Portal" },
];

const THEME_ORDER: Array<"dark" | "light" | "system"> = ["dark", "light", "system"];
const THEME_ICONS = { dark: Sun, light: Moon, system: Monitor };
const THEME_LABELS = {
  dark: "Switch to light mode",
  light: "Switch to system mode",
  system: "Switch to dark mode",
};

export default function Nav() {
  const path = usePathname();
  const { preference, setPreference } = useTheme();
  const cycle = () => {
    const idx = THEME_ORDER.indexOf(preference);
    setPreference(THEME_ORDER[(idx + 1) % THEME_ORDER.length]);
  };
  const Icon = THEME_ICONS[preference];
  return (
    <nav className="nav-bar" role="navigation" aria-label="Main navigation">
      <Link href="/" className="nav-brand">
        🔍 AuditLedger
      </Link>

      <button
        onClick={cycle}
        aria-label={THEME_LABELS[preference]}
        className="secondary"
        style={{ marginLeft: "auto", padding: "6px 10px", display: "flex", alignItems: "center" }}
        title={`Theme: ${preference}`}
      >
        <Icon size={16} />
      </button>

      <div
        className={`nav-overlay ${menuOpen ? "visible" : ""}`}
        onClick={closeMenu}
        aria-hidden="true"
      />

      <div
        id="mobile-menu"
        className={`nav-links ${menuOpen ? "open" : ""}`}
        role="menu"
      >
        {NAV.map(({ href, label }) => (
          <Link
            key={href}
            href={href}
            role="menuitem"
            style={{
              color: path === href ? "var(--accent)" : "var(--text-muted)",
              fontWeight: path === href ? 600 : 400,
              fontSize: 14,
            }}
            onClick={closeMenu}
          >
            {label}
          </Link>
        ))}
        <button
          onClick={() => { toggle(); closeMenu(); }}
          aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
          className="secondary"
          role="menuitem"
          style={{ marginLeft: "auto", padding: "6px 10px", display: "flex", alignItems: "center" }}
        >
          {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
        </button>
      </div>
    </nav>
  );
}
