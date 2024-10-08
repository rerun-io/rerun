use std::collections::HashSet;

use re_log_types::EntityPath;
use re_viewer::external::re_log::external::log;

use crate::{
    common::NodeLocation, edge_undirected_visualizer_system::EdgeInstance,
    node_visualizer_system::NodeInstance,
};

pub(crate) struct Graph<'a, N, E>
where
    N: Iterator<Item = NodeInstance<'a>>,
    E: Iterator<Item = EdgeInstance<'a>>,
{
    node_it: N,
    edge_it: E,
}

impl<'a, N, E> Graph<'a, N, E>
where
    N: Iterator<Item = NodeInstance<'a>>,
    E: Iterator<Item = EdgeInstance<'a>>,
{
    pub fn new(nodes: N, edges: E) -> Self {
        Self {
            node_it: nodes,
            edge_it: edges,
        }
    }

    pub fn nodes(self) -> impl Iterator<Item = Node<'a>> {
        AllNodesIterator::new(self.node_it, edges_to_nodes(self.edge_it))
    }

    pub fn dummy_nodes(self) -> impl Iterator<Item = (NodeLocation, &'a EntityPath)> {
        self.nodes().filter_map(|n| match n {
            Node::Dummy(location, entity_path) => Some((location, entity_path)),
            Node::Regular(_) => None,
        })
    }
}

pub(crate) enum Node<'a> {
    Regular(NodeInstance<'a>),
    Dummy(NodeLocation, &'a EntityPath),
}

fn edges_to_iter<'a>(
    edge: EdgeInstance<'a>,
) -> impl Iterator<Item = (NodeLocation, &'a EntityPath)> {
    let source = (edge.source, edge.entity_path);
    let target = (edge.target, edge.entity_path);
    std::iter::once(source.clone()).chain(std::iter::once(target))
}

pub(crate) fn edges_to_nodes<'a>(
    edges: impl IntoIterator<Item = EdgeInstance<'a>>,
) -> impl Iterator<Item = (NodeLocation, &'a EntityPath)> {
    edges.into_iter().flat_map(|e| edges_to_iter(e))
}

#[derive(Clone)]
pub(crate) struct AllNodesIterator<'a, N, E>
where
    N: Iterator<Item = NodeInstance<'a>>,
    E: Iterator<Item = (NodeLocation, &'a EntityPath)> + Sized,
{
    seen: HashSet<NodeLocation>,
    node_it: N,
    edge_it: E,
}

impl<'a, N, E> AllNodesIterator<'a, N, E>
where
    N: Iterator<Item = NodeInstance<'a>>,
    E: Iterator<Item = (NodeLocation, &'a EntityPath)>,
{
    pub fn new(node_it: N, edge_it: E) -> Self {
        Self {
            seen: HashSet::new(),
            node_it,
            edge_it,
        }
    }
}

impl<'a, N, E> Iterator for AllNodesIterator<'a, N, E>
where
    N: Iterator<Item = NodeInstance<'a>>,
    E: Iterator<Item = (NodeLocation, &'a EntityPath)>,
{
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Node<'a>> {
        if let Some(node) = self.node_it.next() {
            self.seen.insert(node.location.clone());
            return Some(Node::Regular(node));
        }

        for (node, entity_path) in self.edge_it.by_ref() {
            if !self.seen.contains(&node) {
                return Some(Node::Dummy(node, entity_path));
            }
        }

        None
    }
}
