use re_entity_db::EntityDb;
use re_log_types::{ApplicationId, StoreId};

use crate::{Caches, StoreBundle, StoreHub};

/// The current Blueprint and Recording being displayed by the viewer
pub struct StoreContext<'a> {
    /// The `app_id` of the current recording.
    pub app_id: ApplicationId,

    /// The current active blueprint.
    pub blueprint: &'a EntityDb,

    /// The default blueprint (i.e. the one logged from code), if any.
    pub default_blueprint: Option<&'a EntityDb>,

    /// The current active recording.
    ///
    /// If none is active, this will point to a dummy empty recording.
    pub recording: &'a EntityDb,

    /// All the loaded recordings and blueprints.
    ///
    /// This is the same bundle as is in [`Self::hub`], but extracted for ease-of-access.
    pub bundle: &'a StoreBundle,

    /// Things that need caching.
    pub caches: &'a Caches,

    /// The store hub, which keeps track of all the default and active blueprints, among other things.
    pub hub: &'a StoreHub,

    /// Should we enable the heuristics during this frame?
    pub should_enable_heuristics: bool,
}

impl StoreContext<'_> {
    pub fn is_active(&self, store_id: &StoreId) -> bool {
        self.recording.store_id() == *store_id || self.blueprint.store_id() == *store_id
    }
}
