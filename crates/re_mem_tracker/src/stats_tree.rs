use std::collections::BTreeMap;

use crate::AllocStats;

/// Describes a tree of allocation stats
#[derive(Clone, Default)]
pub struct Tree {
    /// Statistics for this node (maybe leaf).
    pub stats: AllocStats,

    /// Children, if any.
    pub children: BTreeMap<String, Tree>,
}

impl Tree {
    pub fn child(&mut self, child: impl Into<String>) -> &mut Tree {
        self.children.entry(child.into()).or_default()
    }
}
