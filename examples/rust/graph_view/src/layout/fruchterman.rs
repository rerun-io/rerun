use std::collections::HashMap;

use rand::distributions::Distribution as _;
use fdg::{nalgebra::Point2, Force as _};
use re_viewer::external::egui;

use crate::{error::Error, types::NodeIndex};

use super::Layout;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FruchtermanReingoldLayout;

impl Layout for FruchtermanReingoldLayout {
    type NodeIx = NodeIndex;

    fn compute(
        &self,
        nodes: impl IntoIterator<Item = (Self::NodeIx, egui::Vec2)>,
        directed: impl IntoIterator<Item = (Self::NodeIx, Self::NodeIx)>,
        undirected: impl IntoIterator<Item = (Self::NodeIx, Self::NodeIx)>,
    ) -> Result<HashMap<Self::NodeIx, egui::Rect>, Error> {
        let mut node_to_index = HashMap::new();
        let mut graph: fdg::ForceGraph<f32, 2, (Self::NodeIx, egui::Vec2), ()> =
            fdg::ForceGraph::default();

        for (node_id, size) in nodes {

            let dist = fdg::rand_distributions::Uniform::new(-10.0, 10.0);

            let ix = graph.add_node(((node_id.clone(), size), Point2::new(dist.sample(&mut rand::thread_rng()), dist.sample(&mut rand::thread_rng()))));
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
        fdg::fruchterman_reingold::FruchtermanReingold::default().apply_many(&mut graph, 100);
        // Center the graph's average around (0,0).
        fdg::simple::Center::default().apply(&mut graph);

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
