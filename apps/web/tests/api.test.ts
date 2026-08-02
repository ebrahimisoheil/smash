import assert from "node:assert/strict";
import test from "node:test";

import { buildUrl } from "@/lib/api";

test("buildUrl appends query parameters to Smash endpoints", () => {
  const url = buildUrl("/graph-summary", { topic: "agent memory", depth: 1 }, "http://127.0.0.1:3000");
  assert.equal(url, "http://127.0.0.1:3000/api/graph-summary?topic=agent+memory&depth=1");
});
