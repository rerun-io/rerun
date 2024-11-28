mod hash;

use egui::{Pos2, Vec2};
pub(crate) use hash::GraphNodeHash;
mod index;
pub(crate) use index::NodeId;

use re_chunk::EntityPath;
use re_log_types::Instance;
use re_types::{components::GraphType, ArrowString};

use crate::{
    ui::DrawableLabel,
    visualizers::{EdgeData, NodeData},
};

pub enum Node {
    Explicit {
        id: NodeId,
        instance: Instance,
        node: ArrowString,
        position: Option<Pos2>,
        label: DrawableLabel,
    },
    Implicit {
        id: NodeId,
        node: ArrowString,
        label: DrawableLabel,
    },
}

impl Node {
    pub fn id(&self) -> NodeId {
        match self {
            Self::Explicit { id, .. } | Self::Implicit { id, .. } => *id,
        }
    }

    pub fn label(&self) -> &DrawableLabel {
        match self {
            Self::Explicit { label, .. } | Self::Implicit { label, .. } => label,
        }
    }

    pub fn size(&self) -> Vec2 {
        self.label().size()
    }

    pub fn position(&self) -> Option<Pos2> {
        match self {
            Self::Explicit { position, .. } => *position,
            Self::Implicit { .. } => None,
        }
    }
}

pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub arrow: bool,
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
                arrow: data.graph_type == GraphType::Directed,
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

    pub fn entity(&self) -> &EntityPath {
        &self.entity
    }
}
