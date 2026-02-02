use re_types_core::ComponentIdentifier;

use crate::{EntityPath, Instance};

/// A general path to some data.
///
/// This always starts with an [`EntityPath`], followed by an optional instance index,
/// followed by an optional [`ComponentIdentifier`].
///
/// For instance:
///
/// * `points`
/// * `points:Color`
/// * `points[#42]`
/// * `points[#42]:Color`
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct DataPath {
    pub entity_path: EntityPath,
    pub instance: Option<Instance>,
    pub component: Option<ComponentIdentifier>,
}

impl std::fmt::Display for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.entity_path.fmt(f)?;
        if let Some(instance) = &self.instance
            && instance != &Instance::ALL
        {
            write!(f, "[#{instance}]")?;
        }
        if let Some(component) = &self.component {
            f.write_str(":")?;
            component.fmt(f)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for DataPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_string().fmt(f)
    }
}
