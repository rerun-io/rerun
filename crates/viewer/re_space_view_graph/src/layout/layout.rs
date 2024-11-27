use egui::{Pos2, Rect};

use crate::{graph::NodeIndex, ui::bounding_rect_from_iter};

pub type LineSegment = [Pos2; 2];

#[derive(Debug, PartialEq, Eq)]
pub struct Layout {
    pub(super) nodes: ahash::HashMap<NodeIndex, Rect>,
    pub(super) edges: ahash::HashMap<(NodeIndex, NodeIndex), LineSegment>,
}

impl Layout {
    pub fn bounding_rect(&self) -> Rect {
        bounding_rect_from_iter(self.nodes.values().copied())
    }

    /// Gets the final position and size of a node in the layout.
    pub fn get_node(&self, node: &NodeIndex) -> Option<Rect> {
        self.nodes.get(node).copied()
    }

    /// Gets the shape of an edge in the final layout.
    pub fn get_edge(&self, from: NodeIndex, to: NodeIndex) -> Option<LineSegment> {
        self.edges.get(&(from, to)).copied()
    }

    /// Updates the size and position of a node, for example after size changes.
    /// Returns `true` if the node changed its size.
    #[deprecated(note = "We should not need to update sizes anymore.")]
    pub fn update(&mut self, node: &NodeIndex, rect: Rect) -> bool {
        debug_assert!(
            self.nodes.contains_key(node),
            "node should exist in the layout"
        );
        if let Some(extent) = self.nodes.get_mut(node) {
            let size_changed = (extent.size() - rect.size()).length_sq() > 0.01;
            *extent = rect;
            return size_changed;
        }
        false
    }
}
