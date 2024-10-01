use std::hash::Hash;

use re_viewer::external::re_types::datatypes;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct QualifiedNode {
    pub entity_hash: re_log_types::EntityPathHash,
    pub node_id: datatypes::GraphNodeId,
}

impl std::fmt::Debug for QualifiedNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.node_id, self.entity_hash)
    }
}

#[derive(Debug, Hash)]
pub(crate) struct QualifiedEdge {
    pub source: QualifiedNode,
    pub target: QualifiedNode,
}
