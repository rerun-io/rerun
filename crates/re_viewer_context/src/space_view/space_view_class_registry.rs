use ahash::HashMap;

use crate::{DynSpaceViewClass, SpaceViewClassName, ViewContextSystem};

use super::space_view_class_placeholder::SpaceViewClassPlaceholder;

#[derive(Debug, thiserror::Error)]
pub enum SpaceViewClassRegistryError {
    #[error("Space view with class name {0:?} was already registered.")]
    DuplicateClassName(SpaceViewClassName),
}

/// Entry in [`SpaceViewClassRegistry`]
pub struct SpaceViewClassRegistryEntry {
    class: Box<dyn DynSpaceViewClass>,
    context_creators: Vec<Box<dyn Fn() -> Box<dyn ViewContextSystem>>>,
}

impl SpaceViewClassRegistryEntry {
    /// Registers a new [`ViewContextSystem`] type for this space view class that will be created and executed every frame.
    ///
    /// It is not allowed to register a given type more than once.
    pub fn register_context<T: ViewContextSystem + Default + 'static>(&mut self) {
        self.context_creators.push(Box::new(|| Box::<T>::default()));
    }
}

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewClassRegistry {
    registry: HashMap<SpaceViewClassName, SpaceViewClassRegistryEntry>,
    placeholder: SpaceViewClassPlaceholder,
}

impl SpaceViewClassRegistry {
    /// Adds a new space view type.
    ///
    /// Fails if a space view type with the same name was already registered.
    pub fn add<T: DynSpaceViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        let mut entry = SpaceViewClassRegistryEntry {
            class: Box::<T>::default(),
            context_creators: Vec::new(),
        };

        // We can't call on_register on entry.class since we require a mutable reference to entry.
        // Working around this by creating a new, identical instance of the class.
        T::default().on_register(&mut entry);

        let type_name = entry.class.name();
        if self.registry.insert(type_name, entry).is_some() {
            return Err(SpaceViewClassRegistryError::DuplicateClassName(type_name));
        }

        Ok(())
    }

    /// Queries a Space View type by class name, returning `None` if it is not registered.
    fn get(&self, name: &SpaceViewClassName) -> Option<&dyn DynSpaceViewClass> {
        self.registry.get(name).map(|boxed| boxed.class.as_ref())
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
        self.registry.values().map(|entry| entry.class.as_ref())
    }
}
