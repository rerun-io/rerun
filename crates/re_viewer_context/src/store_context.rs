use re_entity_db::EntityDb;
use re_log_types::ApplicationId;

/// The current Blueprint and Recording being displayed by the viewer
pub struct StoreContext<'a> {
    pub app_id: ApplicationId,
    pub blueprint: &'a EntityDb,
    pub recording: Option<&'a EntityDb>,
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
