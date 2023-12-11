use ahash::HashMap;

use crate::{
    DynSpaceViewClass, IdentifiedViewSystem, SpaceViewClassIdentifier, ViewContextCollection,
    ViewContextSystem, ViewPartCollection, ViewPartSystem, ViewSystemIdentifier,
};

use super::space_view_class_placeholder::SpaceViewClassPlaceholder;

#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum SpaceViewClassRegistryError {
    #[error("Space View with class identifier {0:?} was already registered.")]
    DuplicateClassIdentifier(SpaceViewClassIdentifier),

    #[error("A Context System with identifier {0:?} was already registered.")]
    IdentifierAlreadyInUseForContextSystem(&'static str),

    #[error("A View Part System with identifier {0:?} was already registered.")]
    IdentifierAlreadyInUseForViewSystem(&'static str),

    #[error("Space View with class identifier {0:?} was not registered.")]
    UnknownClassIdentifier(SpaceViewClassIdentifier),
}

/// System registry for a space view class.
///
/// All context & part systems that are registered here will be created and executed every frame
/// for every instance of the space view class this belongs to.
#[derive(Default)]
pub struct SpaceViewSystemRegistry {
    contexts:
        HashMap<ViewSystemIdentifier, Box<dyn Fn() -> Box<dyn ViewContextSystem> + Send + Sync>>,
    parts: HashMap<ViewSystemIdentifier, Box<dyn Fn() -> Box<dyn ViewPartSystem> + Send + Sync>>,
}

impl SpaceViewSystemRegistry {
    /// Registers a new [`ViewContextSystem`] type for this space view class that will be created and executed every frame.
    ///
    /// It is not allowed to register a given type more than once.
    pub fn register_context_system<
        T: ViewContextSystem + IdentifiedViewSystem + Default + 'static,
    >(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        // Name should also not overlap with part systems.
        if self.parts.contains_key(&T::identifier()) {
            return Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForViewSystem(
                    T::identifier().as_str(),
                ),
            );
        }

        if let std::collections::hash_map::Entry::Vacant(e) = self.contexts.entry(T::identifier()) {
            e.insert(Box::new(|| Box::<T>::default()));
            Ok(())
        } else {
            Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForContextSystem(
                    T::identifier().as_str(),
                ),
            )
        }
    }

    /// Registers a new [`ViewPartSystem`] type for this space view class that will be created and executed every frame.
    ///
    /// It is not allowed to register a given type more than once.
    pub fn register_part_system<T: ViewPartSystem + IdentifiedViewSystem + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        // Name should also not overlap with context systems.
        if self.parts.contains_key(&T::identifier()) {
            return Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForContextSystem(
                    T::identifier().as_str(),
                ),
            );
        }

        if let std::collections::hash_map::Entry::Vacant(e) = self.parts.entry(T::identifier()) {
            e.insert(Box::new(|| Box::<T>::default()));
            Ok(())
        } else {
            Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForViewSystem(
                    T::identifier().as_str(),
                ),
            )
        }
    }

    pub fn new_context_collection(
        &self,
        space_view_class_identifier: SpaceViewClassIdentifier,
    ) -> ViewContextCollection {
        re_tracing::profile_function!();

        ViewContextCollection {
            systems: self
                .contexts
                .iter()
                .map(|(name, factory)| {
                    let part = factory();
                    (*name, part)
                })
                .collect(),
            space_view_class_identifier,
        }
    }

    pub fn new_part_collection(&self) -> ViewPartCollection {
        re_tracing::profile_function!();

        ViewPartCollection {
            systems: self
                .parts
                .iter()
                .map(|(name, factory)| {
                    let part = factory();
                    (*name, part)
                })
                .collect(),
        }
    }
}

/// Entry in [`SpaceViewClassRegistry`]
struct SpaceViewClassRegistryEntry {
    class: Box<dyn DynSpaceViewClass>,
    systems: SpaceViewSystemRegistry,
}

