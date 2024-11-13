mod hash;
pub(crate) use hash::GraphNodeHash;
mod index;
pub(crate) use index::NodeIndex;

use re_types::components::{GraphNode, GraphType};

use crate::{
    types::{EdgeInstance, NodeInstance},
    visualizers::{EdgeData, NodeData},
};

pub struct NodeInstanceImplicit {
    pub node: GraphNode,
    pub index: NodeIndex,
}

pub struct Graph<'a> {
    explicit: &'a [NodeInstance],
    implicit: Vec<NodeInstanceImplicit>,
    edges: &'a [EdgeInstance],
    kind: GraphType,
}

impl<'a> Graph<'a> {
    pub fn new(node_data: Option<&'a NodeData>, edge_data: Option<&'a EdgeData>) -> Self {
        // We keep track of the nodes to find implicit nodes.
        let mut seen = ahash::HashSet::default();

        let explicit = if let Some(data) = node_data {
            seen.extend(data.nodes.iter().map(|n| n.index));
            data.nodes.as_slice()
        } else {
            &[][..]
        };

        let (edges, implicit, kind) = if let Some(data) = edge_data {
            let mut implicit = Vec::new();
            for edge in &data.edges {
                if !seen.contains(&edge.source_index) {
                    implicit.push(NodeInstanceImplicit {
                        node: edge.source.clone(),
                        index: edge.source_index,
                    });
                    seen.insert(edge.source_index);
                }
                if !seen.contains(&edge.target_index) {
                    implicit.push(NodeInstanceImplicit {
                        node: edge.target.clone(),
                        index: edge.target_index,
                    });
                    seen.insert(edge.target_index);
                }
            }
            (data.edges.as_slice(), implicit, Some(data.graph_type))
        } else {
            (&[][..], Vec::new(), None)
        };

        Self {
            explicit,
            implicit,
            edges,
            kind: kind.unwrap_or_default(),
        }
    }

    pub fn nodes_explicit(&self) -> impl Iterator<Item = &NodeInstance> {
        self.explicit.iter()
    }

    pub fn nodes_implicit(&self) -> impl Iterator<Item = &NodeInstanceImplicit> + '_ {
        self.implicit.iter()
    }

    pub fn edges(&self) -> impl Iterator<Item = &EdgeInstance> {
        self.edges.iter()
    }

    pub fn kind(&self) -> GraphType {
        self.kind
    }
}
