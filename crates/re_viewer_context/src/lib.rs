//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

mod annotations;
mod app_options;
mod caches;
mod component_ui_registry;
mod item;
mod scene_query;
mod selection_history;
mod selection_state;
mod tensor;
mod time_control;
mod utils;
mod viewer_context;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;

pub use annotations::{AnnotationMap, Annotations, ResolvedAnnotationInfo, MISSING_ANNOTATIONS};
pub use app_options::AppOptions;
pub use caches::{Cache, Caches};
pub use component_ui_registry::{ComponentUiRegistry, UiVerbosity};
pub use item::{Item, ItemCollection};
pub use scene_query::SceneQuery;
pub use selection_history::SelectionHistory;
pub use selection_state::{
    HoverHighlight, HoveredSpace, InteractionHighlight, SelectionHighlight, SelectionState,
};
pub use tensor::{TensorDecodeCache, TensorStats, TensorStatsCache};
pub use time_control::{Looping, PlayState, TimeControl, TimeView};
pub use utils::{auto_color, level_to_rich_text, DefaultColor};
pub use viewer_context::{RecordingConfig, ViewerContext};

#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::Clipboard;

// ---------------------------------------------------------------------------

/// A unique id for each space view.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]

pub struct SpaceViewId(uuid::Uuid);

impl SpaceViewId {
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn random_from_str(s: &str) -> Self {
        // TODO(jleibs): This is overkill; we can probably get away with 64-bit ids.
        use rand::{Rng as _, SeedableRng as _};
        use std::hash::{Hash as _, Hasher as _};

        let salt: u64 = 0x307b_e149_0a3a_5552;

        let mut hasher = std::collections::hash_map::DefaultHasher::default();
        salt.hash(&mut hasher);
        s.hash(&mut hasher);
        let mut rng = rand::rngs::StdRng::seed_from_u64(hasher.finish());
        let uuid = uuid::Builder::from_random_bytes(rng.gen()).into_uuid();

        Self(uuid)
    }

    pub fn gpu_readback_id(self) -> re_renderer::GpuReadbackIdentifier {
        re_log_types::hash::Hash64::hash(self).hash64()
    }
}

impl std::fmt::Display for SpaceViewId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#}", self.0)
    }
}

slotmap::new_key_type! {
    /// Identifier for a blueprint group.
    pub struct DataBlueprintGroupHandle;
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}
