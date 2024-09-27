use re_viewer::external::re_types::datatypes;

#[derive(PartialEq, Eq, Hash)]
pub(crate) struct QualifiedNode {
    pub entity_path: re_log_types::EntityPath,
    pub node_id: datatypes::GraphNodeId,
}

pub(crate) struct QualifiedEdge {
    pub source: QualifiedNode,
    pub target: QualifiedNode,
}
