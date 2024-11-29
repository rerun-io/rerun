use egui::{Pos2, Rect, Vec2};
use fjadra as fj;

use crate::graph::NodeId;

use super::{request::NodeTemplate, result::EdgeGeometry, Layout, LayoutRequest};

impl<'a> From<&'a NodeTemplate> for fj::Node {
    fn from(node: &'a NodeTemplate) -> Self {
        match node.fixed_position {
            Some(pos) => Self::default().fixed_position(pos.x as f64, pos.y as f64),
            _ => Self::default(),
        }
    }
}

pub struct ForceLayoutProvider {
    simulation: fj::Simulation,
    node_index: ahash::HashMap<NodeId, usize>,
    pub request: LayoutRequest,
}

impl ForceLayoutProvider {
    pub fn new(request: LayoutRequest) -> Self {
        let nodes = request.graphs.iter().flat_map(|(_, graph_template)| {
            graph_template
                .nodes
                .iter()
                .map(|n| (n.0, fj::Node::from(n.1)))
        });

        let mut node_index = ahash::HashMap::default();
        let all_nodes: Vec<fj::Node> = nodes
            .enumerate()
            .map(|(i, n)| {
                node_index.insert(*n.0, i);
                n.1
            })
            .collect();

        let all_edges_iter = request
            .graphs
            .iter()
            .flat_map(|(_, graph_template)| graph_template.edges.iter());

        // Looking at self-edges does not make sense in a force-based layout, so we filter those out.
        let considered_edges = all_edges_iter
            .clone()
            .filter(|((from, to), _)| (from != to))
            .map(|((from, to), _)| (node_index[from], node_index[to]));

        // TODO(grtlr): Currently we guesstimate good forces. Eventually these should be exposed as blueprints.
        let simulation = fj::SimulationBuilder::default()
            .with_alpha_decay(0.01) // TODO(grtlr): slows down the simulation for demo
            .build(all_nodes)
            .add_force(
                "link",
                fj::Link::new(considered_edges).distance(50.0).iterations(2),
            )
            .add_force("charge", fj::ManyBody::new())
            // TODO(grtlr): This is a small stop-gap until we have blueprints to prevent nodes from flying away.
            .add_force("x", fj::PositionX::new().strength(0.01))
            .add_force("y", fj::PositionY::new().strength(0.01));

        Self {
            simulation,
            node_index,
            request,
        }
    }

    pub fn init(&self) -> Layout {
        let positions = self.simulation.positions().collect::<Vec<_>>();
        let mut extents = ahash::HashMap::default();

        for graph in self.request.graphs.values() {
            for (id, node) in &graph.nodes {
                let i = self.node_index[id];
                let [x, y] = positions[i];
                let pos = Pos2::new(x as f32, y as f32);
                extents.insert(*id, Rect::from_center_size(pos, node.size));
            }
        }

        Layout {
            nodes: extents,
            // Without any real node positions, we probably don't want to draw edges either.
            edges: ahash::HashMap::default(),
        }
    }

    /// Returns `true` if finished.
    pub fn tick(&mut self, layout: &mut Layout) -> bool {
        self.simulation.tick(1);

        let positions = self.simulation.positions().collect::<Vec<_>>();

        for (node, extent) in &mut layout.nodes {
            let i = self.node_index[node];
            let [x, y] = positions[i];
            let pos = Pos2::new(x as f32, y as f32);
            extent.set_center(pos);
        }

        // TODO(grtlr): Better to create slots for each edge.
        for ((from, to), template) in self.request.graphs.values().flat_map(|g| g.edges.iter()) {
            let geometries = layout.edges.entry((*from, *to)).or_default();
            match from == to {
                true => {
                    for i in 1..=template.len() {
                        // Self-edges are not supported in force-based layouts.
                        let anchor = intersects_ray_from_center(layout.nodes[from], Vec2::UP);
                        geometries.push(EdgeGeometry::CubicBezier {
                            // TODO(grtlr): We could probably consider the actual node size here.
                            source: anchor + Vec2::LEFT * 4.,
                            target: anchor + Vec2::RIGHT * 4.,
                            // TODO(grtlr): The actual length of that spline should follow the `distance` parameter of the link force.
                            control: [
                                anchor + Vec2::new(-50. * i as f32, -60. * i as f32),
                                anchor + Vec2::new(50. * i as f32, -60. * i as f32),
                            ],
                        });
                    }
                }
                false => {
                    geometries.push(line_segment(layout.nodes[from], layout.nodes[to]));
                }
            }
        }

        self.simulation.finished()
    }
}

/// Helper function to calculate the line segment between two rectangles.
fn line_segment(source: Rect, target: Rect) -> EdgeGeometry {
    let source_center = source.center();
    let target_center = target.center();

    // Calculate direction vector from source to target
    let direction = (target_center - source_center).normalized();

    // Find the border points on both rectangles
    let source_point = intersects_ray_from_center(source, direction);
    let target_point = intersects_ray_from_center(target, -direction); // Reverse direction for target

    EdgeGeometry::Line {
        source: source_point,
        target: target_point,
    }
}

/// Helper function to find the point where the line intersects the border of a rectangle
fn intersects_ray_from_center(rect: Rect, direction: Vec2) -> Pos2 {
    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;

    for i in 0..2 {
        let inv_d = 1.0 / -direction[i];
        let mut t0 = (rect.min[i] - rect.center()[i]) * inv_d;
        let mut t1 = (rect.max[i] - rect.center()[i]) * inv_d;

        if inv_d < 0.0 {
            std::mem::swap(&mut t0, &mut t1);
        }

        tmin = tmin.max(t0);
        tmax = tmax.min(t1);
    }

    let t = tmax.min(tmin); // Pick the first intersection
    rect.center() + t * -direction
}

#[cfg(test)]
mod test {
    use super::*;
    use egui::pos2;

    #[test]
    fn test_ray_intersection() {
        let rect = Rect::from_min_max(pos2(1.0, 1.0), pos2(3.0, 3.0));

        assert_eq!(
            intersects_ray_from_center(rect, Vec2::RIGHT),
            pos2(3.0, 2.0),
            "rightward ray"
        );

        assert_eq!(
            intersects_ray_from_center(rect, Vec2::UP),
            pos2(2.0, 1.0),
            "upward ray"
        );

        assert_eq!(
            intersects_ray_from_center(rect, Vec2::LEFT),
            pos2(1.0, 2.0),
            "leftward ray"
        );

        assert_eq!(
            intersects_ray_from_center(rect, Vec2::DOWN),
            pos2(2.0, 3.0),
            "downward ray"
        );

        assert_eq!(
            intersects_ray_from_center(rect, (Vec2::LEFT + Vec2::DOWN).normalized()),
            pos2(1.0, 3.0),
            "bottom-left corner ray"
        );

        assert_eq!(
            intersects_ray_from_center(rect, (Vec2::LEFT + Vec2::UP).normalized()),
            pos2(1.0, 1.0),
            "top-left corner ray"
        );

        assert_eq!(
            intersects_ray_from_center(rect, (Vec2::RIGHT + Vec2::DOWN).normalized()),
            pos2(3.0, 3.0),
            "bottom-right corner ray"
        );

        assert_eq!(
            intersects_ray_from_center(rect, (Vec2::RIGHT + Vec2::UP).normalized()),
            pos2(3.0, 1.0),
            "top-right corner ray"
        );
    }
}
