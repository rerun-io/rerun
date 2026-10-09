//! Loading configuration: entity-path prefix, timeline name, video mode.

use re_chunk::{EntityPath, TimelineName};

/// Configuration for [`crate::LeRobotDataset::stream`].
#[derive(Debug, Clone)]
pub struct LeRobotConfig {
    /// Prefix prepended to every emitted entity path (defaults to the root `/`).
    pub entity_path_prefix: EntityPath,

    /// Name of the per-episode timeline.
    ///
    /// `None` → named after the column it is derived from: `frame_index` when present
    /// (a sequence timeline), otherwise `timestamp` (a duration timeline).
    pub timeline_name: Option<TimelineName>,

    /// How video features are emitted.
    pub video: VideoMode,
}

impl Default for LeRobotConfig {
    fn default() -> Self {
        Self {
            entity_path_prefix: EntityPath::root(),
            timeline_name: None,
            video: VideoMode::Native,
        }
    }
}

/// How video features are emitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VideoMode {
    /// Emit videos as a `VideoStream`: the whole per-episode file for v2, the episode's
    /// slice of the shared file for v3.
    #[default]
    Native,

    /// Skip all video features.
    Skip,
}
