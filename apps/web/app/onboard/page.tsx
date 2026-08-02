import { BackendUnavailable } from "@/components/backend-unavailable";
import { PageIntro, SectionCard } from "@/components/ui";
import { smashApi } from "@/lib/api";

export default async function OnboardPage() {
  const [status, prompts, ingest] = await Promise.all([
    smashApi.statusSafe(),
    smashApi.promptsSafe(),
    smashApi.ingestStatusSafe()
  ]);
  const backendFailure = !status.available
    ? status
    : !prompts.available
      ? prompts
      : !ingest.available
        ? ingest
        : null;

  return (
    <div className="page-stack">
      <PageIntro
        eyebrow="Onboarding"
        title="Get the workspace into a usable state fast."
        description="This page keeps the existing first-run structure: readiness, source seeding, starter prompts, and the next operational commands."
      />
      {status.available && prompts.available && ingest.available ? (
        <>
          <div className="two-up">
            <SectionCard
              title="Readiness"
              description={status.data.ready ? "Workspace is usable now." : "Resolve missing requirements before relying on recall."}
            >
              <ul className="simple-list">
                <li>{status.data.page_count} pages indexed</li>
                <li>{status.data.memory_count} memories available</li>
                <li>{status.data.needs_review_count} items still need review</li>
              </ul>
            </SectionCard>
            <SectionCard title="Source plan" description={ingest.data.plan.summary}>
              <ul className="simple-list">
                {ingest.data.plan.steps.map((step) => (
                  <li key={step}>{step}</li>
                ))}
              </ul>
            </SectionCard>
          </div>
          <SectionCard title="Starter prompts" description="Use these to start memory-aware work without opening docs pages first.">
            <div className="prompt-grid">
              {(prompts.data.prompts ?? []).map((prompt) => (
                <article key={`${prompt.label}-${prompt.prompt}`} className="prompt-card">
                  <span className="eyebrow">prompt</span>
                  <h3>{prompt.label}</h3>
                  <code>{prompt.prompt}</code>
                  {prompt.when ? <p className="prompt-when">{prompt.when}</p> : null}
                </article>
              ))}
            </div>
          </SectionCard>
        </>
      ) : (
        backendFailure && <BackendUnavailable error={backendFailure.error} baseUrl={backendFailure.baseUrl} />
      )}
    </div>
  );
}
