import { SectionCard } from "@/components/ui";

export function BackendUnavailable({
  title = "Local Smash backend unavailable",
  error,
  baseUrl
}: {
  title?: string;
  error: string;
  baseUrl: string;
}) {
  return (
    <SectionCard
      title={title}
      description="The Next.js shell is running, but it could not reach the local Smash API."
    >
      <div className="backend-alert">
        <p>
          <strong>Error:</strong> {error}
        </p>
        <p>
          <strong>Expected API:</strong> <code>{baseUrl}</code>
        </p>
        <div className="command-list">
          <code>python3 /Users/soheilebrahimi/Documents/smash/serve.py</code>
          <code>python3 /Users/soheilebrahimi/Documents/smash/api.py</code>
        </div>
      </div>
    </SectionCard>
  );
}
