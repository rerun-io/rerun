use re_log_types::EntityPath;
use re_viewer_context::{DataResultNode, DataResultTree};

/// Helper for presenting [`DataResultNode`]s within a larger entity-based tree.
///
/// The blueprint tree presents data results within possibly larger tree structures. For example,
/// all "in-tree" data results are presented within the subtree defined by the view origin. Yet
/// part of this subtree may not be a [`DataResultNode`] by itself. This enum can represent any node
/// of any subtree, regardless of whether it is a [`DataResultNode`] or note.
pub enum DataResultNodeOrPath<'a> {
    Path(&'a EntityPath),
    DataResultNode(&'a DataResultNode),
}

impl<'a> DataResultNodeOrPath<'a> {
    pub fn from_path_lookup(result_tree: &'a DataResultTree, path: &'a EntityPath) -> Self {
        result_tree
            .lookup_node_by_path(path.hash())
            .map_or(DataResultNodeOrPath::Path(path), |node| {
                DataResultNodeOrPath::DataResultNode(node)
            })
    }

    pub fn path(&self) -> &'a EntityPath {
        match self {
            DataResultNodeOrPath::Path(path) => path,
            DataResultNodeOrPath::DataResultNode(node) => &node.data_result.entity_path,
        }
    }

    pub fn data_result_node(&self) -> Option<&'a DataResultNode> {
        match self {
            DataResultNodeOrPath::Path(_) => None,
            DataResultNodeOrPath::DataResultNode(node) => Some(node),
        }
    }
}
