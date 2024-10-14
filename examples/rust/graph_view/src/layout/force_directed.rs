use std::collections::HashMap;

use fdg_sim::{self as fdg, ForceGraphHelper};
use re_viewer::external::egui;

use crate::{error::Error, types::NodeIndex};

#[deprecated]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ForceBasedLayout;

impl ForceBasedLayout {
    pub fn compute(
        &self,
        nodes: impl IntoIterator<Item = (NodeIndex, egui::Vec2)>,
        directed: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
        undirected: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
    ) -> Result<HashMap<NodeIndex, egui::Rect>, Error> {
        let mut node_to_index = HashMap::new();
        let mut graph: fdg::ForceGraph<(NodeIndex, egui::Vec2), ()> = fdg::ForceGraph::default();

        for (node_id, size) in nodes {
            let ix = graph.add_force_node(node_id.to_string(), (node_id.clone(), size));
            node_to_index.insert(node_id, ix);
        }

        for (source, target) in directed.into_iter().chain(undirected) {
            let source_ix = node_to_index
                .get(&source)
                .ok_or_else(|| Error::EdgeUnknownNode(source.to_string()))?;
            let target_ix = node_to_index
                .get(&target)
                .ok_or_else(|| Error::EdgeUnknownNode(source.to_string()))?;
            graph.add_edge(*source_ix, *target_ix, ());
        }

        // create a simulation from the graph
        let mut simulation =
            fdg::Simulation::from_graph(graph, fdg::SimulationParameters::default());

        for _ in 0..1000 {
            simulation.update(0.035);
        }

        let res = simulation
            .get_graph()
            .node_weights()
            .map(|fdg::Node { data, location, .. }| {
                let (ix, size) = data;
                let center = egui::Pos2::new(location.x, location.y);
                let rect = egui::Rect::from_center_size(center, *size);
                (ix.clone(), rect)
            })
            .collect();

        Ok(res)
    }
}
