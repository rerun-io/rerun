use std::collections::HashMap;

use fdg::{nalgebra::Point2, Force as _};
use rand::distributions::Distribution as _;
use re_viewer::external::egui;

use crate::{error::Error, graph::NodeIndex};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FruchtermanReingoldLayout;

impl FruchtermanReingoldLayout {
    pub fn compute(
        &self,
        nodes: impl IntoIterator<Item = (NodeIndex, egui::Vec2)>,
        directed: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
        undirected: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
    ) -> Result<HashMap<NodeIndex, egui::Rect>, Error> {
        let mut node_to_index = HashMap::new();
        let mut graph: fdg::ForceGraph<f32, 2, (NodeIndex, egui::Vec2), ()> =
            fdg::ForceGraph::default();

        for (node_id, size) in nodes {
            let dist = fdg::rand_distributions::Uniform::new(-10.0, 10.0);

            let ix = graph.add_node((
                (node_id.clone(), size),
                Point2::new(
                    dist.sample(&mut rand::thread_rng()),
                    dist.sample(&mut rand::thread_rng()),
                ),
            ));
            node_to_index.insert(node_id, ix);
        }

        for (source, target) in directed.into_iter().chain(undirected) {
            let source_ix = node_to_index.get(&source).ok_or(Error::EdgeUnknownNode)?;
            let target_ix = node_to_index.get(&target).ok_or(Error::EdgeUnknownNode)?;
            graph.add_edge(*source_ix, *target_ix, ());
        }

        // create a simulation from the graph
        fdg::fruchterman_reingold::FruchtermanReingold::default().apply_many(&mut graph, 1000);
        // Center the graph's average around (0,0).
        fdg::simple::Center::default().apply_many(&mut graph, 100);

        let res = graph
            .node_weights()
            .map(|(data, pos)| {
                let (ix, size) = data;
                let center = egui::Pos2::new(pos.x, pos.y);
                let rect = egui::Rect::from_center_size(center, *size);
                (ix.clone(), rect)
            })
            .collect();

        Ok(res)
    }
}
