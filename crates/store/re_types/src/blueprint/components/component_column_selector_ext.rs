use re_log_types::EntityPath;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a component expressed as string.
    pub fn new(entity_path: &EntityPath, component: String) -> Self {
        crate::blueprint::datatypes::ComponentColumnSelector::new(entity_path, component).into()
    }
}
