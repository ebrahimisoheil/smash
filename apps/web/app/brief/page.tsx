import { BackendUnavailable } from "@/components/backend-unavailable";
import { BriefWorkspace } from "@/components/brief-workspace";
import { PageIntro, SectionCard } from "@/components/ui";
import { smashApi } from "@/lib/api";

export default async function BriefPage() {
  const brief = await smashApi.memoryBriefSafe("local memory");

  return (
    <div className="page-stack">
      <PageIntro
        eyebrow="Brief Workspace"
        title="Bounded retrieval, workspace style."
        description="This keeps the brief/query step close to the rest of the operational UI instead of burying it in a docs-style page."
      />
      {brief.available ? (
        <SectionCard title="Memory brief" description="Search a topic and inspect the compact memory packet returned by Smash.">
          <BriefWorkspace initialBrief={brief.data} />
        </SectionCard>
      ) : (
        <BackendUnavailable error={brief.error} baseUrl={brief.baseUrl} />
      )}
    </div>
  );
}
