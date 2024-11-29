use std::collections::{BTreeMap, BTreeSet};

use egui::{Pos2, Vec2};
use re_chunk::EntityPath;

use crate::graph::{Graph, NodeId};

#[derive(PartialEq)]
pub(super) struct NodeTemplate {
    pub(super) size: Vec2,
    pub(super) fixed_position: Option<Pos2>,
}

#[derive(Default, PartialEq)]
pub(super) struct GraphTemplate {
    pub(super) nodes: BTreeMap<NodeId, NodeTemplate>,
    pub(super) edges: BTreeSet<(NodeId, NodeId)>,
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
                entity.nodes.insert(node.id(), shape);
            }

            for edge in graph.edges() {
                entity.edges.insert((edge.from, edge.to));
            }
        }

        request
    }
}
