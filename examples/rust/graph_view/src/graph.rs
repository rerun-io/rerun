use std::collections::HashSet;

use re_log_types::{EntityPath, EntityPathHash};
use re_viewer::external::re_types::datatypes;

use crate::visualizers::{
    edges_undirected::{EdgeInstance, EdgeUndirectedVisualizerData},
    nodes::{GraphNodeVisualizerData, NodeInstance},
};

use re_viewer::external::re_types::datatypes::{GraphLocation, GraphNodeId};

pub(crate) enum Node<'a> {
    Regular(NodeInstance<'a>),
    Unknown(UnknownNodeInstance<'a>),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct NodeIndex {
    pub entity_hash: EntityPathHash,
    pub node_id: GraphNodeId,
}

impl From<GraphLocation> for NodeIndex {
    fn from(location: GraphLocation) -> Self {
        Self {
            entity_hash: EntityPath::from(location.entity_path).hash(),
            node_id: location.node_id,
        }
    }
}

impl From<&GraphLocation> for NodeIndex {
    fn from(location: &GraphLocation) -> Self {
        Self {
            entity_hash: EntityPath::from(location.entity_path.clone()).hash(),
            node_id: location.node_id.clone(),
        }
    }
}

impl std::fmt::Display for NodeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.node_id, self.entity_hash)
    }
}

impl<'a> From<&NodeInstance<'a>> for NodeIndex {
    fn from(node: &NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.clone(),
        }
    }
}

pub(crate) struct UnknownNodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNodeId,
    pub entity_path: &'a EntityPath,
}

impl<'a> From<&UnknownNodeInstance<'a>> for NodeIndex {
    fn from(node: &UnknownNodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.clone(),
        }
    }
}

pub(crate) struct Graph<'a> {
    /// Contains all nodes that are part mentioned in the edges but not part of the `nodes` list
    unknown: Vec<(EntityPath, datatypes::GraphNodeId)>,
    nodes: &'a Vec<GraphNodeVisualizerData>,
    undirected: &'a Vec<EdgeUndirectedVisualizerData>,
}

impl<'a> Graph<'a> {
    pub fn from_nodes_edges(
        nodes: &'a Vec<GraphNodeVisualizerData>,
        undirected: &'a Vec<EdgeUndirectedVisualizerData>,
    ) -> Self {
        let seen: HashSet<(&EntityPath, &datatypes::GraphNodeId)> = nodes
            .iter()
            .flat_map(|entity| entity.nodes())
            .map(|n| (n.entity_path, n.node_id))
            .collect();

        let unknown = undirected
            .iter()
            .flat_map(|entity| entity.edges().flat_map(|edge| edge.nodes()))
            .filter_map(|n| {
                let entity_path = EntityPath::from(n.entity_path.clone());
                if seen.contains(&(&entity_path, &n.node_id)) {
                    None
                } else {
                    Some((entity_path, n.node_id))
                }
            })
            .collect();

        Self {
            unknown,
            nodes,
            undirected,
        }
    }

    pub fn nodes_by_entity(&self) -> impl Iterator<Item = &GraphNodeVisualizerData> {
        self.nodes.iter()
    }

    pub fn all_nodes(&self) -> impl Iterator<Item = Node> {
        let nodes = self.nodes.iter().flat_map(|entity| entity.nodes().map(Node::Regular));
        let unknowns= self.unknown_nodes().map(Node::Unknown);
        nodes.chain(unknowns)
    }

    pub fn edges_by_entity(&self) -> impl Iterator<Item = &EdgeUndirectedVisualizerData> {
        self.undirected.iter()
    }

    pub fn edges(&self) -> impl Iterator<Item = EdgeInstance> {
        self.undirected.iter().flat_map(|entity| entity.edges())
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
