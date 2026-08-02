"use client";

import { useState } from "react";

import { smashMutations } from "@/lib/api";
import type { MemoryInboxPayload } from "@/lib/types";
import { EmptyState, Pill } from "@/components/ui";

export function InboxBoard({ initialInbox }: { initialInbox: MemoryInboxPayload }) {
  const [items, setItems] = useState(initialInbox.items);
  const [busy, setBusy] = useState<string>("");
  const [message, setMessage] = useState<string>("");

  async function markReviewed(identifier: string) {
    setBusy(identifier);
    setMessage("");
    try {
      const result = await smashMutations.reviewMemory(identifier);
      if (result.updated) {
        setItems((current) => current.filter((item) => item.name !== identifier));
        setMessage(`${identifier} reviewed`);
      } else {
        setMessage(result.error ?? "Review failed");
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Review failed");
    } finally {
      setBusy("");
    }
  }

  if (!items.length) {
    return <EmptyState title="Inbox clear" copy="No pending memory review items right now." />;
  }

  return (
    <div className="memory-list">
      {items.map((item) => (
        <article key={item.name} className="memory-card">
          <div className="memory-card-top">
            <div>
              <h3>{item.title}</h3>
              <p className="memory-meta">
                {item.memory_type} · {item.scope} · {item.visibility}
              </p>
            </div>
            <Pill tone={item.review_status === "pending" ? "warn" : "good"}>{item.review_status}</Pill>
          </div>
          <p className="memory-copy">{item.tldr || item.snippet}</p>
          {item.issues?.length ? (
            <ul className="issue-list">
              {item.issues.map((issue) => (
                <li key={issue.code}>
                  <strong>{issue.code}</strong> {issue.message}
                </li>
              ))}
            </ul>
          ) : null}
          <div className="memory-card-actions">
            <button disabled={busy === item.name} className="action-button" onClick={() => markReviewed(item.name)}>
              {busy === item.name ? "Reviewing..." : "Mark reviewed"}
            </button>
            <span className="memory-path">{item.path}</span>
          </div>
        </article>
      ))}
      {message ? <p className="inline-feedback">{message}</p> : null}
    </div>
  );
}
