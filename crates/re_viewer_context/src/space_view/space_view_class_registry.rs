use ahash::HashMap;

use crate::{SpaceViewClass, SpaceViewClassName};

#[derive(Debug, thiserror::Error)]
pub enum SpaceViewTypeRegistryError {
    #[error("Space view with typename \"{0}\" was already registered.")]
    DuplicateTypeName(SpaceViewClassName),

    #[error("Space view with typename \"{0}\" was not found.")]
    TypeNotFound(SpaceViewClassName),
}

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewClassRegistry(HashMap<SpaceViewClassName, Box<dyn SpaceViewClass>>);

impl SpaceViewClassRegistry {
    /// Adds a new space view type.
    ///
    /// Fails if a space view type with the same name was already registered.
    pub fn add(
        &mut self,
        space_view_type: impl SpaceViewClass + 'static,
    ) -> Result<(), SpaceViewTypeRegistryError> {
        let type_name = space_view_type.type_name();
        if self
            .0
            .insert(type_name, Box::new(space_view_type))
            .is_some()
        {
            return Err(SpaceViewTypeRegistryError::DuplicateTypeName(type_name));
        }

        Ok(())
    }

    /// Queries a space view type by name.
    pub fn query(
        &self,
        name: SpaceViewClassName,
    ) -> Result<&dyn SpaceViewClass, SpaceViewTypeRegistryError> {
        self.0
            .get(&name)
            .map(|boxed| boxed.as_ref())
            .ok_or(SpaceViewTypeRegistryError::TypeNotFound(name))
    }
}
