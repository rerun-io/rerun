//! Contains all the (geometric) information that is considered when performing a graph layout.
//!
//! We support:
//! * Multiple edges between the same two nodes.
//! * Self-edges
//!
//! <div class="warning"> Duplicated graph nodes are undefined behavior.</div>

use std::collections::BTreeMap;

use egui::{Pos2, Vec2};
use re_chunk::EntityPath;

use crate::graph::{EdgeId, Graph, NodeId};

#[derive(PartialEq)]
pub(super) struct NodeTemplate {
    pub(super) size: Vec2,
    pub(super) fixed_position: Option<Pos2>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct EdgeTemplate {
    pub source: NodeId,
    pub target: NodeId,
    pub target_arrow: bool,
}

#[derive(Default, PartialEq)]
pub(super) struct GraphTemplate {
    pub(super) nodes: BTreeMap<NodeId, NodeTemplate>,

    /// The edges in the layout.
    ///
    /// Each entry can contain multiple edges.
    pub(super) edges: BTreeMap<EdgeId, Vec<EdgeTemplate>>,
}

/// A [`LayoutRequest`] encapsulates all the information that is considered when computing a layout.
///
/// It implements [`PartialEq`] to check if a layout is up-to-date, or if it needs to be recomputed.
#[derive(PartialEq)]
pub struct LayoutRequest {
    pub(super) graphs: BTreeMap<EntityPath, GraphTemplate>,
}

impl LayoutRequest {
    pub fn from_graphs<'a>(graphs: impl IntoIterator<Item = &'a Graph>) -> Self {
        let mut request = Self {
            graphs: BTreeMap::new(),
        };

        for graph in graphs {
            let entity = request.graphs.entry(graph.entity().clone()).or_default();

            for node in graph.nodes() {
                let shape = NodeTemplate {
                    size: node.size(),
                    fixed_position: node.position(),
                };
                let duplicate = entity.nodes.insert(node.id(), shape);
                debug_assert!(
                    duplicate.is_none(),
                    "duplicated nodes are undefined behavior"
                );
            }

            for edge in graph.edges() {
                let id = EdgeId {
                    source: edge.source,
                    target: edge.target,
                };

                let es = entity.edges.entry(id).or_default();
                es.push(edge.clone());
            }
        }

        request
    }

    /// Returns `true` if all nodes in this request have fixed positions.
    pub(super) fn all_nodes_fixed(&self) -> bool {
        self.all_nodes().all(|(_, v)| v.fixed_position.is_some())
    }

    /// Returns all nodes from all graphs in this request.
    pub(super) fn all_nodes(&self) -> impl Iterator<Item = (NodeId, &NodeTemplate)> + '_ {
        self.graphs
            .iter()
            .flat_map(|(_, graph)| graph.nodes.iter().map(|(k, v)| (*k, v)))
    }

    /// Returns all edges from all graphs in this request.
    pub(super) fn all_edges(&self) -> impl Iterator<Item = (EdgeId, &[EdgeTemplate])> + '_ {
        self.graphs
            .iter()
            .flat_map(|(_, graph)| graph.edges.iter().map(|(k, v)| (*k, v.as_slice())))
    }
}
