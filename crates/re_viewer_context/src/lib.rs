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
    command_channel, CommandReceiver, CommandSender, SystemCommand, SystemCommandSender,
};
pub use component_ui_registry::{ComponentUiRegistry, UiVerbosity};
pub use item::{resolve_mono_instance_path, resolve_mono_instance_path_item, Item, ItemCollection};
use nohash_hasher::{IntMap, IntSet};
use once_cell::sync::Lazy;
use re_log_types::{EntityPath, EntityPathPart, Index};
pub use selection_history::SelectionHistory;
pub use selection_state::{
    HoverHighlight, HoveredSpace, InteractionHighlight, SelectionHighlight, SelectionState,
};
pub use space_view::{
    default_heuristic_filter, AutoSpawnHeuristic, DataResult, DynSpaceViewClass,
    HeuristicFilterContext, NamedViewSystem, PerSystemDataResults, PerSystemEntities,
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
    pub use nohash_hasher;
    pub use {re_arrow_store, re_data_store, re_log_types, re_query, re_ui};
}

// ---------------------------------------------------------------------------

pub type EntitiesPerSystem = IntMap<ViewSystemName, IntSet<EntityPath>>;

pub type EntitiesPerSystemPerClass = IntMap<SpaceViewClassName, EntitiesPerSystem>;

pub trait BlueprintIdRegistry {
    fn registry() -> &'static EntityPath;
}

macro_rules! define_blueprint_id_registry {
    ($type:ident, $registry:ident, $registry_name:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $registry;

        impl $registry {
            const REGISTRY: &'static str = $registry_name;
        }

        impl BlueprintIdRegistry for $registry {
            fn registry() -> &'static EntityPath {
                static REGISTRY_PATH: Lazy<EntityPath> = Lazy::new(|| $registry::REGISTRY.into());
                &REGISTRY_PATH
            }
        }
        pub type $type = BlueprintId<$registry>;
    };
}

define_blueprint_id_registry!(SpaceViewId, SpaceViewIdRegistry, "space_view");

/// A unique id for each space view.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]

pub struct BlueprintId<T: BlueprintIdRegistry> {
    id: uuid::Uuid,
    #[serde(skip)]
    _registry: std::marker::PhantomData<T>,
}

impl<T: BlueprintIdRegistry> BlueprintId<T> {
    pub fn invalid() -> Self {
        Self {
            id: uuid::Uuid::nil(),
            _registry: std::marker::PhantomData,
        }
    }

    pub fn random() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            _registry: std::marker::PhantomData,
        }
    }

    pub fn from_entity_path(path: &EntityPath) -> Self {
        debug_assert!(path.is_child_of(T::registry()));
        path.last()
            .and_then(|last| uuid::Uuid::parse_str(last.to_string().as_str()).ok())
            .map_or(Self::invalid(), |id| Self {
                id,
                _registry: std::marker::PhantomData,
            })
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

        uuid.into()
    }

    pub fn gpu_readback_id(self) -> re_renderer::GpuReadbackIdentifier {
        re_log_types::hash::Hash64::hash(self.id).hash64()
    }

    #[inline]
    pub fn as_entity_path(&self) -> EntityPath {
        T::registry()
            .iter()
            .cloned()
            .chain(std::iter::once(EntityPathPart::Index(Index::Uuid(self.id))))
            .collect()
    }

    #[inline]
    pub fn registry() -> &'static EntityPath {
        T::registry()
    }
}

impl<T: BlueprintIdRegistry> From<uuid::Uuid> for BlueprintId<T> {
    #[inline]
    fn from(id: uuid::Uuid) -> Self {
        Self {
            id,
            _registry: std::marker::PhantomData,
        }
    }
}

impl<T: BlueprintIdRegistry> std::fmt::Display for BlueprintId<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({:#})", T::registry(), self.id)
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
