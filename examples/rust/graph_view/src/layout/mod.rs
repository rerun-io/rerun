use std::collections::HashMap;

use re_viewer::external::egui;

use crate::error::Error;

mod force_directed;
pub(crate) use force_directed::ForceBasedLayout;

pub(crate) trait LayoutProvider {
    type NodeIx: Clone + Eq + std::hash::Hash;

    fn name() -> &'static str;

    fn compute(
        &self,
        nodes: impl IntoIterator<Item = (Self::NodeIx, egui::Vec2)>,
        directed: impl IntoIterator<Item = (Self::NodeIx, Self::NodeIx)>,
        undirected: impl IntoIterator<Item = (Self::NodeIx, Self::NodeIx)>,
    ) -> Result<HashMap<Self::NodeIx, egui::Rect>, Error>;
}
