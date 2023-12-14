use ahash::{HashMap, HashSet};

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
    IdentifierAlreadyInUseForVisualizer(&'static str),

    #[error("Space View with class identifier {0:?} was not registered.")]
    UnknownClassIdentifier(SpaceViewClassIdentifier),
}

/// Utility for registering space view systems, passed on to [`SpaceViewClass::on_register`].
pub struct SpaceViewSystemRegistrator<'a> {
    registry: &'a mut SpaceViewClassRegistry,
    identifier: SpaceViewClassIdentifier,
    context_systems: HashSet<ViewSystemIdentifier>,
    visualizers: HashSet<ViewSystemIdentifier>,
}

impl SpaceViewSystemRegistrator<'_> {
    /// Registers a new [`ViewContextSystem`] type for a space view class that will be created and executed every frame.
    ///
    /// It is not allowed to register a given type more than once within the same space view class.
    /// Different space view classes may however share the same [`ViewContextSystem`] type.
    pub fn register_context_system<
        T: ViewContextSystem + IdentifiedViewSystem + Default + 'static,
    >(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        // Name should not overlap with context systems.
        if self.registry.visualizers.contains_key(&T::identifier()) {
            return Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForVisualizer(
                    T::identifier().as_str(),
                ),
            );
        }

        if self.context_systems.insert(T::identifier()) {
            self.registry
                .context_systems
                .entry(T::identifier())
                .or_insert_with(|| SystemTypeRegistryEntry {
                    factory_method: Box::new(|| Box::<T>::default()),
                    used_by: Default::default(),
                })
                .used_by
                .insert(self.identifier);

            Ok(())
        } else {
            Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForContextSystem(
                    T::identifier().as_str(),
                ),
            )
        }
    }

    /// Registers a new [`ViewPartSystem`] type for a space view class that will be created and executed every frame.
    ///
    /// It is not allowed to register a given type more than once within the same space view class.
    /// Different space view classes may however share the same [`ViewPartSystem`] type.
    pub fn register_visualizer<T: ViewPartSystem + IdentifiedViewSystem + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        // Name should not overlap with context systems.
        if self.registry.context_systems.contains_key(&T::identifier()) {
            return Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForContextSystem(
                    T::identifier().as_str(),
                ),
            );
        }

        if self.visualizers.insert(T::identifier()) {
            self.registry
                .visualizers
                .entry(T::identifier())
                .or_insert_with(|| SystemTypeRegistryEntry {
                    factory_method: Box::new(|| Box::<T>::default()),
                    used_by: Default::default(),
                })
                .used_by
                .insert(self.identifier);

            Ok(())
        } else {
            Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForVisualizer(
                    T::identifier().as_str(),
                ),
            )
        }
    }
}

/// Space view class entry in [`SpaceViewClassRegistry`].
struct SpaceViewClassRegistryEntry {
    class: Box<dyn DynSpaceViewClass>,
    context_systems: HashSet<ViewSystemIdentifier>,
    visualizers: HashSet<ViewSystemIdentifier>,
}

#[allow(clippy::derivable_impls)] // Clippy gets this one wrong.
impl Default for SpaceViewClassRegistryEntry {
    fn default() -> Self {
        Self {
            class: Box::<SpaceViewClassPlaceholder>::default(),
            context_systems: Default::default(),
            visualizers: Default::default(),
        }
    }
}

/// System type entry in [`SpaceViewClassRegistry`].
struct SystemTypeRegistryEntry<T: ?Sized> {
    factory_method: Box<dyn Fn() -> Box<T> + Send + Sync>,
    used_by: HashSet<SpaceViewClassIdentifier>,
}

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewClassRegistry {
    space_view_classes: HashMap<SpaceViewClassIdentifier, SpaceViewClassRegistryEntry>,
    visualizers: HashMap<ViewSystemIdentifier, SystemTypeRegistryEntry<dyn ViewPartSystem>>,
    context_systems: HashMap<ViewSystemIdentifier, SystemTypeRegistryEntry<dyn ViewContextSystem>>,
    placeholder: SpaceViewClassRegistryEntry,
}

