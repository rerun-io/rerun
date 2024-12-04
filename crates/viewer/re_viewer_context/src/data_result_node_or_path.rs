use re_chunk::EntityPath;

use crate::{DataResultNode, DataResultTree};

// TODO(@Wumpf): document this
pub enum DataResultNodeOrPath<'a> {
    Path(&'a EntityPath),
    DataResultNode(&'a DataResultNode),
}

impl<'a> DataResultNodeOrPath<'a> {
    pub fn from_path_lookup(result_tree: &'a DataResultTree, path: &'a EntityPath) -> Self {
        result_tree
            .lookup_node_by_path(path)
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
