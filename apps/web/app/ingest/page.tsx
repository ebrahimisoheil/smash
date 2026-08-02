import { BackendUnavailable } from "@/components/backend-unavailable";
import { PageIntro, Pill, SectionCard, StatCard } from "@/components/ui";
import { smashApi } from "@/lib/api";

export default async function IngestPage() {
  const ingest = await smashApi.ingestStatusSafe();

  return (
    <div className="page-stack">
      <PageIntro
        eyebrow="Ingest Status"
        title="Source coverage, not guesswork."
        description="The ingestion path stays operational and inspectable: raw files, represented pages, guidance, and next checks."
      />
      {ingest.available ? (
        <>
          <section className="stats-grid">
            <StatCard label="Raw files" value={ingest.data.raw_count} />
            <StatCard label="Source pages" value={ingest.data.source_page_count} tone="accent" />
            <StatCard label="Pending" value={ingest.data.pending_count} tone={ingest.data.pending_count ? "warn" : "good"} />
            <StatCard label="Backlinks" value={ingest.data.backlinks_status} tone={ingest.data.backlinks_status === "current" ? "good" : "warn"} />
          </section>
          <div className="two-up">
            <SectionCard title={ingest.data.plan.title} description={ingest.data.plan.summary}>
              <ul className="simple-list">
                {ingest.data.plan.steps.map((step) => (
                  <li key={step}>{step}</li>
                ))}
              </ul>
            </SectionCard>
            <SectionCard title="Guidance" description={ingest.data.guidance.summary}>
              <div className="command-list">
                {(ingest.data.guidance.commands ?? []).map((command) => (
                  <code key={command}>{command}</code>
                ))}
              </div>
              <div className="pill-row">
                <Pill tone={ingest.data.raw_secret_warning_count ? "warn" : "good"}>{ingest.data.raw_secret_warning_count} secret warnings</Pill>
                <Pill tone={ingest.data.raw_scan_warning_count ? "warn" : "good"}>{ingest.data.raw_scan_warning_count} scan warnings</Pill>
              </div>
            </SectionCard>
          </div>
          <SectionCard title={ingest.data.completion.title} description={ingest.data.completion.summary}>
            <div className="source-grid">
              {ingest.data.completion.items.map((item) => (
                <article key={item.raw} className="source-card">
                  <div className="source-card-top">
                    <h3>{item.raw}</h3>
                    <Pill tone={item.secret_warnings.length ? "warn" : "good"}>{item.size_bytes} bytes</Pill>
                  </div>
                  <p>{item.source_pages.map((page) => page.title).join(", ")}</p>
                  <code>{item.memory_prompt}</code>
                  <code>{item.query_prompt}</code>
                </article>
              ))}
            </div>
          </SectionCard>
        </>
      ) : (
        <BackendUnavailable error={ingest.error} baseUrl={ingest.baseUrl} />
      )}
    </div>
  );
}
