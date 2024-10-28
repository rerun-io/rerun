use re_log_types::EntityPath;
use re_types::components;

use crate::{
    types::UnknownNodeInstance,
    visualizers::{EdgeData, NodeData},
};

mod hash;
pub(crate) use hash::GraphNodeHash;
mod index;
pub(crate) use index::NodeIndex;

// TODO(grtlr): This struct should act as an abstraction over the graph in the future.
pub(crate) struct Graph<'a> {
    /// Contains all nodes that are part mentioned in the edges but not part of the `nodes` list
    unknown: ahash::HashSet<(&'a EntityPath, components::GraphNode)>,
}

impl<'a> Graph<'a> {
    pub fn from_nodes_edges(nodes: &'a [NodeData], edges: &'a [EdgeData]) -> Self {
        let seen = nodes
            .iter()
            .flat_map(|entity| entity.nodes())
            .map(NodeIndex::from)
            .collect::<nohash_hasher::IntSet<NodeIndex>>();

        let mut unknown = ahash::HashSet::default();
        for entity in edges {
            for edge in entity.edges() {
                for node in edge.nodes() {
                    if seen.contains(&NodeIndex::from_entity_node(&entity.entity_path, node)) {
                        continue;
                    }
                    unknown.insert((&entity.entity_path, node.clone()));
                }
            }
        }

        Self { unknown }
    }

    pub fn unknown_nodes(&'a self) -> impl Iterator<Item = UnknownNodeInstance<'a>> {
        self.unknown
            .iter()
            .map(|(entity_path, node_id)| UnknownNodeInstance {
                entity_path,
                node_id,
            })
    }
}
