use egui::Rect;
use fjadra::{Center, Link, ManyBody, PositionX, PositionY, SimulationBuilder};
use re_chunk::{EntityPath, TimeInt, Timeline};

use crate::{
    graph::NodeIndex,
    ui::bounding_rect_from_iter,
    visualizers::{all_edges, all_nodes, EdgeData, NodeData},
};

/// Used to determine if a layout is up-to-date or outdated.
#[derive(Debug, PartialEq, Eq)]
pub struct Timestamp {
    timeline: Timeline,
    time: TimeInt,
}

pub struct Layout {
    valid_at: Timestamp,
    extents: ahash::HashMap<NodeIndex, Rect>,
}

impl std::fmt::Debug for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Layout")
            .field("valid_at", &self.valid_at)
            .finish()
    }
}

impl Layout {
    pub fn needs_update(&self, timeline: Timeline, time: TimeInt) -> bool {
        self.valid_at.timeline != timeline || self.valid_at.time != time
    }

    pub fn bounding_rect(&self) -> Rect {
        bounding_rect_from_iter(self.extents.values())
    }

    pub fn get(&self, node: &NodeIndex) -> Option<&Rect> {
        self.extents.get(node)
    }

    pub fn update(&mut self, node: &NodeIndex, rect: Rect) {
        *self
            .extents
            .get_mut(node)
            .expect("node should exist in layout") = rect;
    }
}

pub struct LayoutProvider;

impl LayoutProvider {
    pub fn compute<'a>(
        timeline: Timeline,
        time: TimeInt,
        nodes: impl IntoIterator<Item = (&'a EntityPath, &'a NodeData)>,
        edges: impl IntoIterator<Item = (&'a EntityPath, &'a EdgeData)> + Clone,
    ) -> Layout {
        // Will hold the positions of the nodes, stored as bounding rectangles.
        let mut extents = ahash::HashMap::default();

        let nodes = all_nodes(nodes, edges.clone())
            .map(|n| n.1)
            .collect::<Vec<NodeIndex>>();
        let node_index: ahash::HashMap<NodeIndex, usize> =
            nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();

        let edges = all_edges(edges)
            .map(|(_, (source, target))| (node_index[&source], node_index[&target]));

        let mut simulation = SimulationBuilder::default()
            .build(nodes.iter().map(|_| Option::<[f64; 2]>::None))
            .add_force("link", Link::new(edges))
            .add_force("charge", ManyBody::new().strength(-300.0))
            .add_force("x", PositionX::new())
            .add_force("y", PositionY::new());

        let positions = simulation.iter().last().expect("simulation should run");
        for (node, i) in node_index {
            extents.entry(node).or_insert_with(|| {
                let pos = positions[i];
                let pos = egui::Pos2::new(pos[0] as f32, pos[1] as f32);
                let size = egui::Vec2::ZERO;
                egui::Rect::from_min_size(pos, size)
            });
        }

        Layout {
            valid_at: Timestamp { timeline, time },
            extents,
        }
    }
}
