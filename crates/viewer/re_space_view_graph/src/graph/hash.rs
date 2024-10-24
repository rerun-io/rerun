use re_log_types::hash::Hash64;
use re_types::datatypes;

/// A 64 bit hash of [`GraphNodeId`] with very small risk of collision.
#[derive(Copy, Clone, Eq, PartialOrd, Ord)]
pub(crate) struct GraphNodeHash(Hash64);

impl GraphNodeHash {
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0.hash64()
    }
}

impl std::hash::Hash for GraphNodeHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl std::fmt::Debug for GraphNodeHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeIdHash({:016X})", self.hash64())
    }
}

impl From<&datatypes::GraphNode> for GraphNodeHash {
    fn from(node_id: &datatypes::GraphNode) -> Self {
        Self(Hash64::hash(node_id))
    }
}

impl std::cmp::PartialEq for GraphNodeHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
