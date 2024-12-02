//! Performs the layout of the graph, i.e. converting an [`LayoutRequest`] into a [`Layout`].

// For now we have only a single layout provider that is based on a force-directed model.
// In the future, this could be expanded to support different (specialized layout alorithms).
// Low-hanging fruit would be tree-based layouts. But we could also think about more complex
// layouts, such as `dot` from `graphviz`.

use egui::{Pos2, Rect, Vec2};
use fjadra::{self as fj};

use crate::graph::{EdgeId, NodeId};

use super::{
    request::NodeTemplate,
    slots::{slotted_edges, Slot, SlotKind},
    EdgeGeometry, EdgeTemplate, Layout, LayoutRequest, PathGeometry,
};

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
            .filter(|(id, _)| !id.is_self_edge())
            .map(|(id, _)| (node_index[&id.source], node_index[&id.target]));

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
            entities: Vec::new(),
        }
    }

    /// Returns `true` if finished.
    pub fn tick(&mut self, layout: &mut Layout) -> bool {
        self.simulation.tick(1);

        let positions = self.simulation.positions().collect::<Vec<_>>();

        // We clear all unnecessary data deom the previous layout, but keep its space allocated.
        layout.entities.clear();
        layout.edges.clear();

        for (entity, graph) in &self.request.graphs {
            let mut current_rect = Rect::NOTHING;

            for node in graph.nodes.keys() {
                let extent = layout.nodes.get_mut(node).expect("node has to be present");
                let i = self.node_index[node];
                let [x, y] = positions[i];
                let pos = Pos2::new(x as f32, y as f32);
                extent.set_center(pos);
                current_rect = current_rect.union(*extent);
            }

            layout.entities.push((entity.clone(), current_rect));

            // Multiple edges can occupy the same space in the layout.
            for Slot { kind, edges } in
                slotted_edges(graph.edges.values().flat_map(|ts| ts.iter())).values()
            {
                match kind {
                    SlotKind::SelfEdge { node } => {
                        let rect = layout.nodes[node];
                        let id = EdgeId::self_edge(*node);
                        let geometries = layout.edges.entry(id).or_default();
                        geometries.extend(layout_self_edges(rect, edges));
                    }
                    SlotKind::Regular {
                        source: slot_source,
                        target: slot_target,
                    } => {
                        if let &[edge] = edges.as_slice() {
                            // A single regular straight edge.
                            let target_arrow = edge.target_arrow;
                            let geometries = layout
                                .edges
                                .entry(EdgeId {
                                    source: edge.source,
                                    target: edge.target,
                                })
                                .or_default();
                            geometries.push(EdgeGeometry {
                                target_arrow,
                                path: line_segment(
                                    layout.nodes[&edge.source],
                                    layout.nodes[&edge.target],
                                ),
                            });
                        } else {
                            // Multiple edges occupy the same space, so we fan them out.
                            let num_edges = edges.len();

                            // Controls the amount of space (in scene coordinates) that a slot can occupy.
                            let fan_amount = 20.0;

                            for (i, edge) in edges.iter().enumerate() {
                                let source_rect = layout.nodes[slot_source];
                                let target_rect = layout.nodes[slot_target];

                                let d = (target_rect.center() - source_rect.center()).normalized();

                                let source_pos = source_rect.intersects_ray_from_center(d);
                                let target_pos = target_rect.intersects_ray_from_center(-d);

                                // How far along the edge should the control points be?
                                let c1_base = source_pos + (target_pos - source_pos) * 0.25;
                                let c2_base = source_pos + (target_pos - source_pos) * 0.75;

                                let c1_base_n = Vec2::new(-c1_base.y, c1_base.x).normalized();
                                let mut c2_base_n = Vec2::new(-c2_base.y, c2_base.x).normalized();

                                // Make sure both point to the same side of the edge.
                                if c1_base_n.dot(c2_base_n) < 0.0 {
                                    // If they point in opposite directions, flip one of them.
                                    c2_base_n = -c2_base_n;
                                }

                                let c1_left = c1_base + c1_base_n * (fan_amount / 2.);
                                let c2_left = c2_base + c2_base_n * (fan_amount / 2.);

                                let c1_right = c1_base - c1_base_n * (fan_amount / 2.);
                                let c2_right = c2_base - c2_base_n * (fan_amount / 2.);

                                // Calculate an offset for the control points based on index `i`, spreading points equidistantly.
                                let t = (i as f32) / (num_edges - 1) as f32;

                                // Compute control points, `c1` and `c2`, based on the offset
                                let c1 = c1_right + (c1_left - c1_right) * t;
                                let c2 = c2_right + (c2_left - c2_right) * t;

                                let geometries = layout
                                    .edges
                                    .entry(EdgeId {
                                        source: edge.source,
                                        target: edge.target,
                                    })
                                    .or_default();

                                // We potentially need to restore the direction of the edge, after we have used it's canonical form earlier.
                                let path = if edge.source == *slot_source {
                                    PathGeometry::CubicBezier {
                                        source: source_pos,
                                        target: target_pos,
                                        control: [c1, c2],
                                    }
                                } else {
                                    PathGeometry::CubicBezier {
                                        source: target_pos,
                                        target: source_pos,
                                        control: [c2, c1],
                                    }
                                };

                                geometries.push(EdgeGeometry {
                                    target_arrow: edge.target_arrow,
                                    path,
                                });
                            }
                        }
                    }
                }
            }
        }

        self.simulation.finished()
    }
}

/// Helper function to calculate the line segment between two rectangles.
fn line_segment(source: Rect, target: Rect) -> PathGeometry {
    let source_center = source.center();
    let target_center = target.center();

    // Calculate direction vector from source to target
    let direction = (target_center - source_center).normalized();

    // Find the border points on both rectangles
    let source_point = source.intersects_ray_from_center(direction);
    let target_point = target.intersects_ray_from_center(-direction); // Reverse direction for target

    PathGeometry::Line {
        source: source_point,
        target: target_point,
    }
}

fn layout_self_edges<'a>(
    rect: Rect,
    edges: &'a [&EdgeTemplate],
) -> impl Iterator<Item = EdgeGeometry> + 'a {
    edges.iter().enumerate().map(move |(i, edge)| {
        let offset = (i + 1) as f32;
        let target_arrow = edge.target_arrow;
        let anchor = rect.center_top();

        EdgeGeometry {
            target_arrow,
            path: PathGeometry::CubicBezier {
                // TODO(grtlr): We could probably consider the actual node size here.
                source: anchor + Vec2::LEFT * 4.,
                target: anchor + Vec2::RIGHT * 4.,
                // TODO(grtlr): The actual length of that spline should follow the `distance` parameter of the link force.
                control: [
                    anchor + Vec2::new(-30. * offset, -40. * offset),
                    anchor + Vec2::new(30. * offset, -40. * offset),
                ],
            },
        }
    })
}
