use crate::{path::EntityPath, ComponentName};

/// A [`EntityPath`] plus a [`ComponentName`].
///
/// Example: `camera / "left" / points / #42`.`color`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DataPath {
    /// `camera / "left" / points / #42`
    pub obj_path: EntityPath,

    /// "color"
    pub component_name: ComponentName,
}

impl DataPath {
    #[inline]
    pub fn new(obj_path: EntityPath, component_name: ComponentName) -> Self {
        Self {
            obj_path,
            component_name,
        }
    }

    #[inline]
    pub fn obj_path(&self) -> &EntityPath {
        &self.obj_path
    }

    #[inline]
    pub fn component_name(&self) -> &ComponentName {
        &self.component_name
    }
}

impl std::fmt::Display for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write as _;
        self.obj_path.fmt(f)?;
        f.write_char('.')?;
        self.component_name.fmt(f)
    }
}