impl SpaceViewClassRegistry {
    /// Adds a new space view class.
    ///
    /// Fails if a space view class with the same name was already registered.
    pub fn add_class<T: DynSpaceViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        let class = Box::<T>::default();

        let mut registrator = SpaceViewSystemRegistrator {
            registry: self,
            identifier: class.identifier(),
            context_systems: Default::default(),
            visualizers: Default::default(),
        };

        class.on_register(&mut registrator)?;

        let SpaceViewSystemRegistrator {
            registry: _,
            identifier,
            context_systems,
            visualizers,
        } = registrator;

        if self
            .space_view_classes
            .insert(
                identifier,
                SpaceViewClassRegistryEntry {
                    class,
                    context_systems,
                    visualizers,
                },
            )
            .is_some()
        {
            return Err(SpaceViewClassRegistryError::DuplicateClassIdentifier(
                identifier,
            ));
        }

        Ok(())
    }

    /// Removes a space view class from the registry.
    pub fn remove_class<T: DynSpaceViewClass + Sized>(
        &mut self,
    ) -> Result<(), SpaceViewClassRegistryError> {
        let identifier: SpaceViewClassIdentifier = T::identifier_str().into();
        if self.space_view_classes.remove(&identifier).is_none() {
            return Err(SpaceViewClassRegistryError::UnknownClassIdentifier(
                identifier,
            ));
        }

        self.context_systems.retain(|_, context_system_entry| {
            context_system_entry.used_by.remove(&identifier);
            !context_system_entry.used_by.is_empty()
        });

        self.visualizers.retain(|_, visualizer_entry| {
            visualizer_entry.used_by.remove(&identifier);
            !visualizer_entry.used_by.is_empty()
        });

        Ok(())
    }

    /// Queries a Space View type by class name, returning `None` if it is not registered.
    fn get_class(&self, name: &SpaceViewClassIdentifier) -> Option<&dyn DynSpaceViewClass> {
        self.space_view_classes
            .get(name)
            .map(|boxed| boxed.class.as_ref())
    }

    /// Returns the user-facing name for the given space view class.
    ///
    /// If the class is unknown, returns a placeholder name.
    pub fn display_name(&self, name: &SpaceViewClassIdentifier) -> &'static str {
        self.space_view_classes
            .get(name)
            .map_or("<unknown space view class>", |boxed| {
                boxed.class.display_name()
            })
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

    /// Iterates over all registered Space View class types.
    pub fn iter_classes(&self) -> impl Iterator<Item = &dyn DynSpaceViewClass> {
        self.space_view_classes
            .values()
            .map(|entry| entry.class.as_ref())
    }

    pub fn new_context_collection(
        &self,
        space_view_class_identifier: SpaceViewClassIdentifier,
    ) -> ViewContextCollection {
        re_tracing::profile_function!();

        let Some(class) = self.space_view_classes.get(&space_view_class_identifier) else {
            return ViewContextCollection {
                systems: Default::default(),
                space_view_class_identifier,
            };
        };

        ViewContextCollection {
            systems: class
                .context_systems
                .iter()
                .filter_map(|name| {
                    self.context_systems.get(name).map(|entry| {
                        let part = (entry.factory_method)();
                        (*name, part)
                    })
                })
                .collect(),
            space_view_class_identifier,
        }
    }

    pub fn new_part_collection(
        &self,
        space_view_class_identifier: SpaceViewClassIdentifier,
    ) -> ViewPartCollection {
        re_tracing::profile_function!();

        let Some(class) = self.space_view_classes.get(&space_view_class_identifier) else {
            return ViewPartCollection {
                systems: Default::default(),
            };
        };

        ViewPartCollection {
            systems: class
                .visualizers
                .iter()
                .filter_map(|name| {
                    self.visualizers.get(name).map(|entry| {
                        let part = (entry.factory_method)();
                        (*name, part)
                    })
                })
                .collect(),
        }
    }
}
