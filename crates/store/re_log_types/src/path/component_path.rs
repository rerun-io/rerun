use re_types_core::ComponentDescriptor;

use crate::path::EntityPath;

/// A [`EntityPath`] plus a [`ComponentDescriptor`].
///
/// Example: `camera/left/points:Points3D:color`
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ComponentPath {
    /// `camera / "left" / points / #42`
    pub entity_path: EntityPath,

    /// e.g. `Points3D:color`
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
