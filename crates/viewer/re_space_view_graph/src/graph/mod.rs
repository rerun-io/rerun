mod hash;
use std::hash::{Hash as _, Hasher as _};

use egui::Pos2;
pub(crate) use hash::GraphNodeHash;
mod index;
pub(crate) use index::NodeIndex;

use re_chunk::EntityPath;
use re_log_types::Instance;
use re_types::{components::GraphType, ArrowString};

use crate::{
    ui::draw::DrawableLabel,
    visualizers::{EdgeData, NodeData},
};

pub enum Node {
    Explicit {
        id: NodeIndex,
        instance: Instance,
        node: ArrowString,
        position: Option<Pos2>,
        label: DrawableLabel,
    },
    Implicit {
        id: NodeIndex,
        node: ArrowString,
        label: DrawableLabel,
    },
}

impl Node {
    pub fn id(&self) -> NodeIndex {
        match self {
            Node::Explicit { id, .. } => *id,
            Node::Implicit { id, .. } => *id,
        }
    }

    pub fn label(&self) -> &DrawableLabel {
        match self {
            Node::Explicit { label, .. } => label,
            Node::Implicit { label, .. } => label,
        }
    }
}

pub struct Edge {
    pub from: NodeIndex,
    pub to: NodeIndex,
    marker: bool,
}

pub struct Graph {
    entity: EntityPath,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    kind: GraphType,
}

impl Graph {
    pub fn new<'a>(
        ui: &egui::Ui,
        entity: EntityPath,
        node_data: Option<&'a NodeData>,
        edge_data: Option<&'a EdgeData>,
    ) -> Self {
        // We keep track of the nodes to find implicit nodes.
        let mut seen = ahash::HashSet::default();

        let mut nodes: Vec<Node> = if let Some(data) = node_data {
            seen.extend(data.nodes.iter().map(|n| n.index));
            data.nodes
                .iter()
                .map(|n| Node::Explicit {
                    id: n.index,
                    instance: n.instance,
                    node: n.node.0 .0.clone(),
                    position: n.position,
                    label: DrawableLabel::from_label(ui, &n.label),
                })
                .collect()
        } else {
            Vec::new()
        };

        let (edges, kind) = if let Some(data) = edge_data {
            for edge in &data.edges {
                if !seen.contains(&edge.source_index) {
                    nodes.push(Node::Implicit {
                        id: edge.source_index,
                        node: edge.source.0 .0.clone(),
                        label: DrawableLabel::implicit_circle(),
                    });
                    seen.insert(edge.source_index);
                }
                if !seen.contains(&edge.target_index) {
                    nodes.push(Node::Implicit {
                        id: edge.target_index,
                        node: edge.target.0 .0.clone(),
                        label: DrawableLabel::implicit_circle(),
                    });
                    seen.insert(edge.target_index);
                }
            }

            let es = data.edges.iter().map(|e| Edge {
                from: e.source_index,
                to: e.target_index,
                marker: data.graph_type == GraphType::Directed,
            });

            (es.collect(), data.graph_type)
        } else {
            (Vec::new(), GraphType::default())
        };

        Self {
            entity,
            nodes,
            edges,
            kind,
        }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn kind(&self) -> GraphType {
        self.kind
    }

    pub fn size_hash(&self) -> u64 {
        let mut hasher = ahash::AHasher::default();
        for node in &self.nodes {
            match node {
                Node::Explicit {
                    id,
                    position,
                    label,
                    // The following fields can be ignored:
                    instance: _instance,
                    node: _node,
                } => {
                    id.hash(&mut hasher);

                    position.as_ref().map(bytemuck::bytes_of).hash(&mut hasher);
                    bytemuck::bytes_of(&label.size()).hash(&mut hasher);
                }
                Node::Implicit {
                    id,
                    label,
                    // The following fields can be ignored:
                    node: _node,
                } => {
                    id.hash(&mut hasher);
                    bytemuck::bytes_of(&label.size()).hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }
}
