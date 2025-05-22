use re_types_core::ComponentDescriptor;

use crate::{EntityPath, Instance};

/// A general path to some data.
///
/// This always starts with an [`EntityPath`], followed by an optional instance index,
/// followed by an optional [`ComponentDescriptor`].
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
    pub component_descriptor: Option<ComponentDescriptor>,
}

impl std::fmt::Display for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.entity_path.fmt(f)?;
        if let Some(instance) = &self.instance {
            write!(f, "[#{instance}]")?;
        }
        if let Some(component_descriptor) = &self.component_descriptor {
            write!(f, ":{component_descriptor:?}")?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_string().fmt(f)
    }
}
