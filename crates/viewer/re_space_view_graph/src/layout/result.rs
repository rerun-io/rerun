use egui::{Pos2, Rect};

use crate::graph::NodeId;

pub type LineSegment = [Pos2; 2];

fn bounding_rect_points(points: impl IntoIterator<Item = impl Into<Pos2>>) -> egui::Rect {
    points
        .into_iter()
        .fold(egui::Rect::NOTHING, |mut acc, pos| {
            acc.extend_with(pos.into());
            acc
        })
}

#[derive(Clone, Debug)]
pub enum EdgeGeometry {
    Line {
        start: Pos2,
        end: Pos2,
    },
    /// Represents a cubic bezier curve.
    ///
    /// In the future we could probably support more complex splines.
    CubicBezier {
        start: Pos2,
        end: Pos2,
        control: [Pos2; 2],
    },
    // We could add other geometries, such as `Orthogonal` here too.
}

impl EdgeGeometry {
    pub fn bounding_rect(&self) -> Rect {
        match self {
            EdgeGeometry::Line { start, end } => Rect::from_two_pos(*start, *end),
            // TODO(grtlr): This is just a crude (upper) approximation, as the resulting bounding box can be too large.
            EdgeGeometry::CubicBezier {
                start,
                end,
                ref control,
            } => Rect::from_points(&[&[*start, *end], control.as_slice()].concat()),
        }
    }
}

#[derive(Debug)]
pub struct Layout {
    pub(super) nodes: ahash::HashMap<NodeId, Rect>,
    pub(super) edges: ahash::HashMap<(NodeId, NodeId), EdgeGeometry>,
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
    pub fn get_edge(&self, from: NodeId, to: NodeId) -> Option<&EdgeGeometry> {
        self.edges.get(&(from, to))
    }
}
