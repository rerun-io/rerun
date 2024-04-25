use re_types_core::ComponentName;

use crate::{EntityPath, Instance};

/// A general path to some data.
///
/// This always starts with an [`EntityPath`], followed by an optional instance index,
/// followed by an optional [`ComponentName`].
///
/// For instance:
///
/// * `points`
/// * `points:Color`
/// * `points[#42]`
/// * `points[#42]:Color`
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct DataPath {
    pub entity_path: EntityPath,
    pub instance: Option<Instance>,
    pub component_name: Option<ComponentName>,
}

impl std::fmt::Display for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.entity_path.fmt(f)?;
        if let Some(instance) = &self.instance {
            write!(f, "[#{instance}]")?;
        }
        if let Some(component_name) = &self.component_name {
            write!(f, ":{component_name:?}")?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_string().fmt(f)
    }
}
