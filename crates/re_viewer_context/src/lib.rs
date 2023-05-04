//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

mod app_options;
mod caches;
mod component_ui_registry;
mod item;
mod selection_history;
mod selection_state;
mod time_control;
mod viewer_context;

pub use app_options::AppOptions;
pub use caches::{Cache, Caches};
pub use component_ui_registry::{ComponentUiRegistry, UiVerbosity};
pub use item::{Item, ItemCollection};
pub use selection_history::SelectionHistory;
pub use selection_state::{
    HoverHighlight, HoveredSpace, InteractionHighlight, SelectionHighlight, SelectionState,
};
pub use time_control::{Looping, PlayState, TimeControl, TimeView};
pub use viewer_context::{RecordingConfig, ViewerContext};

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

    pub fn gpu_readback_id(self) -> re_renderer::GpuReadbackIdentifier {
        re_log_types::hash::Hash64::hash(self).hash64()
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
