use egui::{Pos2, Rect};

use crate::graph::NodeId;

pub type LineSegment = [Pos2; 2];

#[derive(Debug, PartialEq, Eq)]
pub struct Layout {
    pub(super) nodes: ahash::HashMap<NodeId, Rect>,
    pub(super) edges: ahash::HashMap<(NodeId, NodeId), LineSegment>,
    // TODO(grtlr): Consider adding the entity rects here too.
}

fn bounding_rect_from_iter(rectangles: impl Iterator<Item = egui::Rect>) -> egui::Rect {
    rectangles.fold(egui::Rect::NOTHING, |acc, rect| acc.union(rect))
}

impl Layout {
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
    pub fn get_edge(&self, from: NodeId, to: NodeId) -> Option<LineSegment> {
        self.edges.get(&(from, to)).copied()
    }
}
