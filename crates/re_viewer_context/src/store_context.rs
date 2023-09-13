use re_data_store::StoreDb;

/// The current Blueprint and Recording being displayed by the viewer
pub struct StoreContext<'a> {
    pub blueprint: &'a StoreDb,
    pub recording: Option<&'a StoreDb>,
    pub all_recordings: Vec<&'a StoreDb>,
}
