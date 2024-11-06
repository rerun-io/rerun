use re_types::{
    components::{self, GraphNode},
    ArrowString,
};

use crate::graph::NodeIndex;

pub struct NodeInstance {
    pub node: components::GraphNode,
    pub index: NodeIndex,
    pub label: Option<ArrowString>,
    pub color: Option<egui::Color32>,
    pub position: Option<egui::Pos2>,
    pub radius: Option<components::Radius>,
}

pub struct EdgeInstance {
    pub source: GraphNode,
    pub target: GraphNode,
    pub source_index: NodeIndex,
    pub target_index: NodeIndex,
}

impl EdgeInstance {
    pub fn nodes(&self) -> impl Iterator<Item = &components::GraphNode> {
        [&self.source, &self.target].into_iter()
    }
}
