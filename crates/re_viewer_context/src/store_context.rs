use re_entity_db::EntityDb;
use re_log_types::ApplicationId;

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

    /// In no specific order.
    pub all_recordings: Vec<&'a EntityDb>,
}

impl<'a> StoreContext<'a> {
    pub fn recording(&self, store_id: &re_log_types::StoreId) -> Option<&'a EntityDb> {
        self.all_recordings
            .iter()
            .find(|rec| rec.store_id() == store_id)
            .copied()
    }
}
