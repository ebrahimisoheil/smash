import type { Route } from "next";
import Link from "next/link";
import React, { ReactNode } from "react";

const navItems = [
  { href: "/", label: "Status" },
  { href: "/inbox", label: "Inbox" },
  { href: "/ingest", label: "Ingest" },
  { href: "/graph", label: "Graph" },
  { href: "/onboard", label: "Onboard" },
  { href: "/brief", label: "Brief" }
] satisfies Array<{ href: Route; label: string }>;

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="topbar-brand">
          <span className="brand-kicker">Phase 3</span>
          <Link className="brand-mark" href="/">
            Smash Control
          </Link>
          <p className="brand-copy">
            Same operational structure, rebuilt as a sturdier local dashboard.
          </p>
        </div>
        <nav className="topbar-nav" aria-label="Primary">
          {navItems.map((item) => (
            <Link key={item.href} href={item.href} className="rail-link">
              <span>{item.label}</span>
            </Link>
          ))}
        </nav>
        <div className="topbar-foot">
          <span className="rail-foot-label">Local API</span>
          <span className="rail-foot-value">127.0.0.1 only</span>
        </div>
      </header>
      <main className="surface page-shell">{children}</main>
    </div>
  );
}
