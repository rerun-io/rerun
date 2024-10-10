use std::collections::HashMap;

use re_viewer::external::egui;

use crate::{error::Error, types::NodeIndex};

mod dot;
pub(crate) use dot::DotLayout;
mod force_directed;
pub(crate) use force_directed::ForceBasedLayout;

pub(crate) trait Layout {
    type NodeIx: Clone + Eq + std::hash::Hash;

    fn compute(
        &self,
        nodes: impl IntoIterator<Item = (Self::NodeIx, egui::Vec2)>,
        directed: impl IntoIterator<Item = (Self::NodeIx, Self::NodeIx)>,
        undirected: impl IntoIterator<Item = (Self::NodeIx, Self::NodeIx)>,
    ) -> Result<HashMap<Self::NodeIx, egui::Rect>, Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LayoutProvider {
    Dot(DotLayout),
    ForceDirected(ForceBasedLayout),
}

impl LayoutProvider {
    pub(crate) fn new_dot() -> Self {
        LayoutProvider::Dot(Default::default())
    }

    pub(crate) fn new_force_directed() -> Self {
        LayoutProvider::ForceDirected(Default::default())
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
        }
    }
}
