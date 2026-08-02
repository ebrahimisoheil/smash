"use client";

import { useState } from "react";

import { smashMutations } from "@/lib/api";

type ActionKey = "validate" | "rebuildIndex" | "rebuildBacklinks";

export function StatusActions() {
  const [message, setMessage] = useState<string>("");
  const [pending, setPending] = useState<ActionKey | "">("");

  async function run(action: ActionKey) {
    setPending(action);
    setMessage("");
    try {
      const result =
        action === "validate"
          ? await smashMutations.validate()
          : action === "rebuildIndex"
            ? await smashMutations.rebuildIndex()
            : await smashMutations.rebuildBacklinks();
      setMessage(result.error ? result.error : `${action} completed`);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Action failed");
    } finally {
      setPending("");
    }
  }

  return (
    <div className="action-strip">
      <button disabled={pending !== ""} onClick={() => run("validate")} className="action-button">
        {pending === "validate" ? "Running validation..." : "Validate"}
      </button>
      <button disabled={pending !== ""} onClick={() => run("rebuildIndex")} className="action-button">
        {pending === "rebuildIndex" ? "Rebuilding index..." : "Rebuild index"}
      </button>
      <button disabled={pending !== ""} onClick={() => run("rebuildBacklinks")} className="action-button ghost">
        {pending === "rebuildBacklinks" ? "Rebuilding backlinks..." : "Rebuild backlinks"}
      </button>
      {message ? <p className="inline-feedback">{message}</p> : null}
    </div>
  );
}
