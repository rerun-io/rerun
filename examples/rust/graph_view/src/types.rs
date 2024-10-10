use re_log_types::{EntityPath, EntityPathHash, Instance};
use re_viewer::external::{
    egui,
    re_types::{datatypes, ArrowString},
};

impl<'a> EdgeInstance<'a> {
    pub fn nodes(&'a self) -> impl Iterator<Item = datatypes::GraphLocation> {
        [self.source.clone(), self.target.clone()].into_iter()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct NodeIndex {
    pub entity_hash: EntityPathHash,
    pub node_id: datatypes::GraphNodeId,
}

impl From<datatypes::GraphLocation> for NodeIndex {
    fn from(location: datatypes::GraphLocation) -> Self {
        Self {
            entity_hash: EntityPath::from(location.entity_path).hash(),
            node_id: location.node_id,
        }
    }
}

impl From<&datatypes::GraphLocation> for NodeIndex {
    fn from(location: &datatypes::GraphLocation) -> Self {
        Self {
            entity_hash: EntityPath::from(location.entity_path.clone()).hash(),
            node_id: location.node_id.clone(),
        }
    }
}

impl std::fmt::Display for NodeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.node_id, self.entity_hash)
    }
}

impl<'a> From<&NodeInstance<'a>> for NodeIndex {
    fn from(node: &NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.clone(),
        }
    }
}

pub(crate) struct NodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNodeId,
    pub entity_path: &'a EntityPath,
    pub instance: Instance,
    pub label: Option<&'a ArrowString>,
    pub color: Option<egui::Color32>,
}

pub struct EdgeInstance<'a> {
    pub source: &'a datatypes::GraphLocation,
    pub target: &'a datatypes::GraphLocation,
    pub entity_path: &'a re_log_types::EntityPath,
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
            node_id: node.node_id.clone(),
        }
    }
}
