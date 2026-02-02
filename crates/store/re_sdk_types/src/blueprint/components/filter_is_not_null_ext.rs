use re_log_types::EntityPath;

use super::FilterIsNotNull;

impl FilterIsNotNull {
    /// Create a new [`Self`].
    pub fn new(active: bool, entity_path: &EntityPath, column_name: String) -> Self {
        let datatype = crate::blueprint::datatypes::FilterIsNotNull {
            active: active.into(),
            column: crate::blueprint::datatypes::ComponentColumnSelector::new(
                entity_path,
                column_name,
            ),
        };

        Self(datatype)
    }

    /// Is the filter active?
    pub fn active(&self) -> bool {
        self.active.into()
    }

    /// Entity path of the filter column.
    pub fn entity_path(&self) -> EntityPath {
        self.column.entity_path()
    }

    /// Component column selector of the filter column
    pub fn column_selector(&self) -> re_sorbet::ComponentColumnSelector {
        self.column.column_selector()
    }
}
