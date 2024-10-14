use std::{collections::HashSet};

use re_log_types::EntityPath;
use re_viewer::external::re_types::datatypes;

use crate::{
    error::Error,
    types::{EdgeInstance, NodeInstance, UnknownNodeInstance},
    visualizers::{EdgesDirectedData, EdgesUndirectedData, NodeVisualizerData},
};

mod hash;
pub(crate) use hash::NodeIdHash;
mod index;
pub(crate) use index::NodeIndex;

pub(crate) enum Node<'a> {
    Regular(NodeInstance<'a>),
    Unknown(UnknownNodeInstance<'a>),
}

impl<'a> From<&Node<'a>> for NodeIndex {
    fn from(node: &Node) -> Self {
        match node {
            Node::Regular(node) => node.into(),
            Node::Unknown(node) => node.into(),
        }
    }
}

impl<'a> From<Node<'a>> for NodeIndex {
    fn from(node: Node) -> Self {
        match node {
            Node::Regular(node) => node.into(),
            Node::Unknown(node) => node.into(),
        }
    }
}

pub(crate) struct Graph<'a> {
    /// Contains all nodes that are part mentioned in the edges but not part of the `nodes` list
    unknown: HashSet<(EntityPath, datatypes::GraphNodeId)>,
    nodes: &'a Vec<NodeVisualizerData>,
    directed: &'a Vec<EdgesDirectedData>,
    undirected: &'a Vec<EdgesUndirectedData>,
}

impl<'a> Graph<'a> {
    pub fn from_nodes_edges(
        nodes: &'a Vec<NodeVisualizerData>,
        directed: &'a Vec<EdgesDirectedData>,
        undirected: &'a Vec<EdgesUndirectedData>,
    ) -> Option<Self> {
        let mut seen: HashSet<(&EntityPath, &datatypes::GraphNodeId)> = nodes
            .iter()
            .flat_map(|entity| entity.nodes())
            .map(|n| (n.entity_path, n.node_id))
            .collect();

        let mut unknown = HashSet::new();
        for entity in undirected {
            for edge in entity.edges() {
                for node in edge.nodes() {
                    let entity_path = EntityPath::from(node.entity_path.clone());
                    if seen.contains(&(&entity_path, &node.node_id)) {
                        continue;
                    }
                    unknown.insert((entity_path, node.node_id));
                }
            }
        }
        for entity in directed {
            for edge in entity.edges() {
                for node in edge.nodes() {
                    let entity_path = EntityPath::from(node.entity_path.clone());
                    if seen.contains(&(&entity_path, &node.node_id)) {
                        continue;
                    }
                    unknown.insert((entity_path, node.node_id));
                }
            }
        }

        if nodes.is_empty() && unknown.is_empty() {
            return None;
        }

        Some(Self {
            unknown,
            nodes,
            directed,
            undirected,
        })
    }

    pub fn nodes_by_entity(&self) -> impl Iterator<Item = &NodeVisualizerData> {
        self.nodes.iter()
    }

    pub fn all_nodes(&'a self) -> impl Iterator<Item = Node> {
        let nodes = self
            .nodes
            .iter()
            .flat_map(|entity| entity.nodes().map(Node::Regular));
        let unknowns = self.unknown_nodes().map(Node::Unknown);
        nodes.chain(unknowns)
    }

    pub fn edges_by_entity(&self) -> impl Iterator<Item = &EdgesUndirectedData> {
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
