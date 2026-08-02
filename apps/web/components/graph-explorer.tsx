"use client";

import { FormEvent, useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import rehypeRaw from "rehype-raw";
import remarkGfm from "remark-gfm";

import { EmptyState, Pill } from "@/components/ui";
import { clientGet } from "@/lib/api";
import type { ContextPayload, GraphEdge, GraphNode, GraphPayload, PageLinksPayload } from "@/lib/types";

type ViewMode = "graph" | "notes";
type NoteMode = "preview" | "markdown";

type PositionedNode = GraphNode & {
  x: number;
  y: number;
  radius: number;
  layer: string;
};

const palette: Record<string, string> = {
  concepts: "#4e79a7",
  entities: "#f28e2b",
  memories: "#edc948",
  sources: "#59a14f",
  comparisons: "#e15759",
  explorations: "#76b7b2",
  root: "#bab0ac",
  default: "#8ea4bd"
};

const categoryOrder = ["root", "sources", "memories", "concepts", "entities", "comparisons", "explorations"];

function nodeColor(node: GraphNode) {
  return palette[node.category ?? "default"] ?? palette.default;
}

function nodeKind(node: GraphNode) {
  return node.type ?? node.page_type ?? node.group ?? "unknown";
}

function shortLabel(label: string, max = 22) {
  return label.length > max ? `${label.slice(0, max)}...` : label;
}

function markdownForPreview(markdown: string) {
  return markdown.replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, target: string, label?: string) => {
    const cleanTarget = String(target).trim();
    const cleanLabel = String(label || target).trim();
    return `[${cleanLabel}](smash-node://${encodeURIComponent(cleanTarget)})`;
  });
}

function internalHrefToNodeId(href?: string) {
  if (!href || href.startsWith("#") || /^[a-z][a-z0-9+.-]*:/i.test(href)) {
    return "";
  }
  const withoutHash = href.split("#")[0] ?? "";
  const withoutQuery = withoutHash.split("?")[0] ?? "";
  const lastSegment = withoutQuery.split("/").filter(Boolean).pop() ?? withoutQuery;
  return decodeURIComponent(lastSegment.replace(/\.md$/i, "")).trim();
}

function isLocalWikiHref(href?: string) {
  if (!href || href.startsWith("#")) {
    return false;
  }
  return href.startsWith("/") || href.startsWith("./") || href.startsWith("../") || href.endsWith(".md") || href.startsWith("wiki/");
}

function compareCategory(left: string, right: string) {
  const leftIndex = categoryOrder.indexOf(left);
  const rightIndex = categoryOrder.indexOf(right);
  return (leftIndex === -1 ? 999 : leftIndex) - (rightIndex === -1 ? 999 : rightIndex);
}

