use ahash::HashMap;

use crate::{DynSpaceViewClass, SpaceViewClassName};

use super::space_view_class_placeholder::SpaceViewClassPlaceholder;

#[derive(Debug, thiserror::Error)]
pub enum SpaceViewClassRegistryError {
    #[error("Space view with class name {0:?} was already registered.")]
    DuplicateClassName(SpaceViewClassName),
}

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewClassRegistry {
    registry: HashMap<SpaceViewClassName, Box<dyn DynSpaceViewClass>>,
    placeholder: SpaceViewClassPlaceholder,
}

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
            .registry
            .insert(type_name, Box::new(space_view_type))
            .is_some()
        {
            return Err(SpaceViewClassRegistryError::DuplicateClassName(type_name));
        }

        Ok(())
    }

    /// Queries a Space View type by class name, returning `None` if it is not registered.
    pub fn get(&self, name: &SpaceViewClassName) -> Option<&dyn DynSpaceViewClass> {
        self.registry.get(name).map(|boxed| boxed.as_ref())
    }

    /// Queries a Space View type by class name and logs if it fails, returning a placeholder class.
    pub fn get_or_log_error(&self, name: &SpaceViewClassName) -> &dyn DynSpaceViewClass {
        if let Some(result) = self.get(name) {
            result
        } else {
            re_log::error_once!("Unknown space view class {:?}", name);
            &self.placeholder
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn DynSpaceViewClass> {
        self.registry.values().map(|boxed| boxed.as_ref())
    }
}
