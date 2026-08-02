import { promises as fs } from "fs";
import path from "path";

import { NextRequest, NextResponse } from "next/server";

const WIKI_ROOT = path.resolve(process.cwd(), "..", "..", "wiki");

function resolveWikiPage(pagePath: string) {
  if (!pagePath.startsWith("wiki/") || !pagePath.endsWith(".md")) {
    throw new Error("Only wiki Markdown pages can be edited");
  }

  const resolved = path.resolve(process.cwd(), "..", "..", pagePath);
  if (!resolved.startsWith(`${WIKI_ROOT}${path.sep}`)) {
    throw new Error("Page path escapes wiki root");
  }
  return resolved;
}

export async function POST(request: NextRequest) {
  try {
    const payload = (await request.json()) as { path?: unknown; content?: unknown };
    const pagePath = typeof payload.path === "string" ? payload.path : "";
    const content = typeof payload.content === "string" ? payload.content : "";

    if (!pagePath || !content.trim()) {
      return NextResponse.json({ saved: false, error: "path and content are required" }, { status: 400 });
    }

    const resolved = resolveWikiPage(pagePath);
    await fs.writeFile(resolved, content.endsWith("\n") ? content : `${content}\n`, "utf8");

    return NextResponse.json({ saved: true, path: pagePath });
  } catch (error) {
    return NextResponse.json(
      { saved: false, error: error instanceof Error ? error.message : "Could not save page" },
      { status: 400 }
    );
  }
}
