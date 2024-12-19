//! Provides geometric (shape) abstractions for the different elements of a graph layout.

use egui::{Pos2, Rect, Vec2};

#[derive(Clone, Debug)]
pub enum PathGeometry {
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

#[derive(Debug)]
pub struct EdgeGeometry {
    pub target_arrow: bool,
    pub path: PathGeometry,
}

impl EdgeGeometry {
    pub fn bounding_rect(&self) -> Rect {
        match self.path {
            PathGeometry::Line { source, target } => Rect::from_two_pos(source, target),
            // TODO(grtlr): This is just a crude (upper) approximation, as the resulting bounding box can be too large.
            // For now this is fine, as there are no interactions on edges yet.
            PathGeometry::CubicBezier {
                source,
                target,
                ref control,
            } => Rect::from_points(&[&[source, target], control.as_slice()].concat()),
        }
    }

    /// The starting position of an edge.
    #[expect(unused)]
    pub fn source_pos(&self) -> Pos2 {
        match self.path {
            PathGeometry::Line { source, .. } | PathGeometry::CubicBezier { source, .. } => source,
        }
    }

    /// The end position of an edge.
    pub fn target_pos(&self) -> Pos2 {
        match self.path {
            PathGeometry::Line { target, .. } | PathGeometry::CubicBezier { target, .. } => target,
        }
    }

    /// The direction of the edge at the source node (normalized).
    #[expect(unused)]
    pub fn source_arrow_direction(&self) -> Vec2 {
        use PathGeometry::{CubicBezier, Line};
        match self.path {
            Line { source, target } => (source.to_vec2() - target.to_vec2()).normalized(),
            CubicBezier {
                source, control, ..
            } => (control[0].to_vec2() - source.to_vec2()).normalized(),
        }
    }

    /// The direction of the edge at the target node (normalized).
    pub fn target_arrow_direction(&self) -> Vec2 {
        use PathGeometry::{CubicBezier, Line};
        match self.path {
            Line { source, target } => (target.to_vec2() - source.to_vec2()).normalized(),
            CubicBezier {
                target, control, ..
            } => (target.to_vec2() - control[1].to_vec2()).normalized(),
        }
    }
}
