use super::VisualizerInstructionId;
use crate::datatypes::Uuid;

impl VisualizerInstructionId {
    /// Create an invalid visualizer instruction ID (nil UUID).
    #[inline]
    pub fn invalid() -> Self {
        Self(Uuid::from(uuid::Uuid::nil()))
    }

    /// Generate a new random visualizer instruction ID.
    #[inline]
    pub fn new_random() -> Self {
        Self(Uuid::random())
    }

    /// Create a deterministic visualizer instruction ID from a hash and index.
    ///
    /// This is used internally for generating stable IDs for heuristically
    /// created visualizers.
    #[inline]
    pub fn new_deterministic(hash: u64, index: usize) -> Self {
        Self(Uuid::from(uuid::Uuid::from_u64_pair(hash, index as u64)))
    }
}

impl std::fmt::Display for VisualizerInstructionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
