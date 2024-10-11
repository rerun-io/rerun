use re_log_types::EntityPath;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a [`ComponentName`].
    pub fn new(entity_path: &EntityPath, component_name: &str) -> Self {
        crate::blueprint::datatypes::ComponentColumnSelector::new(entity_path, component_name)
            .into()
    }
}
