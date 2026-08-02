export default function Home() {
  return (
    <main>
      <h1>SMASH V2</h1>
      <p>Source processing is explicit and never presented as durable Memory.</p>
      <section aria-labelledby="processing-heading">
        <h2 id="processing-heading">Processing states</h2>
        <ul>
          <li>Queued — waiting for a worker</li>
          <li>Extracting / Chunking / Indexing — work in progress</li>
          <li>Ready — searchable source evidence</li>
          <li>Partially ready — usable with an outstanding step</li>
          <li>Failed or Quarantined — actionable review required</li>
        </ul>
      </section>
      <p>Processor output creates artifacts and proposals; it does not activate Memory.</p>
    </main>
  );
}
