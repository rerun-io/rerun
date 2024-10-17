use std::collections::HashMap;

use re_viewer::external::egui;

use crate::{error::Error, graph::NodeIndex};

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Force;

impl Force {
    pub fn compute(
        &self,
        nodes: impl IntoIterator<Item = (NodeIndex, egui::Vec2)>,
    ) -> Result<HashMap<NodeIndex, egui::Rect>, Error> {
        let nodes = nodes.into_iter().collect::<Vec<_>>();

        let particles = vec![[0.0, 0.0]; nodes.len()];

        let mut sim = re_force::Simulation::new(particles);

        let res = sim.tick(1_000);

        Ok(nodes
            .into_iter()
            .zip(res.iter())
            .map(|((node_id, _), particle)| {
                let center = particle.pos;
                let rect = egui::Rect::from_center_size(center, egui::Vec2::new(10.0, 10.0));
                (node_id, rect)
            })
            .collect())
    }
}
