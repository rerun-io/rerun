use re_log_types::EntityPath;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a column name.
    pub fn new(entity_path: &EntityPath, component: String) -> Self {
        Self {
            entity_path: entity_path.into(),
            component: component.into(),
        }
    }

    /// The parsed entity path.
    pub fn entity_path(&self) -> EntityPath {
        EntityPath::from(self.entity_path.as_str())
    }

    /// The parsed omponent column selector.
    pub fn column_selector(&self) -> re_sorbet::ComponentColumnSelector {
        let entity_path = EntityPath::from(self.entity_path.as_str());
        let component = self.component.to_string();
        re_sorbet::ComponentColumnSelector {
            entity_path,
            component,
        }
    }
}
