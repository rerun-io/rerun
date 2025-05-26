use re_log_types::EntityPath;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a [`re_types_core::ComponentName`] expressed as string.
    pub fn new(entity_path: &EntityPath, component_name: &str) -> Self {
        Self {
            entity_path: entity_path.into(),
            component: component_name.into(),
        }
    }

    /// Returns the column name for this component selector.
    ///
    /// This is typically used to resolve dataframe queries.
    pub fn column_name(&self) -> String {
        // TODO(#10129): This needs to be adapted once the blueprint changes.
        format!("{}:{}", self.entity_path.as_str(), self.component.as_str(),)
    }
}
