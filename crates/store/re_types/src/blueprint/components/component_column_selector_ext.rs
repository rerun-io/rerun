use re_log_types::EntityPath;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a [`re_types_core::ComponentName`] expressed as string.
    pub fn new(entity_path: &EntityPath, component_name: String) -> Self {
        crate::blueprint::datatypes::ComponentColumnSelector::new(entity_path, component_name)
            .into()
    }
}
