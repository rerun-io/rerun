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

pub struct ForceLayout;

impl ForceLayout {
    pub fn compute<'a>(graphs: impl Iterator<Item = &'a Graph<'a>> + Clone) -> Layout {
        let explicit =
            graphs.clone().flat_map(|g| g.nodes_explicit().map(|n| (n.index, fj::Node::from(n))));
        let implicit =
            graphs.clone().flat_map(|g| g.nodes_implicit().map(|n| (n.index, fj::Node::default())));

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

        let mut simulation = fj::SimulationBuilder::default()
            .build(all_nodes)
            .add_force("link", fj::Link::new(all_edges))
            .add_force("charge", fj::ManyBody::new().strength(-300.0))
            .add_force("x", fj::PositionX::new())
            .add_force("y", fj::PositionY::new());

        let positions = simulation.iter().last().expect("simulation should run");

        let extents = node_index
            .into_iter()
            .map(|(n, i)| {
                let [x, y] = positions[i];
                let pos = Pos2::new(x as f32, y as f32);
                (n, Rect::from_center_size(pos, Vec2::ZERO))
            })
            .collect();

        Layout { extents }
    }
}
