use re_types_core::{ComponentDescriptor, ComponentName};

use crate::path::EntityPath;

/// A [`EntityPath`] plus a [`ComponentName`].
///
/// Example: `camera/left/points:Color`
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ComponentPath {
    /// `camera / "left" / points / #42`
    pub entity_path: EntityPath,

    /// e.g. `Points3D:Color#color`
    pub component_descriptor: ComponentDescriptor,
}

impl ComponentPath {
    #[inline]
    pub fn new(entity_path: EntityPath, component_descriptor: ComponentDescriptor) -> Self {
        Self {
            entity_path,
            component_descriptor,
        }
    }

    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }

    #[inline]
    pub fn component_name(&self) -> ComponentName {
        self.component_descriptor.component_name
    }

    #[inline]
    pub fn component_descriptor(&self) -> &ComponentDescriptor {
        &self.component_descriptor
    }
}

impl std::fmt::Display for ComponentPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.entity_path.fmt(f)?;
        f.write_str(":")?;
        self.component_descriptor.fmt(f)?;
        Ok(())
    }
}
