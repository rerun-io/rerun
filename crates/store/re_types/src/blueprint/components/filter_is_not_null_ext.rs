use re_log_types::EntityPath;
use re_types_core::ComponentName;

use super::FilterIsNotNull;

impl FilterIsNotNull {
    /// Create a new [`Self`].
    pub fn new(active: bool, entity_path: &EntityPath, component_name: ComponentName) -> Self {
        let datatype = crate::blueprint::datatypes::FilterIsNotNull {
            active: active.into(),
            column: crate::blueprint::datatypes::ComponentColumnSelector {
                entity_path: entity_path.to_string().into(),
                component: component_name.as_str().into(),
            },
        };

        Self(datatype)
    }

    /// Is the filter active?
    pub fn active(&self) -> bool {
        self.active.into()
    }

    /// Entity path of the filter column.
    pub fn entity_path(&self) -> EntityPath {
        EntityPath::from(self.column.entity_path.as_str())
    }

    /// Component name of the filter column.
    pub fn component_name(&self) -> ComponentName {
        self.column.component.as_str().into()
    }
}
