use egui::{Pos2, Rect};

use crate::{graph::NodeIndex, ui::bounding_rect_from_iter};

pub type LineSegment = [Pos2; 2];

#[derive(Debug, PartialEq, Eq)]
pub struct Layout {
    pub(super) nodes: ahash::HashMap<NodeIndex, Rect>,
    pub(super) edges: ahash::HashMap<(NodeIndex, NodeIndex), LineSegment>,
}

impl Layout {
    #[deprecated]
    pub fn bounding_rect(&self) -> Rect {
        // TODO(grtlr): We mostly use this for debugging, but we should probably
        // take all elements of the layout into account.
        bounding_rect_from_iter(self.nodes.values().copied())
    }

    /// Gets the final position and size of a node in the layout.
    ///
    /// Returns `Rect::ZERO` if the node is not present in the layout.
    pub fn get_node(&self, node: &NodeIndex) -> Rect {
        self.nodes.get(node).copied().unwrap_or(Rect::ZERO)
    }

    /// Gets the shape of an edge in the final layout.
    ///
    /// Returns `[Pos2::ZERO, Pos2::ZERO]` if the edge is not present in the layout.
    pub fn get_edge(&self, from: NodeIndex, to: NodeIndex) -> LineSegment {
        self.edges
            .get(&(from, to))
            .copied()
            .unwrap_or([Pos2::ZERO, Pos2::ZERO])
    }
}
