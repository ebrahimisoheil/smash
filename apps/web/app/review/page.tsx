const proposals = [
  {
    id: "proposal-demo-1",
    claim: "Use UTC for all durable timestamps.",
    state: "pending_review",
    policy: "Personal-Area confirmation",
    evidence: "Source chunk · line=1;char=0..42",
    conflicts: "No duplicate or contradiction candidates",
  },
  {
    id: "proposal-demo-2",
    claim: "Shared customer facts require independent review.",
    state: "approval_required",
    policy: "Shared/Cross-Map · independent reviewer",
    evidence: "Source chunk · line=4;char=12..78",
    conflicts: "1 contradiction candidate",
  },
];

export default function ReviewPage() {
  return (
    <main>
      <header>
        <p>ENGRAVE V2 · Review</p>
        <h1>Nothing becomes durable silently.</h1>
        <p>
          Proposals are observations until a reviewer confirms the exact claim,
          reason, scope, applicability, evidence, and conflicts.
        </p>
      </header>
      <section aria-labelledby="queue-heading">
        <h2 id="queue-heading">Review queue</h2>
        <div>
          {proposals.map((proposal) => (
            <article key={proposal.id} aria-labelledby={`${proposal.id}-claim`}>
              <p><strong>{proposal.state.replace("_", " ")}</strong></p>
              <h3 id={`${proposal.id}-claim`}>{proposal.claim}</h3>
              <dl>
                <dt>Admission</dt><dd>{proposal.policy}</dd>
                <dt>Evidence</dt><dd>{proposal.evidence}</dd>
                <dt>Review signals</dt><dd>{proposal.conflicts}</dd>
              </dl>
              <p>Actions are explicit: approve, edit, reject, or defer. A stale review is rejected and returns the current version.</p>
              <button type="button">Open review</button>
            </article>
          ))}
        </div>
      </section>
      <section aria-labelledby="lifecycle-heading">
        <h2 id="lifecycle-heading">Lifecycle states</h2>
        <p>Proposed → Active → Archived / Restored / Expired / Superseded. Supersession preserves lineage; archived and expired records are not active.</p>
      </section>
    </main>
  );
}
