use re_entity_db::{EntityDb, StoreBundle};
use re_log_types::{ApplicationId, StoreId};

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
    pub bundle: &'a StoreBundle,
}

impl StoreContext<'_> {
    pub fn is_active(&self, store_id: &StoreId) -> bool {
        self.recording.store_id() == store_id || self.blueprint.store_id() == store_id
    }
}
