use re_log_types::{EntityPath, Instance};
use re_types::{datatypes, ArrowString};

use crate::graph::NodeIndex;

impl<'a> EdgeInstance<'a> {
    pub fn nodes(&'a self) -> impl Iterator<Item = datatypes::GraphLocation> {
        [self.source.clone(), self.target.clone()].into_iter()
    }
}

impl<'a> From<&NodeInstance<'a>> for NodeIndex {
    fn from(node: &NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.into(),
        }
    }
}

impl<'a> From<NodeInstance<'a>> for NodeIndex {
    fn from(node: NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.into(),
        }
    }
}

pub(crate) struct NodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNodeId,
    pub entity_path: &'a EntityPath,
    pub instance: Instance,
    pub show_labels: bool,
    pub label: Option<&'a ArrowString>,
    pub color: Option<egui::Color32>,
}

pub struct EdgeInstance<'a> {
    pub source: &'a datatypes::GraphLocation,
    pub target: &'a datatypes::GraphLocation,
    pub _entity_path: &'a re_log_types::EntityPath,
    pub instance: Instance,
    pub color: Option<egui::Color32>,
}

pub(crate) struct UnknownNodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNodeId,
    pub entity_path: &'a EntityPath,
}

impl<'a> From<&UnknownNodeInstance<'a>> for NodeIndex {
    fn from(node: &UnknownNodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.into(),
        }
    }
}

impl<'a> From<UnknownNodeInstance<'a>> for NodeIndex {
    fn from(node: UnknownNodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.into(),
        }
    }
}
