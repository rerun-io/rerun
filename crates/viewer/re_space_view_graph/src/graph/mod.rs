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
pub(crate) struct Graph {
    /// Contains all nodes that are part mentioned in the edges but not part of the `nodes` list
    unknown: ahash::HashSet<(EntityPath, components::GraphNode)>,
}

impl Graph {
    pub fn from_nodes_edges(nodes: &[NodeData], edges: &[EdgeData]) -> Self {
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
                    unknown.insert((entity.entity_path.clone(), node.clone()));
                }
            }
        }

        Self { unknown }
    }

    pub fn unknown_nodes(&self) -> Vec<UnknownNodeInstance> {
        self.unknown
            .iter()
            .cloned()
            .map(|(entity_path, node_id)| UnknownNodeInstance {
                entity_path,
                node_id,
            })
            .collect()
    }
}
