import { BackendUnavailable } from "@/components/backend-unavailable";
import { GraphExplorer } from "@/components/graph-explorer";
import { PageIntro, SectionCard } from "@/components/ui";
import { smashApi } from "@/lib/api";

export default async function GraphPage() {
  const graph = await smashApi.graphSummarySafe("Smash", 1);

  return (
    <div className="page-stack">
      <PageIntro
        eyebrow="Graph Explorer"
        title="Inspect the graph, not just the picture."
        description="Same bounded-first graph structure, but with searchable nodes, real context, connected-page inspection, and a focused graph map."
      />
      {graph.available ? (
        <SectionCard title="Knowledge graph" description="Start from a bounded summary, inspect a node, then escalate to full graph only when needed.">
          <GraphExplorer initialGraph={graph.data} />
        </SectionCard>
      ) : (
        <BackendUnavailable error={graph.error} baseUrl={graph.baseUrl} />
      )}
    </div>
  );
}
