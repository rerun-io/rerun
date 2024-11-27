use std::collections::{BTreeMap, BTreeSet};

use egui::{Pos2, Vec2};
use re_chunk::EntityPath;

use crate::graph::{Graph, NodeIndex};

#[derive(PartialEq)]
struct NodeTemplate {
    size: Vec2,
    fixed_position: Option<Pos2>,
}

#[derive(Default, PartialEq)]
struct GraphTemplate {
    nodes: BTreeMap<NodeIndex, NodeTemplate>,
    edges: BTreeSet<(NodeIndex, NodeIndex)>,
}

#[derive(PartialEq)]
pub struct LayoutRequest {
    graphs: BTreeMap<EntityPath, GraphTemplate>,
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
