import { BackendUnavailable } from "@/components/backend-unavailable";
import { StatusActions } from "@/components/status-actions";
import { KeyValueList, PageIntro, SectionCard, StatCard } from "@/components/ui";
import { smashApi } from "@/lib/api";

export default async function HomePage() {
  const [status, health, profile] = await Promise.all([
    smashApi.statusSafe(),
    smashApi.healthSafe(),
    smashApi.memoryProfileSafe()
  ]);

  return (
    <div className="page-stack">
      <PageIntro
        eyebrow="Status Dashboard"
        title="Operational clarity first."
        description="This parallel surface keeps Smash’s workflow structure intact, but presents readiness, operations, and memory posture as a faster control room."
      >
        <StatusActions />
      </PageIntro>

      {!status.available ? <BackendUnavailable error={status.error} baseUrl={status.baseUrl} /> : null}
      {!health.available && status.available ? <BackendUnavailable error={health.error} baseUrl={health.baseUrl} /> : null}
      {!profile.available && status.available && health.available ? (
        <BackendUnavailable error={profile.error} baseUrl={profile.baseUrl} />
      ) : null}

      {status.available && health.available && profile.available ? (
        <>
          <section className="stats-grid">
            <StatCard label="Ready" value={status.data.ready ? "Yes" : "No"} tone={status.data.ready ? "good" : "warn"} detail={`Smash ${status.data.version}`} />
            <StatCard label="Pages" value={status.data.page_count} tone="accent" detail="Inspectable wiki pages" />
            <StatCard label="Memories" value={status.data.memory_count} detail={`${status.data.active_memory_count} active`} />
            <StatCard
              label="Needs review"
              value={status.data.needs_review_count}
              tone={status.data.needs_review_count ? "warn" : "good"}
              detail={`${profile.data.top_tags.length} visible top tags`}
            />
          </section>

          <div className="two-up">
            <SectionCard title="Validation" description="Current integrity and local operational safety.">
              <KeyValueList
                items={[
                  { label: "Passed", value: health.data.status.validation?.passed ? "Yes" : "No" },
                  { label: "Errors", value: health.data.status.validation?.error_count ?? 0 },
                  { label: "Warnings", value: health.data.status.validation?.warning_count ?? 0 }
                ]}
              />
            </SectionCard>
            <SectionCard title="Operations" description="Interrupted or stale work is surfaced here immediately.">
              <KeyValueList
                items={[
                  { label: "Active", value: health.data.operations.active_count },
                  { label: "Failed", value: health.data.operations.failed_count },
                  { label: "Stale", value: health.data.operations.stale_count }
                ]}
              />
            </SectionCard>
          </div>

          <SectionCard title="Memory shape" description="A quick summary of what the current workspace remembers.">
            <div className="tag-grid">
              {Object.entries(profile.data.by_type).map(([label, count]) => (
                <div key={label} className="tag-card">
                  <span>{label}</span>
                  <strong>{count}</strong>
                </div>
              ))}
            </div>
          </SectionCard>
          <SectionCard title="Next actions" description="The same bounded operating loop the current Smash surfaces recommend.">
            <div className="command-list">
              {(status.data.next_actions ?? []).map((action) => (
                <code key={action.label}>
                  {action.label}
                  {action.tool ? ` → ${action.tool}` : ""}
                </code>
              ))}
            </div>
          </SectionCard>
        </>
      ) : null}
    </div>
  );
}
