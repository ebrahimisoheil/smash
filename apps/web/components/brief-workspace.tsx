"use client";

import { FormEvent, useState } from "react";

import { clientGet } from "@/lib/api";
import type { MemoryBriefPayload } from "@/lib/types";
import { EmptyState, Pill } from "@/components/ui";

export function BriefWorkspace({ initialBrief }: { initialBrief: MemoryBriefPayload }) {
  const [query, setQuery] = useState(initialBrief.query);
  const [brief, setBrief] = useState(initialBrief);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setLoading(true);
    setError("");
    try {
      const next = await clientGet<MemoryBriefPayload>("/memory-brief", { q: query, limit: 8 });
      setBrief(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Query failed");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="brief-workspace">
      <form className="query-bar" onSubmit={handleSubmit}>
        <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Ask for a bounded memory packet" />
        <button className="action-button" disabled={loading}>
          {loading ? "Loading..." : "Refresh brief"}
        </button>
      </form>
      {error ? <p className="inline-feedback">{error}</p> : null}
      <div className="brief-summary">
        <Pill tone="accent">{brief.selection}</Pill>
        <Pill tone={brief.review?.count ? "warn" : "good"}>{brief.review?.count ?? 0} pending review</Pill>
        <Pill tone={brief.captures?.warning_count ? "warn" : "default"}>{brief.captures?.warning_count ?? 0} capture warnings</Pill>
      </div>
      {brief.relevant_memories.length ? (
        <div className="memory-list">
          {brief.relevant_memories.map((item) => (
            <article key={item.name} className="memory-card">
              <div className="memory-card-top">
                <div>
                  <h3>{item.title}</h3>
                  <p className="memory-meta">
                    {item.memory_type} · {item.scope}
                  </p>
                </div>
                <Pill tone={item.review_status === "pending" ? "warn" : "good"}>{item.review_status}</Pill>
              </div>
              <p className="memory-copy">{item.tldr}</p>
              <p className="memory-snippet">{item.snippet}</p>
            </article>
          ))}
        </div>
      ) : (
        <EmptyState title="No relevant memories" copy="Try a broader phrase or seed more source-backed context first." />
      )}
    </div>
  );
}