#[allow(clippy::derivable_impls)] // Clippy gets this one wrong.
impl Default for SpaceViewClassRegistryEntry {
    fn default() -> Self {
        Self {
            class: Box::<SpaceViewClassPlaceholder>::default(),
            systems: SpaceViewSystemRegistry::default(),
        }
    }
}

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewClassRegistry {
    registry: HashMap<SpaceViewClassIdentifier, SpaceViewClassRegistryEntry>,
    placeholder: SpaceViewClassRegistryEntry,
}

impl SpaceViewClassRegistry {
    /// Adds a new space view class.
    ///
    /// Fails if a space view class with the same name was already registered.
    pub fn add_class<T: DynSpaceViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        let mut entry = SpaceViewClassRegistryEntry {
            class: Box::<T>::default(),
            systems: SpaceViewSystemRegistry::default(),
        };

        entry.class.on_register(&mut entry.systems)?;

        let type_name = entry.class.identifier();
        if self.registry.insert(type_name, entry).is_some() {
            return Err(SpaceViewClassRegistryError::DuplicateClassIdentifier(
                type_name,
            ));
        }

        Ok(())
    }

    /// Removes a space view class from the registry.
    pub fn remove_class(
        &mut self,
        type_name: &SpaceViewClassIdentifier,
    ) -> Result<(), SpaceViewClassRegistryError> {
        if self.registry.remove(type_name).is_none() {
            return Err(SpaceViewClassRegistryError::UnknownClassIdentifier(
                *type_name,
            ));
        }

        Ok(())
    }

    /// Queries a Space View type by class name, returning `None` if it is not registered.
    fn get_class(&self, name: &SpaceViewClassIdentifier) -> Option<&dyn DynSpaceViewClass> {
        self.registry.get(name).map(|boxed| boxed.class.as_ref())
    }

    /// Returns the user-facing name for the given space view class.
    ///
    /// If the class is unknown, returns a placeholder name.
    pub fn display_name(&self, name: &SpaceViewClassIdentifier) -> &'static str {
        self.registry
            .get(name)
            .map_or("<unknown space view class>", |boxed| {
                boxed.class.display_name()
            })
    }

    /// Queries a Space View type's system registry by class name, returning `None` if the class is not registered.
    fn get_system_registry(
        &self,
        name: &SpaceViewClassIdentifier,
    ) -> Option<&SpaceViewSystemRegistry> {
        self.registry.get(name).map(|boxed| &boxed.systems)
    }

    /// Queries a Space View type by class name and logs if it fails, returning a placeholder class.
    pub fn get_class_or_log_error(
        &self,
        name: &SpaceViewClassIdentifier,
    ) -> &dyn DynSpaceViewClass {
        if let Some(result) = self.get_class(name) {
            result
        } else {
            re_log::error_once!("Unknown space view class {:?}", name);
            self.placeholder.class.as_ref()
        }
    }

    /// Queries a Space View's system registry by class name and logs if it fails, returning a placeholder class.
    pub fn get_system_registry_or_log_error(
        &self,
        name: &SpaceViewClassIdentifier,
    ) -> &SpaceViewSystemRegistry {
        if let Some(result) = self.get_system_registry(name) {
            result
        } else {
            re_log::error_once!("Unknown space view class {:?}", name);
            &self.placeholder.systems
        }
    }

    /// Iterates over all registered Space View class names and their system registries.
    pub fn iter_system_registries(
        &self,
    ) -> impl Iterator<Item = (&SpaceViewClassIdentifier, &SpaceViewSystemRegistry)> {
        self.registry
            .iter()
            .map(|(name, entry)| (name, &entry.systems))
    }

    /// Iterates over all registered Space View class types.
    pub fn iter_classes(&self) -> impl Iterator<Item = &dyn DynSpaceViewClass> {
        self.registry.values().map(|entry| entry.class.as_ref())
    }
}
