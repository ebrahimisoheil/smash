import type {
  BackendResult,
  GraphPayload,
  HealthPayload,
  IngestStatusPayload,
  JsonRecord,
  MemoryBriefPayload,
  MemoryInboxPayload,
  MemoryProfilePayload,
  MutationResult,
  PromptPayload,
  StatusPayload
} from "@/lib/types";

const SERVER_BASE_URL = process.env.SMASH_API_BASE_URL ?? "http://127.0.0.1:3000";
const CLIENT_BASE_URL = "/api/smash";

function buildUrl(path: string, query?: Record<string, string | number | boolean | undefined>, baseUrl = SERVER_BASE_URL) {
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  const url = new URL(`/api${normalizedPath}`, baseUrl);
  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value === undefined || value === "") {
        continue;
      }
      url.searchParams.set(key, String(value));
    }
  }
  return url.toString();
}

async function readJson<T>(response: Response): Promise<T> {
  const payload = (await response.json()) as T & JsonRecord;
  if (!response.ok) {
    const message = typeof payload.error === "string" ? payload.error : `Request failed with ${response.status}`;
    throw new Error(message);
  }
  return payload;
}

export async function serverGet<T>(path: string, query?: Record<string, string | number | boolean | undefined>): Promise<T> {
  const response = await fetch(buildUrl(path, query), { cache: "no-store" });
  return readJson<T>(response);
}

export async function serverGetSafe<T>(
  path: string,
  query?: Record<string, string | number | boolean | undefined>
): Promise<BackendResult<T>> {
  try {
    const data = await serverGet<T>(path, query);
    return {
      available: true,
      data
    };
  } catch (error) {
    return {
      available: false,
      error: error instanceof Error ? error.message : "fetch failed",
      baseUrl: SERVER_BASE_URL
    };
  }
}

export async function clientGet<T>(path: string, query?: Record<string, string | number | boolean | undefined>): Promise<T> {
  const url = new URL(`${CLIENT_BASE_URL}${path.startsWith("/") ? path : `/${path}`}`, window.location.origin);
  if (query) {
    for (const [key, value] of Object.entries(query)) {
      if (value === undefined || value === "") {
        continue;
      }
      url.searchParams.set(key, String(value));
    }
  }
  const response = await fetch(url.toString(), { cache: "no-store" });
  return readJson<T>(response);
}

export async function clientPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const response = await fetch(`${CLIENT_BASE_URL}${path.startsWith("/") ? path : `/${path}`}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json"
    },
    body: JSON.stringify(body)
  });
  return readJson<T>(response);
}

export const smashApi = {
  status: () => serverGet<StatusPayload>("/status", { validate: true }),
  statusSafe: () => serverGetSafe<StatusPayload>("/status", { validate: true }),
  health: () => serverGet<HealthPayload>("/health"),
  healthSafe: () => serverGetSafe<HealthPayload>("/health"),
  ingestStatus: () => serverGet<IngestStatusPayload>("/ingest-status"),
  ingestStatusSafe: () => serverGetSafe<IngestStatusPayload>("/ingest-status"),
  memoryInbox: () => serverGet<MemoryInboxPayload>("/memory-inbox", { limit: 24 }),
  memoryInboxSafe: () => serverGetSafe<MemoryInboxPayload>("/memory-inbox", { limit: 24 }),
  memoryProfile: () => serverGet<MemoryProfilePayload>("/memory-profile", { limit: 12 }),
  memoryProfileSafe: () => serverGetSafe<MemoryProfilePayload>("/memory-profile", { limit: 12 }),
  memoryBrief: (query = "local memory") => serverGet<MemoryBriefPayload>("/memory-brief", { q: query, limit: 8 }),
  memoryBriefSafe: (query = "local memory") => serverGetSafe<MemoryBriefPayload>("/memory-brief", { q: query, limit: 8 }),
  graphSummary: (topic = "", depth = 1) =>
    serverGet<GraphPayload>("/graph-summary", { topic, limit: 80, depth, max_edges: 160 }),
  graphSummarySafe: (topic = "", depth = 1) =>
    serverGetSafe<GraphPayload>("/graph-summary", { topic, limit: 80, depth, max_edges: 160 }),
  graph: () => serverGet<GraphPayload>("/graph"),
  graphSafe: () => serverGetSafe<GraphPayload>("/graph"),
  prompts: () => serverGet<PromptPayload>("/prompts")
  ,
  promptsSafe: () => serverGetSafe<PromptPayload>("/prompts")
};

export const smashMutations = {
  reviewMemory: (identifier: string) => clientPost<MutationResult>("/review-memory", { identifier }),
  archiveMemory: (identifier: string, reason: string) => clientPost<MutationResult>("/archive-memory", { identifier, reason }),
  restoreMemory: (identifier: string) => clientPost<MutationResult>("/restore-memory", { identifier }),
  validate: () => clientPost<MutationResult>("/validate", {}),
  rebuildIndex: () => clientPost<MutationResult>("/rebuild-index", {}),
  rebuildBacklinks: () => clientPost<MutationResult>("/rebuild-backlinks", {})
};

export { buildUrl, CLIENT_BASE_URL, SERVER_BASE_URL };
