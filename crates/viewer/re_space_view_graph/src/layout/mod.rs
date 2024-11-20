use egui::{Pos2, Rect, Vec2};
use fjadra as fj;

use crate::{
    graph::{Graph, NodeIndex},
    ui::bounding_rect_from_iter,
    visualizers::NodeInstance,
};

#[derive(Debug, PartialEq, Eq)]
pub struct Layout {
    extents: ahash::HashMap<NodeIndex, Rect>,
}

impl Layout {
    pub fn bounding_rect(&self) -> Rect {
        bounding_rect_from_iter(self.extents.values().copied())
    }

    pub fn get(&self, node: &NodeIndex) -> Option<Rect> {
        self.extents.get(node).copied()
    }

    pub fn update(&mut self, node: &NodeIndex, rect: Rect) {
        *self
            .extents
            .get_mut(node)
            .expect("node should exist in layout") = rect;
    }

    /// Returns `true` if any node has a zero size.
    pub fn has_zero_size(&self) -> bool {
        self.extents.values().any(|r| r.size() == Vec2::ZERO)
    }
}

impl<'a> From<&'a NodeInstance> for fj::Node {
    fn from(instance: &'a NodeInstance) -> Self {
        let mut node = Self::default();
        if let Some(pos) = instance.position {
            node = node.fixed_position(pos.x as f64, pos.y as f64);
        }
        node
    }
}

pub struct ForceLayout {
    simulation: fj::Simulation,
    node_index: ahash::HashMap<NodeIndex, usize>,
}

impl ForceLayout {
    pub fn new<'a>(graphs: impl Iterator<Item = &'a Graph<'a>> + Clone) -> Self {
        let explicit = graphs
            .clone()
            .flat_map(|g| g.nodes_explicit().map(|n| (n.index, fj::Node::from(n))));
        let implicit = graphs
            .clone()
            .flat_map(|g| g.nodes_implicit().map(|n| (n.index, fj::Node::default())));

        let mut node_index = ahash::HashMap::default();
        let all_nodes: Vec<fj::Node> = explicit
            .chain(implicit)
            .enumerate()
            .map(|(i, n)| {
                node_index.insert(n.0, i);
                n.1
            })
            .collect();

        let all_edges = graphs.flat_map(|g| {
            g.edges()
                .map(|e| (node_index[&e.source_index], node_index[&e.target_index]))
        });

        // TODO(grtlr): Currently we guesstimate good forces. Eventually these should be exposed as blueprints.
        let simulation = fj::SimulationBuilder::default()
            .with_alpha_decay(0.01) // TODO(grtlr): slows down the simulation for demo
            .build(all_nodes)
            .add_force("link", fj::Link::new(all_edges))
            .add_force("charge", fj::ManyBody::new())
            // TODO(grtlr): detect if we need a center force or a position force, depending on if the graph is disjoint
            // or not. More generally: how much heuristics do we want to bake in here?
            // .add_force("center", fj::Center::new());
            // TODO(grtlr): This is a small stop-gap until we have blueprints.
            .add_force("x", fj::PositionX::new().strength(0.01))
            .add_force("y", fj::PositionY::new().strength(0.01));

        Self {
            simulation,
            node_index,
        }
    }

    pub fn init_layout(&self) -> Layout {
        let positions = self.simulation.positions().collect::<Vec<_>>();
        let mut extents = ahash::HashMap::default();

        for (node, i) in &self.node_index {
            let [x, y] = positions[*i];
            let pos = Pos2::new(x as f32, y as f32);
            let size = Vec2::ZERO;
            let rect = Rect::from_min_size(pos, size);
            extents.insert(*node, rect);
        }

        Layout { extents }
    }

    /// Returns `true` if finished.
    pub fn tick(&mut self, layout: &mut Layout) -> bool {
        self.simulation.tick(1);

        let positions = self.simulation.positions().collect::<Vec<_>>();

        for (node, extent) in &mut layout.extents {
            let i = self.node_index[node];
            let [x, y] = positions[i];
            let pos = Pos2::new(x as f32, y as f32);
            extent.set_center(pos);
        }

        self.simulation.finished()
    }
}
