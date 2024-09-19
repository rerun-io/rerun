use re_log_types::EntityPath;
use re_types_core::ComponentName;

impl super::ComponentColumnSelector {
    /// Create a [`Self`] from an [`EntityPath`] and a [`ComponentName`].
    pub fn new(entity_path: &EntityPath, component_name: ComponentName) -> Self {
        crate::blueprint::datatypes::ComponentColumnSelector {
            entity_path: entity_path.into(),
            component_name: component_name.as_str().into(),
        }
        .into()
    }
}
