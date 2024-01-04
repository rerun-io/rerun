use re_entity_db::EntityDb;

/// The current Blueprint and Recording being displayed by the viewer
pub struct StoreContext<'a> {
    pub blueprint: &'a EntityDb,
    pub recording: Option<&'a EntityDb>,
    pub all_recordings: Vec<&'a EntityDb>,
}
