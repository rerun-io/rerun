use std::collections::HashSet;

use re_log_types::EntityPath;
use re_types::datatypes;

use crate::{
    types::{EdgeInstance, NodeInstance, UnknownNodeInstance},
    visualizers::{EdgeData, NodeData},
};

mod hash;
pub(crate) use hash::GraphNodeHash;
mod index;
pub(crate) use index::NodeIndex;

pub(crate) enum Node<'a> {
    Regular(NodeInstance<'a>),
    Unknown(UnknownNodeInstance<'a>),
}

impl<'a> From<&Node<'a>> for NodeIndex {
    fn from(node: &Node<'a>) -> Self {
        match node {
            Node::Regular(node) => node.into(),
            Node::Unknown(node) => node.into(),
        }
    }
}

impl<'a> From<Node<'a>> for NodeIndex {
    fn from(node: Node<'a>) -> Self {
        match node {
            Node::Regular(node) => node.into(),
            Node::Unknown(node) => node.into(),
        }
    }
}

pub(crate) struct Graph<'a> {
    /// Contains all nodes that are part mentioned in the edges but not part of the `nodes` list
    unknown: HashSet<(&'a EntityPath, datatypes::GraphNode)>,
    nodes: &'a Vec<NodeData>,
    edges: &'a Vec<EdgeData>,
}

impl<'a> Graph<'a> {
    pub fn from_nodes_edges(nodes: &'a Vec<NodeData>, edges: &'a Vec<EdgeData>) -> Self {
        let seen = nodes
            .iter()
            .flat_map(|entity| entity.nodes())
            .map(NodeIndex::from)
            .collect::<HashSet<_>>();

        let mut unknown = HashSet::new();
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

        Self {
            unknown,
            nodes,
            edges,
        }
    }

    pub fn nodes_by_entity(&self) -> impl Iterator<Item = &NodeData> {
        self.nodes.iter()
    }

    pub fn all_nodes(&'a self) -> impl Iterator<Item = Node<'a>> {
        let nodes = self
            .nodes
            .iter()
            .flat_map(|entity| entity.nodes().map(Node::Regular));
        let unknowns = self.unknown_nodes().map(Node::Unknown);
        nodes.chain(unknowns)
    }

    pub fn edges_by_entity(&self) -> impl Iterator<Item = &EdgeData> {
        self.edges.iter()
    }

    pub fn all_edges(&self) -> impl Iterator<Item = EdgeInstance<'_>> {
        self.edges.iter().flat_map(|entity| entity.edges())
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
