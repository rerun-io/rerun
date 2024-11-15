use re_types_core::ComponentName;

use crate::path::EntityPath;

/// A [`EntityPath`] plus a [`ComponentName`].
///
/// Example: `camera/left/points:Color`
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ComponentPath {
    /// `camera / "left" / points / #42`
    pub entity_path: EntityPath,

    /// "color"
    pub component_name: ComponentName,
}

impl ComponentPath {
    #[inline]
    pub fn new(entity_path: EntityPath, component_name: ComponentName) -> Self {
        Self {
            entity_path,
            component_name,
        }
    }

    #[inline]
    pub fn entity_path(&self) -> &EntityPath {
        &self.entity_path
    }

    #[inline]
    pub fn component_name(&self) -> &ComponentName {
        &self.component_name
    }
}

impl std::fmt::Display for ComponentPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.entity_path.fmt(f)?;
        f.write_str(":")?;
        self.component_name.fmt(f)?;
        Ok(())
    }
}
