//! Rerun Viewer context
//!
//! This crate contains data structures that are shared with most modules of the viewer.

mod annotations;
mod app_options;
mod caches;
mod command_sender;
mod component_ui_registry;
mod item;
mod selection_history;
mod selection_state;
mod space_view;
mod store_context;
mod tensor;
mod time_control;
mod utils;
mod viewer_context;

// TODO(andreas): Move to its own crate?
pub mod gpu_bridge;

use std::hash::BuildHasher;

pub use annotations::{
    AnnotationMap, Annotations, ResolvedAnnotationInfo, ResolvedAnnotationInfos,
    MISSING_ANNOTATIONS,
};
pub use app_options::AppOptions;
pub use caches::{Cache, Caches};
pub use command_sender::{
    command_channel, CommandReceiver, CommandSender, FileContents, SystemCommand,
    SystemCommandSender,
};
pub use component_ui_registry::{ComponentUiRegistry, UiVerbosity};
pub use item::{resolve_mono_instance_path, resolve_mono_instance_path_item, Item, ItemCollection};
pub use selection_history::SelectionHistory;
pub use selection_state::{
    HoverHighlight, HoveredSpace, InteractionHighlight, SelectionHighlight, SelectionState,
};
pub use space_view::{
    ArchetypeDefinition, AutoSpawnHeuristic, DynSpaceViewClass, NamedViewSystem, PerSystemEntities,
    SpaceViewClass, SpaceViewClassLayoutPriority, SpaceViewClassName, SpaceViewClassRegistry,
    SpaceViewClassRegistryError, SpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewOutlineMasks, SpaceViewState, SpaceViewSystemExecutionError, SpaceViewSystemRegistry,
    ViewContextCollection, ViewContextSystem, ViewPartCollection, ViewPartSystem, ViewQuery,
    ViewSystemName,
};
pub use store_context::StoreContext;
pub use tensor::{TensorDecodeCache, TensorStats, TensorStatsCache};
pub use time_control::{Looping, PlayState, TimeControl, TimeView};
pub use utils::{auto_color, level_to_rich_text, DefaultColor};
pub use viewer_context::{RecordingConfig, ViewerContext};

#[cfg(not(target_arch = "wasm32"))]
mod clipboard;
#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::Clipboard;

pub mod external {
    pub use {re_arrow_store, re_data_store, re_log_types, re_query, re_ui};
}

// ---------------------------------------------------------------------------

/// A unique id for each space view.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]

pub struct SpaceViewId(uuid::Uuid);

impl SpaceViewId {
    pub fn invalid() -> Self {
        Self(uuid::Uuid::nil())
    }

    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn hashed_from_str(s: &str) -> Self {
        use std::hash::{Hash as _, Hasher as _};

        let salt1: u64 = 0x307b_e149_0a3a_5552;
        let salt2: u64 = 0x6651_522f_f510_13a4;

        let hash1 = {
            let mut hasher = ahash::RandomState::with_seeds(1, 2, 3, 4).build_hasher();
            salt1.hash(&mut hasher);
            s.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            let mut hasher = ahash::RandomState::with_seeds(1, 2, 3, 4).build_hasher();
            salt2.hash(&mut hasher);
            s.hash(&mut hasher);
            hasher.finish()
        };

        let uuid = uuid::Uuid::from_u64_pair(hash1, hash2);

        Self(uuid)
    }

    pub fn gpu_readback_id(self) -> re_renderer::GpuReadbackIdentifier {
        re_log_types::hash::Hash64::hash(self).hash64()
    }
}

impl std::fmt::Display for SpaceViewId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#}", self.0)
    }
}

slotmap::new_key_type! {
    /// Identifier for a blueprint group.
    pub struct DataBlueprintGroupHandle;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NeedsRepaint {
    Yes,
    No,
}
