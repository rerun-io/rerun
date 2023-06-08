use ahash::HashMap;

use crate::{DynSpaceViewClass, SpaceViewClassName};

#[derive(Debug, thiserror::Error)]
pub enum SpaceViewClassRegistryError {
    #[error("Space view with class name {0:?} was already registered.")]
    DuplicateTypeName(SpaceViewClassName),
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
            return Err(SpaceViewClassRegistryError::DuplicateClassName(type_name));
        }

        Ok(())
    }

    /// Queries a Space View type by class name.
    fn get(&self, name: SpaceViewClassName) -> Option<&dyn DynSpaceViewClass> {
        self.0.get(&name).map(|boxed| boxed.as_ref())
    }

    /// Queries a Space View type by class name and logs if it fails.
    pub fn get_or_log_error(&self, name: SpaceViewClassName) -> Option<&dyn DynSpaceViewClass> {
        let result = self.get(name);
        // TODO(wumpf): Workaround for tensor not yet ported
        if result.is_none() && name != "Tensor" {
            re_log::error_once!("Unknown space view class {:?}", name);
        }
        result
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn DynSpaceViewClass> {
        self.0.values().map(|boxed| boxed.as_ref())
    }
}
