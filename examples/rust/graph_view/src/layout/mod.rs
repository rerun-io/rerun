use std::collections::HashMap;

use re_viewer::external::egui;

use crate::{error::Error, graph::NodeIndex};

mod dot;
pub(crate) use dot::DotLayout;
mod force_directed;
pub(crate) use force_directed::ForceBasedLayout;
mod fruchterman;
pub(crate) use fruchterman::FruchtermanReingoldLayout;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LayoutProvider {
    Dot(DotLayout),
    ForceDirected(ForceBasedLayout),
    FruchtermanReingold(FruchtermanReingoldLayout),
}

impl LayoutProvider {
    pub(crate) fn new_dot() -> Self {
        LayoutProvider::Dot(Default::default())
    }

    pub(crate) fn new_force_directed() -> Self {
        LayoutProvider::ForceDirected(Default::default())
    }

    pub(crate) fn new_fruchterman_reingold() -> Self {
        LayoutProvider::FruchtermanReingold(Default::default())
    }
}

impl LayoutProvider {
    pub(crate) fn compute(
        &self,
        nodes: impl IntoIterator<Item = (NodeIndex, egui::Vec2)>,
        directed: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
        undirected: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
    ) -> Result<HashMap<NodeIndex, egui::Rect>, Error> {
        match self {
            LayoutProvider::Dot(layout) => layout.compute(nodes, directed, undirected),
            LayoutProvider::ForceDirected(layout) => layout.compute(nodes, directed, undirected),
            LayoutProvider::FruchtermanReingold(layout) => {
                layout.compute(nodes, directed, undirected)
            }
        }
    }
}

impl Default for LayoutProvider {
    fn default() -> Self {
        LayoutProvider::new_fruchterman_reingold()
    }
}
