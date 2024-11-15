use re_log_types::EntityPath;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a [`re_types_core::ComponentName`] expressed as string.
    pub fn new(entity_path: &EntityPath, component_name: &str) -> Self {
        Self {
            entity_path: entity_path.into(),
            component: component_name.into(),
        }
    }
}
