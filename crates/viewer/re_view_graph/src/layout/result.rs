//! Defines the output of a layout algorithm, i.e. everything that we need to render the graph.

use egui::Rect;
use re_chunk::EntityPath;

use crate::graph::{EdgeId, NodeId};

use super::EdgeGeometry;

#[derive(Debug)]
pub struct Layout {
    pub(super) nodes: ahash::HashMap<NodeId, Rect>,
    pub(super) edges: ahash::HashMap<EdgeId, Vec<EdgeGeometry>>,
    pub(super) entities: Vec<(EntityPath, Rect)>,
}

fn bounding_rect_from_iter(rectangles: impl Iterator<Item = egui::Rect>) -> egui::Rect {
    rectangles.fold(egui::Rect::NOTHING, |acc, rect| acc.union(rect))
}

impl Layout {
    /// Creates an empty layout
    pub fn empty() -> Self {
        Self {
            nodes: ahash::HashMap::default(),
            edges: ahash::HashMap::default(),
            entities: Vec::new(),
        }
    }

    /// Returns the bounding rectangle of the layout.
    pub fn bounding_rect(&self) -> Rect {
        // TODO(grtlr): We mostly use this for debugging, but we should probably
        // take all elements of the layout into account, once we have entity rects too.
        bounding_rect_from_iter(self.nodes.values().copied())
    }

    /// Gets the final position and size of a node in the layout.
    pub fn get_node(&self, node: &NodeId) -> Option<Rect> {
        self.nodes.get(node).copied()
    }

    /// Gets the shape of an edge in the final layout.
    #[expect(unused)]
    pub fn get_edge(&self, edge: &EdgeId) -> Option<&[EdgeGeometry]> {
        self.edges.get(edge).map(|es| es.as_slice())
    }

    /// Returns an iterator over all edges in the layout.
    pub fn edges(&self) -> impl Iterator<Item = (EdgeId, &[EdgeGeometry])> {
        self.edges.iter().map(|(id, es)| (*id, es.as_slice()))
    }

    /// Returns the number of entities in the layout.
    pub fn num_entities(&self) -> usize {
        self.entities.len()
    }

    /// Returns an iterator over all edges in the layout.
    pub fn entities(&self) -> impl Iterator<Item = &(EntityPath, Rect)> {
        self.entities.iter()
    }
}
