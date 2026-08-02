import { BackendUnavailable } from "@/components/backend-unavailable";
import { InboxBoard } from "@/components/inbox-board";
import { PageIntro, SectionCard, StatCard } from "@/components/ui";
import { smashApi } from "@/lib/api";

export default async function InboxPage() {
  const inbox = await smashApi.memoryInboxSafe();

  return (
    <div className="page-stack">
      <PageIntro
        eyebrow="Memory Inbox"
        title="Review before recall drifts."
        description="Pending memory items stay central. The design changes here, not the governance model."
      />
      {inbox.available ? (
        <>
          <section className="stats-grid">
            <StatCard label="Pending" value={inbox.data.review_count} tone={inbox.data.review_count ? "warn" : "good"} />
            <StatCard label="High severity" value={inbox.data.counts_by_severity.high ?? 0} tone="warn" />
            <StatCard label="Medium severity" value={inbox.data.counts_by_severity.medium ?? 0} />
            <StatCard label="Archived included" value={inbox.data.include_archived ? "Yes" : "No"} />
          </section>
          <SectionCard title="Review queue" description="Confirm, then clear the queue without leaving the app.">
            <InboxBoard initialInbox={inbox.data} />
          </SectionCard>
        </>
      ) : (
        <BackendUnavailable error={inbox.error} baseUrl={inbox.baseUrl} />
      )}
    </div>
  );
}
