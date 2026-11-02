use re_types_core::{ComponentDescriptor, ComponentType};

use super::ColumnType;

impl From<ComponentType> for ColumnType {
    #[inline]
    fn from(component_type: ComponentType) -> Self {
        Self {
            component: None,
            component_type_name: Some(component_type.as_str().into()),
        }
    }
}

impl From<ComponentDescriptor> for ColumnType {
    #[inline]
    fn from(descriptor: ComponentDescriptor) -> Self {
        Self {
            component: Some(descriptor.component.as_str().into()),
            component_type_name: descriptor
                .component_type
                .map(|component_type| component_type.as_str().into()),
        }
    }
}
