use egui::{Pos2, Rect, Vec2};
use fjadra as fj;

use crate::graph::{Graph, Node, NodeIndex};

use super::Layout;

impl<'a> From<&'a Node> for fj::Node {
    fn from(node: &'a Node) -> Self {
        match node {
            Node::Explicit {
                position: Some(pos),
                ..
            } => Self::default().fixed_position(pos.x as f64, pos.y as f64),
            _ => Self::default(),
        }
    }
}

pub struct ForceLayoutProvider {
    simulation: fj::Simulation,
    node_index: ahash::HashMap<NodeIndex, usize>,
}

impl ForceLayoutProvider {
    pub fn new(graphs: &[Graph]) -> Self {
        let nodes = graphs
            .iter()
            .flat_map(|g| g.nodes().iter().map(|n| (n.id(), fj::Node::from(n))));

        let mut node_index = ahash::HashMap::default();
        let all_nodes: Vec<fj::Node> = nodes
            .enumerate()
            .map(|(i, n)| {
                node_index.insert(n.0, i);
                n.1
            })
            .collect();

        let all_edges = graphs.iter().flat_map(|g| {
            g.edges()
                .iter()
                .map(|e| (node_index[&e.from], node_index[&e.to]))
        });

        // TODO(grtlr): Currently we guesstimate good forces. Eventually these should be exposed as blueprints.
        let simulation = fj::SimulationBuilder::default()
            .with_alpha_decay(0.01) // TODO(grtlr): slows down the simulation for demo
            .build(all_nodes)
            .add_force(
                "link",
                fj::Link::new(all_edges).distance(50.0).iterations(2),
            )
            .add_force("charge", fj::ManyBody::new())
            // TODO(grtlr): This is a small stop-gap until we have blueprints to prevent nodes from flying away.
            .add_force("x", fj::PositionX::new().strength(0.01))
            .add_force("y", fj::PositionY::new().strength(0.01));

        Self {
            simulation,
            node_index,
        }
    }

    pub fn init(&self) -> Layout {
        let positions = self.simulation.positions().collect::<Vec<_>>();
        let mut extents = ahash::HashMap::default();

        for (node, i) in &self.node_index {
            let [x, y] = positions[*i];
            let pos = Pos2::new(x as f32, y as f32);
            let size = Vec2::ZERO;
            let rect = Rect::from_min_size(pos, size);
            extents.insert(*node, rect);
        }

        Layout { nodes: extents, edges: ahash::HashMap::default() }
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

        self.simulation.finished()
    }
}
