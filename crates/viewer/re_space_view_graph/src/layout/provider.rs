//! Performs the layout of the graph, i.e. converting an [`LayoutRequest`] into a [`Layout`].

// For now we have only a single layout provider that is based on a force-directed model.
// In the future, this could be expanded to support different (specialized layout alorithms).
// Low-hanging fruit would be tree-based layouts. But we could also think about more complex
// layouts, such as `dot` from `graphviz`.

use egui::{Pos2, Rect, Vec2};
use fjadra::{self as fj, Simulation};

use crate::graph::{EdgeId, NodeId};

use super::{
    params::ForceLayoutParams,
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

// TODO(grtlr): Do this more efficiently, as this currently rebuilds all helper functions.
pub fn update_simulation(
    mut simulation: fj::Simulation,
    params: &ForceLayoutParams,
    edges: Vec<(usize, usize)>,
    radii: Vec<f64>,
) -> Simulation {
    // We destructure here to get compiler warnings if we add new parameters.
    let &ForceLayoutParams {
        force_link_enabled,
        force_link_distance,
        force_link_iterations,
        force_many_body_enabled,
        force_many_body_strength,
        force_position_enabled,
        force_position_strength,
        force_position_pos,
        force_center_enabled,
        force_center_strength,
        force_collision_enabled,
        force_collision_strength,
        force_collision_iterations,
    } = params;

    if **force_link_enabled {
        simulation = simulation.add_force(
            "link",
            fj::Link::new(edges)
                .distance(**force_link_distance)
                .iterations(**force_link_iterations as usize),
        );
    }
    if **force_many_body_enabled {
        simulation = simulation.add_force(
            "charge",
            fj::ManyBody::new().strength(**force_many_body_strength),
        );
    }
    if **force_position_enabled {
        simulation = simulation
            .add_force(
                "x",
                fj::PositionX::new()
                    .strength(**force_position_strength)
                    .x(force_position_pos[0].into()),
            )
            .add_force(
                "y",
                fj::PositionY::new()
                    .strength(**force_position_strength)
                    .y(force_position_pos[1].into()),
            );
    }
    if **force_collision_enabled {
        simulation = simulation.add_force(
            "collision",
            fj::Collide::new()
                .radius(move |i| radii[i])
                .iterations(**force_collision_iterations as usize)
                .strength(**force_collision_strength),
        );
    }
    if **force_center_enabled {
        simulation = simulation.add_force(
            "center",
            fj::Center::new().strength(**force_center_strength),
        );
    }
    simulation
}

pub struct ForceLayoutProvider {
    simulation: fj::Simulation,
    pub request: LayoutRequest,
}

fn considered_edges(request: &LayoutRequest) -> Vec<(usize, usize)> {
    let node_index: ahash::HashMap<NodeId, usize> = request
        .all_nodes()
        .enumerate()
        .map(|(i, (id, _))| (id, i))
        .collect();
    request
        .all_edges()
        .filter(|(id, _)| !id.is_self_edge())
        .map(|(id, _)| (node_index[&id.source], node_index[&id.target]))
        .collect()
}

impl ForceLayoutProvider {
    pub fn new(request: LayoutRequest, params: &ForceLayoutParams) -> Self {
        let nodes = request.all_nodes().map(|(_, v)| fj::Node::from(v));
        let radii = request
            .all_nodes()
            .map(|(_, v)| v.size.max_elem() as f64 / 2.0)
            .collect::<Vec<_>>();
        let edges = considered_edges(&request);

        let simulation = fj::SimulationBuilder::default().build(nodes);

        let simulation = update_simulation(simulation, params, edges, radii);

        Self {
            simulation,
            request,
        }
    }

    pub fn new_with_previous(
        request: LayoutRequest,
        layout: &Layout,
        params: &ForceLayoutParams,
    ) -> Self {
        let nodes = request.all_nodes().map(|(id, v)| {
            if let Some(rect) = layout.get_node(&id) {
                let pos = rect.center();
                fj::Node::from(v).position(pos.x as f64, pos.y as f64)
            } else {
                fj::Node::from(v)
            }
        });
        let radii = request
            .all_nodes()
            .map(|(_, v)| v.size.max_elem() as f64 / 2.0)
            .collect::<Vec<_>>();
        let edges = considered_edges(&request);

        // TODO(grtlr): Currently we guesstimate good forces. Eventually these should be exposed as blueprints.
        let simulation = fj::SimulationBuilder::default().build(nodes);

        let simulation = update_simulation(simulation, params, edges, radii);

        Self {
            simulation,
            request,
        }
    }

    fn layout(&self) -> Layout {
        // We make use of the fact here that the simulation is stable, i.e. the
        // order of the nodes is the same as in the `request`.
        let mut positions = self.simulation.positions();

        let mut layout = Layout::empty();

        for (entity, graph) in &self.request.graphs {
            let mut current_rect = Rect::NOTHING;

            for (node, template) in &graph.nodes {
                let [x, y] = positions.next().expect("positions has to match the layout");
                let pos = Pos2::new(x as f32, y as f32);
                let extent = Rect::from_center_size(pos, template.size);
                current_rect = current_rect.union(extent);
                layout.nodes.insert(*node, extent);
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

        layout
    }

    /// Returns `true` if finished.
    pub fn tick(&mut self) -> Layout {
        self.simulation.tick(1);
        self.layout()
    }

    pub fn is_finished(&self) -> bool {
        self.simulation.is_finished()
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
