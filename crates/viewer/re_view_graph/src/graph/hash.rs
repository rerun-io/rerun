use re_log_types::hash::Hash64;
use re_sdk_types::components;

/// A 64 bit hash of [`components::GraphNode`] with very small risk of collision.
#[derive(Copy, Clone, Eq, PartialOrd, Ord)]
pub struct GraphNodeHash(Hash64);

impl nohash_hasher::IsEnabled for GraphNodeHash {}

impl GraphNodeHash {
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0.hash64()
    }
}

// We implement `Hash` manually, because `nohash_hasher` requires a single call to `state.write_*`.
// More info: https://crates.io/crates/nohash-hasher
impl std::hash::Hash for GraphNodeHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash64());
    }
}

impl std::fmt::Debug for GraphNodeHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeIdHash({:016X})", self.hash64())
    }
}

impl From<&components::GraphNode> for GraphNodeHash {
    fn from(node_id: &components::GraphNode) -> Self {
        Self(Hash64::hash(node_id))
    }
}

impl std::cmp::PartialEq for GraphNodeHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
