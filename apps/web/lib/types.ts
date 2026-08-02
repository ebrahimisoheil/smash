export type JsonRecord = Record<string, unknown>;

export type NextAction = {
  label: string;
  tool?: string;
  command?: string;
  priority?: string;
  arguments?: Record<string, unknown>;
};

export type StatusPayload = {
  ready: boolean;
  version: string;
  page_count: number;
  memory_count: number;
  active_memory_count: number;
  needs_review_count: number;
  warnings: string[];
  validation?: {
    passed: boolean;
    error_count: number;
    warning_count: number;
  };
  next_actions?: NextAction[];
};

export type HealthPayload = {
  ready: boolean;
  status: StatusPayload;
  operations: {
    active_count: number;
    failed_count: number;
    stale_count: number;
    operation_count: number;
    next_actions?: NextAction[];
  };
};

export type MemoryIssue = {
  code: string;
  severity: string;
  message: string;
  suggested_action?: string;
};

export type MemoryAction = NextAction & {
  kind: string;
  description?: string;
};

export type MemoryItem = {
  name: string;
  path: string;
  title: string;
  memory_type: string;
  scope: string;
  visibility: string;
  project: string;
  status: string;
  review_status: string;
  tldr: string;
  snippet: string;
  tags: string[];
  issues?: MemoryIssue[];
  highest_severity?: string;
  primary_action?: MemoryAction;
  actions?: MemoryAction[];
};

export type MemoryInboxPayload = {
  review_count: number;
  include_archived: boolean;
  project: string;
  counts_by_severity: Record<string, number>;
  next_actions?: MemoryAction[];
  items: MemoryItem[];
};

export type MemoryProfilePayload = {
  memory_count: number;
  active_count: number;
  review_count: number;
  project: string;
  by_type: Record<string, number>;
  by_scope: Record<string, number>;
  by_visibility: Record<string, number>;
  by_project: Record<string, number>;
  by_status: Record<string, number>;
  top_tags: Array<{ tag: string; count: number }>;
  recent: MemoryItem[];
  preferences: MemoryItem[];
  decisions: MemoryItem[];
  projects: MemoryItem[];
  archived: MemoryItem[];
};

export type MemoryBriefPayload = {
  query: string;
  project: string;
  selection: string;
  profile: MemoryProfilePayload;
  relevant_count: number;
  relevant_memories: MemoryItem[];
  review?: {
    count: number;
  };
  captures?: {
    count: number;
    warning_count: number;
  };
  agent_guidance?: string[];
};

export type IngestCompletionItem = {
  raw: string;
  size_bytes: number;
  memory_prompt: string;
  query_prompt: string;
  secret_warnings: string[];
  scan_error: string;
  source_pages: Array<{
    name: string;
    path: string;
    title: string;
  }>;
};

export type IngestStatusPayload = {
  raw_count: number;
  source_page_count: number;
  pending_count: number;
  stale_count: number;
  backlinks_status: string;
  raw_secret_warning_count: number;
  raw_scan_warning_count: number;
  guidance: {
    state: string;
    summary: string;
    notes?: string[];
    commands?: string[];
  };
  plan: {
    title: string;
    summary: string;
    steps: string[];
    post_checks?: string[];
  };
  completion: {
    title: string;
    summary: string;
    items: IngestCompletionItem[];
  };
};

export type GraphNode = {
  id: string;
  name?: string;
  title?: string;
  group?: string;
  type?: string;
  page_type?: string;
  category?: string;
  degree?: number;
  in_degree?: number;
  out_degree?: number;
  distance?: number;
  summary?: string;
  why_selected?: string;
  radius?: number;
};

export type GraphEdge = {
  source: string;
  target: string;
  weight?: number;
};

export type GraphPayload = {
  topic?: string;
  mode?: string;
  found?: boolean;
  node_count?: number;
  edge_count?: number;
  returned_nodes?: number;
  returned_edges?: number;
  category_counts?: Record<string, number>;
  type_counts?: Record<string, number>;
  top_hubs?: Array<{
    id: string;
    title: string;
    category: string;
    type: string;
    degree: number;
  }>;
  nodes: GraphNode[];
  edges: GraphEdge[];
  total_nodes?: number;
  total_edges?: number;
  graph_mode?: string;
  note?: string;
  graph_note?: string;
  agent_guidance?: string[];
  follow_up?: NextAction[];
};

export type ContextPage = {
  name: string;
  path: string;
  title: string;
  category: string;
  type: string;
  tldr?: string;
  is_primary?: boolean;
  relationship?: string;
  content?: string;
};

export type ContextPayload = {
  topic: string;
  found: boolean;
  primary?: string;
  inbound_count?: number;
  forward_count?: number;
  pages: ContextPage[];
};

export type PageLinksPayload = {
  page: string;
  key?: string;
  inbound_count: number;
  forward_count: number;
  returned_inbound?: number;
  returned_forward?: number;
  truncated?: boolean;
  inbound: string[];
  forward: string[];
  agent_guidance?: string[];
  follow_up?: NextAction[];
};

export type PromptPayload = {
  project?: string;
  prompts?: Array<{
    label: string;
    prompt: string;
    when?: string;
  }>;
  commands?: string[];
  shortcut?: string;
  target?: string;
};

export type MutationResult = {
  error?: string;
  updated?: boolean;
  saved?: boolean;
  created?: boolean;
  review_status?: string;
  status?: string;
  [key: string]: unknown;
};

export type BackendUnavailable = {
  available: false;
  error: string;
  baseUrl: string;
};

export type BackendResult<T> =
  | {
      available: true;
      data: T;
    }
  | BackendUnavailable;
