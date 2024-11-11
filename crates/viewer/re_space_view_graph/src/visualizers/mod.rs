mod edges;
mod nodes;

pub use edges::{EdgeData, EdgesVisualizer};
pub use nodes::{NodeData, NodeVisualizer};
use re_chunk::EntityPath;

use crate::graph::NodeIndex;

/// Gathers all nodes, explicit and implicit, from the visualizers.
///
/// Explicit nodes are nodes that are defined through the `GraphNode` archetype.
/// Implicit nodes are nodes that _only_ appear in edges defined by the `GraphEdge` archetype.
pub fn all_nodes<'a>(
    nodes: impl IntoIterator<Item = (&'a EntityPath, &'a NodeData)>,
    edges: impl IntoIterator<Item = (&'a EntityPath, &'a EdgeData)>,
) -> impl Iterator<Item = (&'a EntityPath, NodeIndex)> {
    let explicit = nodes
        .into_iter()
        .flat_map(|(entity, data)| data.nodes.iter().map(move |n| (entity, n.index)));

    let implicit = edges.into_iter().flat_map(|(entity, data)| {
        data.edges.iter().flat_map(move |edge| {
            edge.nodes()
                .map(move |n| (entity, NodeIndex::from_entity_node(entity, n)))
        })
    });

    explicit.chain(implicit)
}

/// Gathers all edges as tuples of `NodeIndex` from the visualizer.
pub fn all_edges<'a>(
    edges: impl IntoIterator<Item = (&'a EntityPath, &'a EdgeData)>,
) -> impl Iterator<Item = (&'a EntityPath, (NodeIndex, NodeIndex))> {
    edges.into_iter().flat_map(|(entity, data)| {
        data.edges.iter().map(move |edge| {
            let source = NodeIndex::from_entity_node(entity, &edge.source);
            let target = NodeIndex::from_entity_node(entity, &edge.target);

            (entity, (source, target))
        })
    })
}
