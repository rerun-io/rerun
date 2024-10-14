use re_log_types::hash::Hash64;
use re_viewer::external::re_types::datatypes;

/// A 64 bit hash of [`GraphNodeId`] with very small risk of collision.
#[derive(Copy, Clone, Eq, PartialOrd, Ord)]
pub(crate) struct NodeIdHash(Hash64);

impl NodeIdHash {
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0.hash64()
    }
}

impl std::hash::Hash for NodeIdHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl std::fmt::Debug for NodeIdHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeIdHash({:016X})", self.hash64())
    }
}

impl From<&datatypes::GraphNodeId> for NodeIdHash {
    fn from(node_id: &datatypes::GraphNodeId) -> Self {
        Self(Hash64::hash(node_id))
    }
}

impl std::cmp::PartialEq for NodeIdHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
