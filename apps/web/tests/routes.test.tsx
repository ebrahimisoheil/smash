import assert from "node:assert/strict";
import test from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { AppShell } from "@/components/app-shell";
import { EmptyState, PageIntro, SectionCard, StatCard } from "@/components/ui";

test("shell and core UI primitives render", () => {
  const html = renderToStaticMarkup(
    <AppShell>
      <PageIntro eyebrow="Status" title="Title" description="Description" />
      <SectionCard title="Section">
        <StatCard label="Ready" value="Yes" />
        <EmptyState title="Empty" copy="Nothing here." />
      </SectionCard>
    </AppShell>
  );

  assert.match(html, /Smash Control/);
  assert.match(html, /Title/);
  assert.match(html, /Nothing here/);
});
