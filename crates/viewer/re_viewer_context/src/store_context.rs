use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, StoreId};

use crate::Caches;

/// The current Blueprint and Recording being displayed by the viewer
pub struct StoreContext<'a> {
    /// The current active blueprint.
    pub blueprint: &'a EntityDb,

    /// The default blueprint (i.e. the one logged from code), if any.
    pub default_blueprint: Option<&'a EntityDb>,

    /// The current active recording.
    ///
    /// If none is active, this will point to a dummy empty recording.
    pub recording: &'a EntityDb,

    /// Things that need caching.
    pub caches: &'a Caches,

    /// Should we enable the heuristics during this frame?
    pub should_enable_heuristics: bool,
}

impl StoreContext<'_> {
    pub fn is_active(&self, store_id: &StoreId) -> bool {
        self.recording.store_id() == store_id || self.blueprint.store_id() == store_id
    }

    pub fn application_id(&self) -> &ApplicationId {
        self.recording.application_id()
    }

    pub fn recording_store_id(&self) -> &StoreId {
        self.recording.store_id()
    }
}
