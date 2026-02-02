mod edges;
mod nodes;

use std::collections::BTreeSet;

pub use edges::{EdgeData, EdgesVisualizer};
pub use nodes::{Label, NodeData, NodeInstance, NodeVisualizer};
use re_chunk::EntityPath;

/// Iterates over all entities and joins the node and edge data.
pub fn merge<'a>(
    node_data: &'a ahash::HashMap<EntityPath, NodeData>,
    edge_data: &'a ahash::HashMap<EntityPath, EdgeData>,
) -> impl Iterator<Item = (&'a EntityPath, Option<&'a NodeData>, Option<&'a EdgeData>)> + 'a {
    // We sort the entities to ensure that we always process them in the same order.
    let unique_entities = node_data
        .keys()
        .chain(edge_data.keys())
        .collect::<BTreeSet<_>>();

    unique_entities.into_iter().map(|entity| {
        let nodes = node_data.get(entity);
        let edges = edge_data.get(entity);
        (entity, nodes, edges)
    })
}
