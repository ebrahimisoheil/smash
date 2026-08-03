//! Bounded, deterministic graph traversal over Area-local Entities and
//! Relationships.
//!
//! This is the Phase F "bounded graph expansion" primitive described in
//! `docs/contracts/retrieval-algorithm.md` §6. It is a pure, storage-free
//! function: given an already-authorized, single-Area slice of `Entity` and
//! `Relationship` data, it walks outward from a starting set of entities and
//! returns everything reachable within an explicit depth/node/edge budget.
//!
//! Two invariants matter more than the traversal mechanics themselves:
//!
//! - **No hidden fan-out.** The function receives exactly the data it is
//!   allowed to see and never reaches beyond it — there is no fetching, no
//!   I/O, and no notion of "ask for more". Authorization and Cross-Map
//!   (cross-Area) traversal both happen outside this function, before it is
//!   ever called.
//! - **Determinism.** The same `start`/`entities`/`relationships`/`budget`
//!   always produce byte-identical output, regardless of the order the
//!   caller's `Vec`s happen to be in. Every comparison that could otherwise
//!   depend on input order is broken by content (`relation_kind`, then the
//!   UUIDv7-ordered typed ID) rather than by position.

use engrave_contracts::{Entity, EntityId, Relationship, RelationshipId};
use std::collections::{BTreeMap, BTreeSet};

/// Explicit bounds on a single `bounded_traverse` call. Every bound is a
/// hard cap: traversal stops the moment any one of them would be exceeded,
/// and the returned `GraphPacket::truncated` flag records that it did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphBudget {
    /// Maximum number of hops from the `start` set. Depth `0` is the start
    /// set itself; depth `1` is its direct neighbors, and so on.
    pub max_depth: u32,
    /// Maximum number of nodes (including the `start` nodes) in the result.
    pub max_nodes: usize,
    /// Maximum number of edges in the result.
    pub max_edges: usize,
}

impl Default for GraphBudget {
    /// `max_depth: 2` matches the Phase E retrieval-algorithm baseline
    /// (`docs/contracts/retrieval-algorithm.md` §6: "breadth-first depth
    /// `2`, capped at `2 * K` new nodes" for `K_entry = 30`). `max_nodes`
    /// and `max_edges` here are this module's own measured starting point,
    /// not permanent truth — same tunable-default posture as the Phase E
    /// ledger (`docs/phase-e-ledger.md`) takes toward its own baselines.
    /// `200` nodes and `400` edges comfortably cover a `2 * 30 = 60`-node
    /// graph-expansion budget layered on top of a full fused-entry set
    /// while still bounding worst-case work; a future benchmark report may
    /// move these numbers, but callers should not treat them as fixed.
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_nodes: 200,
            max_edges: 400,
        }
    }
}

/// One reachable Entity, tagged with its shortest-path distance (in hops)
/// from the `start` set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphNode {
    pub entity_id: EntityId,
    pub depth: u32,
}

/// One Relationship whose endpoints are both present in the returned node
/// set. Direction is preserved exactly as recorded on the source
/// `Relationship`, even though traversal itself treats relationships as
/// undirected for reachability purposes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphEdge {
    pub relationship_id: RelationshipId,
    pub source_entity_id: EntityId,
    pub target_entity_id: EntityId,
    pub relation_kind: String,
}

/// The result of a bounded traversal: the reachable nodes and edges found
/// within budget, plus whether the budget cut the walk short of the full
/// reachable graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPacket {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// `true` when depth, node, or edge limits stopped traversal before the
    /// full connected component (within `max_depth`) was captured. Never
    /// silently drop data without setting this.
    pub truncated: bool,
}

/// One undirected traversal step: which neighbor a relationship reaches,
/// what kind of relationship it is, and which relationship carries it.
/// Sorting on `(relation_kind, neighbor_id, relationship_id)` gives fully
/// deterministic expansion order — `relationship_id` is UUIDv7-ordered, so
/// it is always available as a final, never-tied tie-break.
type Step = (String, EntityId, RelationshipId);

