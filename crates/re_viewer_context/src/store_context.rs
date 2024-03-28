use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{ApplicationId, StoreId};

use crate::StoreHub;

/// The current Blueprint and Recording being displayed by the viewer
pub struct StoreContext<'a> {
    /// The `app_id` of the current recording
    pub app_id: ApplicationId,

    /// The current active recording.
    pub blueprint: &'a EntityDb,

    /// The current open recording.
    ///
    /// If none is open, this will point to a dummy empty recording.
    pub recording: &'a EntityDb,

    /// All the loaded recordings and blueprints.
    ///
    /// This is the same bundle as is in [`Self::hub`], but extracted for ease-of-access.
    pub bundle: &'a StoreBundle,

    /// The store hub, which keeps track of all the default and active blueprints, among other things.
    pub hub: &'a StoreHub,

    /// The current default blueprint
    pub default_blueprint: Option<&'a StoreId>,
}

impl StoreContext<'_> {
    pub fn is_active(&self, store_id: &StoreId) -> bool {
        self.recording.store_id() == store_id || self.blueprint.store_id() == store_id
    }

    pub fn is_default_blueprint(&self, store_id: &StoreId) -> bool {
        self.default_blueprint == Some(store_id)
    }
}
