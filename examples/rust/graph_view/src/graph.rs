use std::collections::HashSet;

use re_log_types::EntityPath;
use re_viewer::external::re_types::datatypes;

use crate::{
    types::{EdgeInstance, NodeInstance, UnknownNodeInstance},
    visualizers::{NodeVisualizerData, UndirectedEdgesData},
};

pub(crate) enum Node<'a> {
    Regular(NodeInstance<'a>),
    Unknown(UnknownNodeInstance<'a>),
}

pub(crate) struct Graph<'a> {
    /// Contains all nodes that are part mentioned in the edges but not part of the `nodes` list
    unknown: Vec<(EntityPath, datatypes::GraphNodeId)>,
    nodes: &'a Vec<NodeVisualizerData>,
    undirected: &'a Vec<UndirectedEdgesData>,
}

impl<'a> Graph<'a> {
    pub fn from_nodes_edges(
        nodes: &'a Vec<NodeVisualizerData>,
        undirected: &'a Vec<UndirectedEdgesData>,
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

    pub fn nodes_by_entity(&self) -> impl Iterator<Item = &NodeVisualizerData> {
        self.nodes.iter()
    }

    pub fn all_nodes(&self) -> impl Iterator<Item = Node> {
        let nodes = self
            .nodes
            .iter()
            .flat_map(|entity| entity.nodes().map(Node::Regular));
        let unknowns = self.unknown_nodes().map(Node::Unknown);
        nodes.chain(unknowns)
    }

    pub fn edges_by_entity(&self) -> impl Iterator<Item = &UndirectedEdgesData> {
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
