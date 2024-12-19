//! Provides a basic abstraction over a graph that was logged to an entity.

// For now this is pretty basic, but in the future we might replace this with
// something like `petgraph`, which will unlock more user interactions, such as
// highlighting of neighboring nodes and edges.

mod hash;

use egui::{Pos2, Vec2};
pub(crate) use hash::GraphNodeHash;
mod ids;
pub(crate) use ids::{EdgeId, NodeId};

use re_chunk::EntityPath;
use re_types::components::{self, GraphType};

use crate::{
    layout::EdgeTemplate,
    ui::DrawableLabel,
    visualizers::{EdgeData, NodeData, NodeInstance},
};

/// Describes the different kind of nodes that we can have in a graph.
pub enum Node {
    /// An explicit node is a node that was provided via [`re_types::archetypes::GraphNodes`].
    ///
    /// It therefore has an instance, as well as all properties that can be added via that archetype.
    Explicit {
        instance: NodeInstance,
        label: DrawableLabel,
    },

    /// An implicit node is a node that was provided via [`re_types::archetypes::GraphEdges`], but does not have a corresponding [`re_types::components::GraphNode`] in an [`re_types::archetypes::GraphNodes`] archetype.
    ///
    /// Because it was never specified directly, it also does not have many of the properties that an [`Node::Explicit`] has.
    Implicit {
        id: NodeId,
        graph_node: components::GraphNode,
        label: DrawableLabel,
    },
}

impl Node {
    pub fn id(&self) -> NodeId {
        match self {
            Self::Explicit { instance, .. } => instance.id,
            Self::Implicit { id, .. } => *id,
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
            Self::Explicit {
                instance: NodeInstance { position, .. },
                ..
            } => *position,
            Self::Implicit { .. } => None,
        }
    }
}

pub struct Graph {
    entity: EntityPath,
    nodes: Vec<Node>,
    edges: Vec<EdgeTemplate>,
    #[expect(unused)]
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
            seen.extend(data.nodes.iter().map(|n| n.id));
            // TODO(grtlr): We should see if we can get rid of some of the cloning here.
            data.nodes
                .iter()
                .map(|n| Node::Explicit {
                    instance: n.clone(),
                    label: DrawableLabel::from_label(ui, &n.label),
                })
                .collect()
        } else {
            Vec::new()
        };

        let (edges, kind) = if let Some(data) = edge_data {
            for edge in &data.edges {
                if seen.insert(edge.source_index) {
                    nodes.push(Node::Implicit {
                        id: edge.source_index,
                        graph_node: edge.source.clone(),
                        label: DrawableLabel::implicit_circle(ui),
                    });
                }
                if seen.insert(edge.target_index) {
                    nodes.push(Node::Implicit {
                        id: edge.target_index,
                        graph_node: edge.target.clone(),
                        label: DrawableLabel::implicit_circle(ui),
                    });
                }
            }

            let es = data.edges.iter().map(|e| EdgeTemplate {
                source: e.source_index,
                target: e.target_index,
                target_arrow: data.graph_type == GraphType::Directed,
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

    pub fn edges(&self) -> &[EdgeTemplate] {
        &self.edges
    }

    pub fn entity(&self) -> &EntityPath {
        &self.entity
    }
}
