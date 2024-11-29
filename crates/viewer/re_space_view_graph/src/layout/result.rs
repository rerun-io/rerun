use egui::{Pos2, Rect, Vec2};

use crate::graph::NodeId;

#[derive(Clone, Debug)]
pub enum EdgeGeometry {
    /// A simple straight edge.
    Line { source: Pos2, target: Pos2 },

    /// Represents a cubic bezier curve.
    ///
    /// In the future we could probably support more complex splines.
    CubicBezier {
        source: Pos2,
        target: Pos2,
        control: [Pos2; 2],
    },
    // We could add other geometries, such as `Orthogonal` here too.
}

impl EdgeGeometry {
    pub fn bounding_rect(&self) -> Rect {
        match self {
            Self::Line { source, target } => Rect::from_two_pos(*source, *target),
            // TODO(grtlr): This is just a crude (upper) approximation, as the resulting bounding box can be too large.
            Self::CubicBezier {
                source,
                target,
                ref control,
            } => Rect::from_points(&[&[*source, *target], control.as_slice()].concat()),
        }
    }

    pub fn source_pos(&self) -> Pos2 {
        match self {
            Self::Line { source, .. } | Self::CubicBezier { source, .. } => *source,
        }
    }

    pub fn target_pos(&self) -> Pos2 {
        match self {
            Self::Line { target, .. } | Self::CubicBezier { target, .. } => *target,
        }
    }

    /// The direction of the edge at the source node (normalized).
    pub fn source_arrow_direction(&self) -> Vec2 {
        match self {
            Self::Line { source, target } => (source.to_vec2() - target.to_vec2()).normalized(),
            Self::CubicBezier {
                source, control, ..
            } => (control[0].to_vec2() - source.to_vec2()).normalized(),
        }
    }

    /// The direction of the edge at the target node (normalized).
    pub fn target_arrow_direction(&self) -> Vec2 {
        match self {
            Self::Line { source, target } => (target.to_vec2() - source.to_vec2()).normalized(),
            Self::CubicBezier {
                target, control, ..
            } => (target.to_vec2() - control[1].to_vec2()).normalized(),
        }
    }
}

#[derive(Debug)]
pub struct Layout {
    pub(super) nodes: ahash::HashMap<NodeId, Rect>,
    pub(super) edges: ahash::HashMap<(NodeId, NodeId), Vec<EdgeGeometry>>,
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
    pub fn get_edge(&self, from: NodeId, to: NodeId) -> Option<&[EdgeGeometry]> {
        self.edges.get(&(from, to)).map(|es| es.as_slice())
    }
}
