use ahash::HashMap;

use crate::{DynSpaceViewClass, SpaceViewClassName};

#[derive(Debug, thiserror::Error)]
pub enum SpaceViewClassRegistryError {
    #[error("Space view with typename \"{0}\" was already registered.")]
    DuplicateTypeName(SpaceViewClassName),

    #[error("Space view with typename \"{0}\" was not found.")]
    TypeNotFound(SpaceViewClassName),
}

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewClassRegistry(HashMap<SpaceViewClassName, Box<dyn DynSpaceViewClass>>);

impl SpaceViewClassRegistry {
    /// Adds a new space view type.
    ///
    /// Fails if a space view type with the same name was already registered.
    pub fn add<T: DynSpaceViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        let space_view_type = T::default();
        let type_name = space_view_type.name();
        if self
            .0
            .insert(type_name, Box::new(space_view_type))
            .is_some()
        {
            return Err(SpaceViewClassRegistryError::DuplicateTypeName(type_name));
        }

        Ok(())
    }

    /// Queries a space view type by name.
    pub fn get(
        &self,
        name: SpaceViewClassName,
    ) -> Result<&dyn DynSpaceViewClass, SpaceViewClassRegistryError> {
        self.0
            .get(&name)
            .map(|boxed| boxed.as_ref())
            .ok_or(SpaceViewClassRegistryError::TypeNotFound(name))
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn DynSpaceViewClass> {
        self.0.values().map(|boxed| boxed.as_ref())
    }
}