/// Admit `entity_id` into the visited set at `depth` if `max_nodes` allows
/// it. Written as a plain function (rather than a closure) so each call
/// borrows `visited`/`nodes`/`truncated` only for its own duration, which
/// keeps the surrounding traversal loop free to read `visited` between
/// calls.
fn try_admit(
    visited: &mut BTreeMap<EntityId, u32>,
    nodes: &mut Vec<GraphNode>,
    truncated: &mut bool,
    max_nodes: usize,
    entity_id: EntityId,
    depth: u32,
) -> bool {
    if visited.len() >= max_nodes {
        *truncated = true;
        return false;
    }
    visited.insert(entity_id, depth);
    nodes.push(GraphNode { entity_id, depth });
    true
}

/// Bounded breadth-first traversal starting from `start`, following
/// `relationships` in both directions. `start` entity IDs that are not
/// present in `entities` are ignored rather than treated as an error: this
/// function only ever returns what is actually reachable from data it was
/// given.
///
/// Lifecycle state (`EntityState`/`RelationshipState`) is intentionally not
/// consulted here — filtering retired/superseded/rejected data, like
/// authorization, is the caller's responsibility before this function is
/// invoked.
pub fn bounded_traverse(
    start: &[EntityId],
    entities: &[Entity],
    relationships: &[Relationship],
    budget: GraphBudget,
) -> GraphPacket {
    let known_entities: BTreeSet<EntityId> =
        entities.iter().map(|entity| entity.entity_id).collect();

    let mut adjacency: BTreeMap<EntityId, Vec<Step>> = BTreeMap::new();
    for relationship in relationships {
        if !known_entities.contains(&relationship.source_entity_id)
            || !known_entities.contains(&relationship.target_entity_id)
        {
            // Defensive: a relationship pointing outside the given Entity
            // slice cannot be traversed without exceeding the data we were
            // handed, so it is simply invisible to this function.
            continue;
        }
        adjacency
            .entry(relationship.source_entity_id)
            .or_default()
            .push((
                relationship.relation_kind.clone(),
                relationship.target_entity_id,
                relationship.relationship_id,
            ));
        adjacency
            .entry(relationship.target_entity_id)
            .or_default()
            .push((
                relationship.relation_kind.clone(),
                relationship.source_entity_id,
                relationship.relationship_id,
            ));
    }
    for steps in adjacency.values_mut() {
        steps.sort();
    }

    let mut visited: BTreeMap<EntityId, u32> = BTreeMap::new();
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut truncated = false;

    let start_ids: BTreeSet<EntityId> = start
        .iter()
        .copied()
        .filter(|entity_id| known_entities.contains(entity_id))
        .collect();

    let mut frontier: Vec<EntityId> = Vec::new();
    let mut node_budget_exhausted = false;
    for entity_id in &start_ids {
        if try_admit(
            &mut visited,
            &mut nodes,
            &mut truncated,
            budget.max_nodes,
            *entity_id,
            0,
        ) {
            frontier.push(*entity_id);
        } else {
            node_budget_exhausted = true;
            break;
        }
    }

    let mut current_depth = 0u32;
    if !node_budget_exhausted {
        loop {
            if frontier.is_empty() {
                // The reachable component (within prior levels) is
                // exhausted; nothing beyond depth is being cut off.
                break;
            }
            if current_depth >= budget.max_depth {
                let more_beyond_depth = frontier.iter().any(|entity_id| {
                    adjacency.get(entity_id).is_some_and(|steps| {
                        steps
                            .iter()
                            .any(|(_, neighbor, _)| !visited.contains_key(neighbor))
                    })
                });
                if more_beyond_depth {
                    truncated = true;
                }
                break;
            }

            let mut candidates: BTreeSet<(String, EntityId)> = BTreeSet::new();
            for entity_id in &frontier {
                if let Some(steps) = adjacency.get(entity_id) {
                    for (relation_kind, neighbor, _relationship_id) in steps {
                        if !visited.contains_key(neighbor) {
                            candidates.insert((relation_kind.clone(), *neighbor));
                        }
                    }
                }
            }
            if candidates.is_empty() {
                break;
            }

            let next_depth = current_depth + 1;
            let mut next_frontier = Vec::new();
            let mut hit_budget = false;
            for (_relation_kind, neighbor) in candidates {
                if visited.contains_key(&neighbor) {
                    // Already admitted via an earlier (kind, id) candidate
                    // for the same neighbor discovered through a different
                    // relation_kind at this same level.
                    continue;
                }
                if try_admit(
                    &mut visited,
                    &mut nodes,
                    &mut truncated,
                    budget.max_nodes,
                    neighbor,
                    next_depth,
                ) {
                    next_frontier.push(neighbor);
                } else {
                    hit_budget = true;
                    break;
                }
            }
            if hit_budget {
                break;
            }
            frontier = next_frontier;
            current_depth = next_depth;
        }
    }

    // Deterministic output order: ascending depth, then ascending typed ID.
    nodes.sort_by(|left, right| {
        left.depth
            .cmp(&right.depth)
            .then(left.entity_id.cmp(&right.entity_id))
    });

    let mut qualifying: Vec<&Relationship> = relationships
        .iter()
        .filter(|relationship| {
            visited.contains_key(&relationship.source_entity_id)
                && visited.contains_key(&relationship.target_entity_id)
        })
        .collect();
    qualifying.sort_by(|left, right| {
        left.relation_kind
            .cmp(&right.relation_kind)
            .then(left.target_entity_id.cmp(&right.target_entity_id))
            .then(left.relationship_id.cmp(&right.relationship_id))
    });

    let mut edges = Vec::new();
    for relationship in qualifying {
        if edges.len() >= budget.max_edges {
            truncated = true;
            break;
        }
        edges.push(GraphEdge {
            relationship_id: relationship.relationship_id,
            source_entity_id: relationship.source_entity_id,
            target_entity_id: relationship.target_entity_id,
            relation_kind: relationship.relation_kind.clone(),
        });
    }

    GraphPacket {
        nodes,
        edges,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engrave_contracts::{
        AreaId, EntityState, MapVersionId, Origin, RelationshipState, TenantId,
    };

    fn entity(
        entity_id: EntityId,
        tenant_id: TenantId,
        area_id: AreaId,
        map_version_id: MapVersionId,
    ) -> Entity {
        Entity {
            entity_id,
            tenant_id,
            area_id,
            map_version_id,
            kind: "account".into(),
            state: EntityState::Active,
            origin: Origin::Observed,
            descriptor: serde_json::json!({}),
            version: 1,
        }
    }

    fn relationship(
        relationship_id: RelationshipId,
        tenant_id: TenantId,
        area_id: AreaId,
        map_version_id: MapVersionId,
        source_entity_id: EntityId,
        target_entity_id: EntityId,
        relation_kind: &str,
    ) -> Relationship {
        Relationship {
            relationship_id,
            tenant_id,
            area_id,
            map_version_id,
            source_entity_id,
            target_entity_id,
            relation_kind: relation_kind.into(),
            state: RelationshipState::Active,
            origin: Origin::Observed,
            version: 1,
        }
    }

    /// Shared fixture: a simple A --owns--> B --employs--> C chain.
    struct Chain {
        tenant_id: TenantId,
        area_id: AreaId,
        map_version_id: MapVersionId,
        a: EntityId,
        b: EntityId,
        c: EntityId,
        entities: Vec<Entity>,
        relationships: Vec<Relationship>,
    }

    fn chain_fixture() -> Chain {
        let tenant_id = TenantId::new_v7();
        let area_id = AreaId::new_v7();
        let map_version_id = MapVersionId::new_v7();
        let a = EntityId::new_v7();
        let b = EntityId::new_v7();
        let c = EntityId::new_v7();
        let entities = vec![
            entity(a, tenant_id, area_id, map_version_id),
            entity(b, tenant_id, area_id, map_version_id),
            entity(c, tenant_id, area_id, map_version_id),
        ];
        let relationships = vec![
            relationship(
                RelationshipId::new_v7(),
                tenant_id,
                area_id,
                map_version_id,
                a,
                b,
                "owns",
            ),
            relationship(
                RelationshipId::new_v7(),
                tenant_id,
                area_id,
                map_version_id,
                b,
                c,
                "employs",
            ),
        ];
        Chain {
            tenant_id,
            area_id,
            map_version_id,
            a,
            b,
            c,
            entities,
            relationships,
        }
    }

    #[test]
    fn two_hop_chain_respects_max_depth() {
        let chain = chain_fixture();

        let full = bounded_traverse(
            &[chain.a],
            &chain.entities,
            &chain.relationships,
            GraphBudget {
                max_depth: 2,
                ..GraphBudget::default()
            },
        );
        let reached: BTreeSet<EntityId> = full.nodes.iter().map(|node| node.entity_id).collect();
        assert!(reached.contains(&chain.b));
        assert!(reached.contains(&chain.c));
        assert!(!full.truncated);

        let shallow = bounded_traverse(
            &[chain.a],
            &chain.entities,
            &chain.relationships,
            GraphBudget {
                max_depth: 1,
                ..GraphBudget::default()
            },
        );
        let reached_shallow: BTreeSet<EntityId> =
            shallow.nodes.iter().map(|node| node.entity_id).collect();
        assert!(reached_shallow.contains(&chain.b));
        assert!(!reached_shallow.contains(&chain.c));
        assert!(shallow.truncated, "C is reachable but cut off by max_depth");
    }

    #[test]
    fn max_nodes_and_max_edges_caps_are_honored() {
        let chain = chain_fixture();

        let node_capped = bounded_traverse(
            &[chain.a],
            &chain.entities,
            &chain.relationships,
            GraphBudget {
                max_depth: 2,
                max_nodes: 2,
                max_edges: 400,
            },
        );
        assert!(node_capped.nodes.len() <= 2);
        assert!(node_capped.truncated);

        let edge_capped = bounded_traverse(
            &[chain.a],
            &chain.entities,
            &chain.relationships,
            GraphBudget {
                max_depth: 2,
                max_nodes: 200,
                max_edges: 1,
            },
        );
        assert!(edge_capped.edges.len() <= 1);
        assert!(edge_capped.truncated);
    }

    #[test]
    fn traversal_is_deterministic_regardless_of_input_order() {
        let chain = chain_fixture();

        let forward = bounded_traverse(
            &[chain.a],
            &chain.entities,
            &chain.relationships,
            GraphBudget::default(),
        );

        let mut reversed_entities = chain.entities.clone();
        reversed_entities.reverse();
        let mut reversed_relationships = chain.relationships.clone();
        reversed_relationships.reverse();
        let reversed = bounded_traverse(
            &[chain.a],
            &reversed_entities,
            &reversed_relationships,
            GraphBudget::default(),
        );

        assert_eq!(forward, reversed);
    }

    #[test]
    fn disconnected_entity_is_never_included() {
        let chain = chain_fixture();
        let mut entities = chain.entities.clone();
        let stranger = EntityId::new_v7();
        entities.push(entity(
            stranger,
            chain.tenant_id,
            chain.area_id,
            chain.map_version_id,
        ));

        let packet = bounded_traverse(
            &[chain.a],
            &entities,
            &chain.relationships,
            GraphBudget::default(),
        );
        assert!(!packet.nodes.iter().any(|node| node.entity_id == stranger));
        assert!(!packet
            .edges
            .iter()
            .any(|edge| edge.source_entity_id == stranger || edge.target_entity_id == stranger));
    }

    #[test]
    fn empty_start_returns_empty_non_truncated_packet() {
        let chain = chain_fixture();
        let packet = bounded_traverse(
            &[],
            &chain.entities,
            &chain.relationships,
            GraphBudget::default(),
        );
        assert!(packet.nodes.is_empty());
        assert!(packet.edges.is_empty());
        assert!(!packet.truncated);
    }
}