function groupCounts(nodes: GraphNode[]) {
  const counts = new Map<string, number>();
  for (const node of nodes) {
    const key = node.category || "uncategorized";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return Array.from(counts.entries()).sort(([left], [right]) => compareCategory(left, right));
}

function layoutNodes(nodes: GraphNode[], edges: GraphEdge[]) {
  const degree = new Map<string, number>();
  for (const edge of edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
  }

  const groups = Array.from(
    nodes.reduce((result, node) => {
      const key = node.category || "uncategorized";
      result.set(key, [...(result.get(key) ?? []), node]);
      return result;
    }, new Map<string, GraphNode[]>())
  ).sort(([left], [right]) => compareCategory(left, right));

  const columnGap = 150;
  const rowGap = 34;

  return groups.flatMap(([layer, items], layerIndex) => {
    const sorted = [...items].sort((left, right) => {
      const leftWeight = `${left.distance ?? 0}-${(left.degree ?? degree.get(left.id) ?? 0) * -1}-${left.title ?? left.id}`;
      const rightWeight = `${right.distance ?? 0}-${(right.degree ?? degree.get(right.id) ?? 0) * -1}-${right.title ?? right.id}`;
      return leftWeight.localeCompare(rightWeight);
    });

    const x = layerIndex * columnGap - ((groups.length - 1) * columnGap) / 2;
    return sorted.map((node, nodeIndex) => ({
      ...node,
      degree: node.degree ?? degree.get(node.id) ?? 0,
      radius: 6 + Math.min((node.degree ?? degree.get(node.id) ?? 0) * 0.6, 3),
      x,
      y: nodeIndex * rowGap - ((sorted.length - 1) * rowGap) / 2,
      layer
    }));
  });
}

export function GraphExplorer({ initialGraph }: { initialGraph: GraphPayload }) {
  const [topic, setTopic] = useState("");
  const [graph, setGraph] = useState(initialGraph);
  const [search, setSearch] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("notes");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(initialGraph.nodes?.[0]?.id ?? null);
  const [pinnedNode, setPinnedNode] = useState<GraphNode | null>(null);
  const [showFullGraph, setShowFullGraph] = useState(false);
  const [activeCategory, setActiveCategory] = useState("all");
  const [showLabels, setShowLabels] = useState(false);
  const [noteMode, setNoteMode] = useState<NoteMode>("preview");
  const [markdownDraft, setMarkdownDraft] = useState("");
  const [savingNote, setSavingNote] = useState(false);
  const [saveStatus, setSaveStatus] = useState("");
  const [context, setContext] = useState<ContextPayload | null>(null);
  const [links, setLinks] = useState<PageLinksPayload | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState("");

  const categories = useMemo(() => groupCounts(graph.nodes ?? []), [graph.nodes]);

  const filteredNodes = useMemo(() => {
    const query = search.trim().toLowerCase();
    return (graph.nodes ?? []).filter((node) => {
      const categoryMatch = activeCategory === "all" || (node.category || "uncategorized") === activeCategory;
      const searchMatch =
        !query ||
        [node.title, node.name, node.id, node.summary]
          .filter(Boolean)
          .some((value) => String(value).toLowerCase().includes(query));
      return categoryMatch && searchMatch;
    });
  }, [activeCategory, graph.nodes, search]);

  const filteredNodeIds = useMemo(() => new Set(filteredNodes.map((node) => node.id)), [filteredNodes]);

  const visibleEdges = useMemo(
    () => (graph.edges ?? []).filter((edge) => filteredNodeIds.has(edge.source) && filteredNodeIds.has(edge.target)),
    [filteredNodeIds, graph.edges]
  );

  const selected = useMemo(
    () => filteredNodes.find((node) => node.id === selectedId) ?? (pinnedNode?.id === selectedId ? pinnedNode : null) ?? filteredNodes[0] ?? graph.nodes[0] ?? null,
    [filteredNodes, graph.nodes, pinnedNode, selectedId]
  );

  const positionedNodes = useMemo(() => layoutNodes(filteredNodes, visibleEdges), [filteredNodes, visibleEdges]);

  useEffect(() => {
    if (!selected?.id) {
      setContext(null);
      setLinks(null);
      return;
    }

    let cancelled = false;
    setDetailLoading(true);
    setDetailError("");

    Promise.all([
      clientGet<ContextPayload>("/context", { topic: selected.id }),
      clientGet<PageLinksPayload>("/page-links", { page: selected.id })
    ])
      .then(([nextContext, nextLinks]) => {
        if (!cancelled) {
          setContext(nextContext);
          setLinks(nextLinks);
          setMarkdownDraft(nextContext.pages.find((page) => page.is_primary)?.content ?? nextContext.pages[0]?.content ?? "");
          setNoteMode("preview");
          setSaveStatus("");
        }
      })
      .catch((nextError) => {
        if (!cancelled) {
          setDetailError(nextError instanceof Error ? nextError.message : "Could not load node details");
        }
      })
      .finally(() => {
        if (!cancelled) {
          setDetailLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [selected?.id]);

  useEffect(() => {
    if (selected && (filteredNodeIds.has(selected.id) || pinnedNode?.id === selected.id)) {
      return;
    }
    setSelectedId(filteredNodes[0]?.id ?? graph.nodes[0]?.id ?? null);
  }, [filteredNodeIds, filteredNodes, graph.nodes, pinnedNode, selected]);

  async function loadGraph(nextFullGraph: boolean, nextTopic = topic, nextSelectedId?: string) {
    setLoading(true);
    setError("");
    if (!nextSelectedId) {
      setPinnedNode(null);
    }
    try {
      const next = nextFullGraph
        ? await clientGet<GraphPayload>("/graph")
        : await clientGet<GraphPayload>("/graph-summary", { topic: nextTopic, limit: 80, depth: 1, max_edges: 160 });
      setGraph(next);
      setShowFullGraph(nextFullGraph);
      setActiveCategory("all");
      setSearch("");
      setSelectedId(nextSelectedId ?? (next.nodes ?? [])[0]?.id ?? null);
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Could not load graph");
    } finally {
      setLoading(false);
    }
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await loadGraph(false, topic);
  }

  function openNode(nodeId: string) {
    const graphNode = graph.nodes.find((node) => node.id === nodeId);
    setPinnedNode(graphNode ?? { id: nodeId, title: nodeId, category: "linked", type: "page", degree: 0 });
    setSelectedId(nodeId);
    setViewMode("notes");
  }

  async function openSemanticResult(node: GraphNode) {
    setPinnedNode(null);
    setTopic(node.title ?? node.name ?? node.id);
    setSelectedId(node.id);
    setViewMode("notes");
  }

  async function saveMarkdown() {
    if (!primaryPage?.path) {
      setSaveStatus("No wiki path");
      return;
    }

    setSavingNote(true);
    setSaveStatus("");
    try {
      const response = await fetch("/api/page-content", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ path: primaryPage.path, content: markdownDraft })
      });
      const result = (await response.json()) as { saved?: boolean; error?: string };
      if (!response.ok || !result.saved) {
        throw new Error(result.error ?? "Could not save note");
      }
      setSaveStatus("Saved");
    } catch (saveError) {
      setSaveStatus(saveError instanceof Error ? saveError.message : "Could not save note");
    } finally {
      setSavingNote(false);
    }
  }

  if (!graph.nodes?.length) {
    return <EmptyState title="Graph unavailable" copy="Start Smash locally, then request a graph summary or full graph." />;
  }

  const selectedTitle = selected?.title ?? selected?.name ?? selected?.id ?? "Knowledge graph";
  const primaryPage = context?.pages.find((page) => page.is_primary) ?? context?.pages[0] ?? null;
  const noteMarkdown = markdownDraft || primaryPage?.content || primaryPage?.tldr || selected?.summary || "";
  const relatedNodes = filteredNodes
    .filter((node) => node.id !== selected?.id)
    .sort((left, right) => (right.degree ?? 0) - (left.degree ?? 0))
    .slice(0, 6);

  const graphMap = (
    <section className="graph-columns-stage">
      <div className="graph-columns-stage-top">
        <div>
          <span className="graph-columns-kicker">{showFullGraph ? "Full graph" : "Semantic neighborhood"}</span>
          <h3>Layered graph</h3>
        </div>
        <div className="graph-columns-stats">
          <Pill tone="accent">{filteredNodes.length} nodes</Pill>
          <Pill tone="default">{visibleEdges.length} edges</Pill>
        </div>
      </div>

      <svg viewBox="-520 -260 1040 520" className="graph-columns-svg" role="img" aria-label="Layered graph view">
        {categories
          .filter(([category]) => activeCategory === "all" || category === activeCategory)
          .map(([category], index, list) => {
            const x = index * 150 - ((list.length - 1) * 150) / 2;
            return (
              <g key={category}>
                <rect
                  x={x - 54}
                  y={-220}
                  width={108}
                  height={440}
                  rx={28}
                  fill={palette[category] ?? palette.default}
                  opacity="0.07"
                />
                <text x={x} y={-232} textAnchor="middle" className="graph-columns-layer-label">
                  {category}
                </text>
              </g>
            );
          })}

        {visibleEdges.map((edge) => {
          const source = positionedNodes.find((node) => node.id === edge.source);
          const target = positionedNodes.find((node) => node.id === edge.target);
          if (!source || !target) {
            return null;
          }
          const isSelectedEdge = source.id === selected?.id || target.id === selected?.id;
          return (
            <line
              key={`${edge.source}-${edge.target}`}
              x1={source.x}
              y1={source.y}
              x2={target.x}
              y2={target.y}
              stroke={isSelectedEdge ? "rgba(99, 91, 255, 0.48)" : "rgba(15, 23, 42, 0.12)"}
              strokeWidth={isSelectedEdge ? 1.8 : 1}
            />
          );
        })}

        {positionedNodes.map((node) => {
          const isSelected = node.id === selected?.id;
          return (
            <g
              key={node.id}
              className="graph-columns-node"
              transform={`translate(${node.x}, ${node.y})`}
              onClick={() => {
                setPinnedNode(null);
                openNode(node.id);
              }}
            >
              <circle r={(node.radius ?? 8) + (isSelected ? 6 : 3)} fill={nodeColor(node)} opacity={isSelected ? 0.18 : 0.1} />
              <circle
                r={node.radius ?? 8}
                fill={nodeColor(node)}
                stroke={isSelected ? "#0a2540" : "rgba(10,37,64,0.16)"}
                strokeWidth={isSelected ? 2 : 1}
              />
              {showLabels ? (
                <text
                  x="0"
                  y={(node.radius ?? 8) + 15}
                  textAnchor="middle"
                  className={`graph-columns-node-label ${isSelected ? "graph-columns-node-label-selected" : ""}`}
                >
                  {shortLabel(node.title ?? node.name ?? node.id)}
                </text>
              ) : null}
            </g>
          );
        })}
      </svg>
    </section>
  );

  const notePanel = (
    <div className="graph-note-panel">
      <div className="graph-note-header">
        <div>
          <span className="graph-columns-kicker">Selected note</span>
          <h4>{selectedTitle}</h4>
        </div>
        <div className="graph-note-actions">
          <div className="graph-note-tabs" aria-label="Note view">
            <button className={noteMode === "preview" ? "graph-note-tab-active" : ""} onClick={() => setNoteMode("preview")} type="button">
              Preview
            </button>
            <button className={noteMode === "markdown" ? "graph-note-tab-active" : ""} onClick={() => setNoteMode("markdown")} type="button">
              Markdown
            </button>
          </div>
          {noteMode === "markdown" ? (
            <button className="graph-note-save" disabled={savingNote || !primaryPage?.path} onClick={saveMarkdown} type="button">
              {savingNote ? "Saving" : "Save"}
            </button>
          ) : null}
        </div>
      </div>
      <div className="graph-columns-meta graph-note-meta">
        <span>{selected?.category ?? "uncategorized"}</span>
        <span>{selected ? nodeKind(selected) : "unknown"}</span>
        <span>{primaryPage?.path ?? selected?.id}</span>
      </div>
      {saveStatus ? <p className="graph-note-status">{saveStatus}</p> : null}
      {detailError ? <p className="inline-feedback">{detailError}</p> : null}
      {detailLoading ? <p className="graph-muted-copy">Loading note...</p> : null}
      {noteMode === "preview" ? (
        <div className="graph-note-preview">
          {noteMarkdown ? (
            <ReactMarkdown
              rehypePlugins={[rehypeRaw]}
              remarkPlugins={[remarkGfm]}
              components={{
                a: ({ href, children }) => {
                  if (href?.startsWith("smash-node://")) {
                    const target = decodeURIComponent(href.replace("smash-node://", ""));
                    return (
                      <button className="graph-note-link" onClick={() => openNode(target)} type="button">
                        {children}
                      </button>
                    );
                  }
                  const internalNodeId = internalHrefToNodeId(href);
                  if (internalNodeId) {
                    return (
                      <button className="graph-note-link" onClick={() => openNode(internalNodeId)} type="button">
                        {children}
                      </button>
                    );
                  }
                  if (isLocalWikiHref(href)) {
                    return (
                      <button className="graph-note-link" onClick={() => openNode(String(children))} type="button">
                        {children}
                      </button>
                    );
                  }
                  return (
                    <a href={href} rel="noreferrer">
                      {children}
                    </a>
                  );
                }
              }}
            >
              {markdownForPreview(noteMarkdown)}
            </ReactMarkdown>
          ) : (
            <p>Select a node to inspect it.</p>
          )}
        </div>
      ) : (
        <textarea className="graph-note-editor" value={markdownDraft} onChange={(event) => setMarkdownDraft(event.target.value)} spellCheck={false} />
      )}
    </div>
  );

  return (
    <div className="graph-columns">
      <div className="graph-mode-switch" aria-label="Workspace mode">
        <button className={viewMode === "notes" ? "graph-mode-active" : ""} onClick={() => setViewMode("notes")} type="button">
          Notes
        </button>
        <button className={viewMode === "graph" ? "graph-mode-active" : ""} onClick={() => setViewMode("graph")} type="button">
          Graph
        </button>
      </div>

      <div className="graph-columns-controls">
        <form className="query-bar graph-columns-query" onSubmit={submit}>
          <input value={topic} onChange={(event) => setTopic(event.target.value)} placeholder="Semantic search across notes" />
          <button className="action-button" disabled={loading}>
            {loading ? "Searching..." : "Search"}
          </button>
        </form>
        <div className="graph-actions">
          <input
            className="graph-search-input"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Filter visible nodes"
          />
          <button className="action-button ghost" disabled={loading} onClick={() => loadGraph(!showFullGraph)} type="button">
            {showFullGraph ? "Back to summary" : "Load full graph"}
          </button>
          <button className="action-button ghost" onClick={() => setShowLabels((value) => !value)} type="button">
            {showLabels ? "Hide names" : "Show names"}
          </button>
        </div>
      </div>

      <div className="graph-filter-row">
        <button
          className={`legend-pill ${activeCategory === "all" ? "legend-pill-active" : ""}`}
          onClick={() => setActiveCategory("all")}
          type="button"
        >
          all · {graph.nodes.length}
        </button>
        {categories.map(([category, count]) => (
          <button
            key={category}
            className={`legend-pill ${activeCategory === category ? "legend-pill-active" : ""}`}
            onClick={() => setActiveCategory(category)}
            type="button"
          >
            <span className="legend-swatch" style={{ backgroundColor: palette[category] ?? palette.default }} />
            {category} · {count}
          </button>
        ))}
      </div>

      {error ? <p className="inline-feedback">{error}</p> : null}

      <div className={`graph-columns-layout graph-columns-layout-${viewMode}`}>
        <div className="graph-map-slot">{graphMap}</div>
        <aside className="graph-columns-panel">
          {notePanel}
          <div className="graph-columns-card graph-compact-card">
            <div className="graph-detail-hero">
              <h4>Backlinks</h4>
            </div>
            <div className="graph-columns-links">
              <div>
                <span className="graph-columns-kicker">Inbound</span>
                <div className="graph-link-list">
                  {(links?.inbound ?? []).map((item) => (
                    <button key={item} className="graph-link-button" onClick={() => openNode(item)} type="button">
                      {item}
                    </button>
                  ))}
                  {!links?.inbound?.length ? <p className="graph-muted-copy">No inbound links yet.</p> : null}
                </div>
              </div>
              <div>
                <span className="graph-columns-kicker">Outgoing</span>
                <div className="graph-link-list">
                  {(links?.forward ?? []).map((item) => (
                    <button key={item} className="graph-link-button" onClick={() => openNode(item)} type="button">
                      {item}
                    </button>
                  ))}
                  {!links?.forward?.length ? <p className="graph-muted-copy">No outgoing links yet.</p> : null}
                </div>
              </div>
            </div>
          </div>

          <div className="graph-columns-card graph-compact-card">
            <div className="graph-detail-hero">
              <h4>Node cards</h4>
              <span className="graph-columns-kicker">Semantic results</span>
            </div>
            <div className="graph-node-card-grid">
              {relatedNodes.map((node) => (
                <button
                  key={node.id}
                  className={`graph-node-card ${node.id === selected?.id ? "graph-node-card-active" : ""}`}
                  onClick={() => void openSemanticResult(node)}
                  type="button"
                >
                  <span className="graph-node-card-dot" style={{ backgroundColor: nodeColor(node) }} />
                  <strong>{node.title ?? node.name ?? node.id}</strong>
                  <span>
                    {node.category ?? "node"} · degree {node.degree ?? 0}
                  </span>
                  {node.summary ? <p>{shortLabel(node.summary, 110)}</p> : null}
                </button>
              ))}
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}
