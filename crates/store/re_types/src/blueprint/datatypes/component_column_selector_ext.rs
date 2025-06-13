use re_log_types::EntityPath;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a column name.
    pub fn new(entity_path: &EntityPath, column_name: String) -> Self {
        Self {
            entity_path: entity_path.into(),
            component: column_name.into(),
        }
    }

    /// The parsed entity path.
    pub fn entity_path(&self) -> EntityPath {
        EntityPath::from(self.entity_path.as_str())
    }

    /// The parsed omponent column selector.
    pub fn column_selector(&self) -> re_sorbet::ComponentColumnSelector {
        let entity_path = EntityPath::from(self.entity_path.as_str());
        let column_name = self.component.as_str();
        match column_name.rfind(':') {
            Some(i) => re_sorbet::ComponentColumnSelector {
                entity_path,
                archetype_name: Some(column_name[..i].into()),
                archetype_field_name: column_name[(i + 1)..].into(),
            },
            None => re_sorbet::ComponentColumnSelector {
                entity_path,
                archetype_field_name: column_name.to_owned(),
                archetype_name: None,
            },
        }
    }
}
