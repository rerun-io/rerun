use ahash::{HashMap, HashSet};
use itertools::Itertools as _;

use re_chunk_store::{ChunkStore, ChunkStoreSubscriberHandle};
use re_types::ViewClassIdentifier;

use crate::{
    IdentifiedViewSystem, IndicatedEntities, MaybeVisualizableEntities, PerVisualizer, ViewClass,
    ViewContextCollection, ViewContextSystem, ViewSystemIdentifier, VisualizerCollection,
    VisualizerSystem,
};

use super::{
    view_class_placeholder::ViewClassPlaceholder,
    visualizer_entity_subscriber::VisualizerEntitySubscriber,
};

#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum ViewClassRegistryError {
    #[error("View with class identifier {0:?} was already registered.")]
    DuplicateClassIdentifier(ViewClassIdentifier),

    #[error("A context system with identifier {0:?} was already registered.")]
    IdentifierAlreadyInUseForContextSystem(&'static str),

    #[error("A visualizer system with identifier {0:?} was already registered.")]
    IdentifierAlreadyInUseForVisualizer(&'static str),

    #[error("View with class identifier {0:?} was not registered.")]
    UnknownClassIdentifier(ViewClassIdentifier),
}

/// Utility for registering view systems, passed on to [`crate::ViewClass::on_register`].
pub struct ViewSystemRegistrator<'a> {
    registry: &'a mut ViewClassRegistry,
    identifier: ViewClassIdentifier,
    context_systems: HashSet<ViewSystemIdentifier>,
    visualizers: HashSet<ViewSystemIdentifier>,
}

impl ViewSystemRegistrator<'_> {
    /// Registers a new [`ViewContextSystem`] type for a view class that will be created and executed every frame.
    ///
    /// It is not allowed to register a given type more than once within the same view class.
    /// Different view classes may however share the same [`ViewContextSystem`] type.
    pub fn register_context_system<
        T: ViewContextSystem + IdentifiedViewSystem + Default + 'static,
    >(
        &mut self,
    ) -> Result<(), ViewClassRegistryError> {
        // Name should not overlap with context systems.
        if self.registry.visualizers.contains_key(&T::identifier()) {
            return Err(ViewClassRegistryError::IdentifierAlreadyInUseForVisualizer(
                T::identifier().as_str(),
            ));
        }

        if self.context_systems.insert(T::identifier()) {
            self.registry
                .context_systems
                .entry(T::identifier())
                .or_insert_with(|| ContextSystemTypeRegistryEntry {
                    factory_method: Box::new(|| Box::<T>::default()),
                    used_by: Default::default(),
                })
                .used_by
                .insert(self.identifier);

            Ok(())
        } else {
            Err(
                ViewClassRegistryError::IdentifierAlreadyInUseForContextSystem(
                    T::identifier().as_str(),
                ),
            )
        }
    }

    /// Registers a new [`VisualizerSystem`] type for a view class that will be created and executed every frame.
    ///
    /// It is not allowed to register a given type more than once within the same view class.
    /// Different view classes may however share the same [`VisualizerSystem`] type.
    pub fn register_visualizer<T: VisualizerSystem + IdentifiedViewSystem + Default + 'static>(
        &mut self,
    ) -> Result<(), ViewClassRegistryError> {
        // Name should not overlap with context systems.
        if self.registry.context_systems.contains_key(&T::identifier()) {
            return Err(
                ViewClassRegistryError::IdentifierAlreadyInUseForContextSystem(
                    T::identifier().as_str(),
                ),
            );
        }

        if self.visualizers.insert(T::identifier()) {
            self.registry
                .visualizers
                .entry(T::identifier())
                .or_insert_with(|| {
                    let entity_subscriber_handle = ChunkStore::register_subscriber(Box::new(
                        VisualizerEntitySubscriber::new(&T::default()),
                    ));

                    VisualizerTypeRegistryEntry {
                        factory_method: Box::new(|| Box::<T>::default()),
                        used_by: Default::default(),
                        entity_subscriber_handle,
                    }
                })
                .used_by
                .insert(self.identifier);

            Ok(())
        } else {
            Err(ViewClassRegistryError::IdentifierAlreadyInUseForVisualizer(
                T::identifier().as_str(),
            ))
        }
    }
}

/// View class entry in [`ViewClassRegistry`].
pub struct ViewClassRegistryEntry {
    pub class: Box<dyn ViewClass>,
    pub identifier: ViewClassIdentifier,
    pub context_system_ids: HashSet<ViewSystemIdentifier>,
    pub visualizer_system_ids: HashSet<ViewSystemIdentifier>,
}

#[allow(clippy::derivable_impls)] // Clippy gets this one wrong.
impl Default for ViewClassRegistryEntry {
    fn default() -> Self {
        Self {
            class: Box::<ViewClassPlaceholder>::default(),
            identifier: ViewClassPlaceholder::identifier(),
            context_system_ids: Default::default(),
            visualizer_system_ids: Default::default(),
        }
    }
}

/// Context system type entry in [`ViewClassRegistry`].
struct ContextSystemTypeRegistryEntry {
    factory_method: Box<dyn Fn() -> Box<dyn ViewContextSystem> + Send + Sync>,
    used_by: HashSet<ViewClassIdentifier>,
}

/// Visualizer entry in [`ViewClassRegistry`].
struct VisualizerTypeRegistryEntry {
    factory_method: Box<dyn Fn() -> Box<dyn VisualizerSystem> + Send + Sync>,
    used_by: HashSet<ViewClassIdentifier>,

    /// Handle to subscription of [`VisualizerEntitySubscriber`] for this visualizer.
    entity_subscriber_handle: ChunkStoreSubscriberHandle,
}

impl Drop for VisualizerTypeRegistryEntry {
    fn drop(&mut self) {
        // TODO(andreas): ChunkStore unsubscribe is not yet implemented!
        //ChunkStore::unregister_subscriber(self.entity_subscriber_handle);
    }
}

/// Registry of all known view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct ViewClassRegistry {
    view_classes: HashMap<ViewClassIdentifier, ViewClassRegistryEntry>,
    context_systems: HashMap<ViewSystemIdentifier, ContextSystemTypeRegistryEntry>,
    visualizers: HashMap<ViewSystemIdentifier, VisualizerTypeRegistryEntry>,
    placeholder: ViewClassRegistryEntry,
}

impl ViewClassRegistry {
    /// Adds a new view class.
    ///
    /// Fails if a view class with the same name was already registered.
    pub fn add_class<T: ViewClass + Default + 'static>(
        &mut self,
    ) -> Result<(), ViewClassRegistryError> {
        let class = Box::<T>::default();

        let mut registrator = ViewSystemRegistrator {
            registry: self,
            identifier: T::identifier(),
            context_systems: Default::default(),
            visualizers: Default::default(),
        };

        class.on_register(&mut registrator)?;

        let ViewSystemRegistrator {
            registry: _,
            identifier,
            context_systems,
            visualizers,
        } = registrator;

        if self
            .view_classes
            .insert(
                identifier,
                ViewClassRegistryEntry {
                    class,
                    identifier,
                    context_system_ids: context_systems,
                    visualizer_system_ids: visualizers,
                },
            )
            .is_some()
        {
            return Err(ViewClassRegistryError::DuplicateClassIdentifier(identifier));
        }

        Ok(())
    }

    /// Removes a view class from the registry.
    pub fn remove_class<T: ViewClass + Sized>(&mut self) -> Result<(), ViewClassRegistryError> {
        let identifier = T::identifier();
        if self.view_classes.remove(&identifier).is_none() {
            return Err(ViewClassRegistryError::UnknownClassIdentifier(identifier));
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

    /// Queries a View type by class name, returning `None` if it is not registered.
    pub fn class(&self, name: ViewClassIdentifier) -> Option<&dyn ViewClass> {
        self.view_classes
            .get(&name)
            .map(|boxed| boxed.class.as_ref())
    }

    /// Returns the user-facing name for the given view class.
    ///
    /// If the class is unknown, returns a placeholder name.
    pub fn display_name(&self, name: ViewClassIdentifier) -> &'static str {
        self.view_classes
            .get(&name)
            .map_or("<unknown view class>", |boxed| boxed.class.display_name())
    }

    /// Queries a View type by class name and logs if it fails, returning a placeholder class.
    pub fn get_class_or_log_error(&self, name: ViewClassIdentifier) -> &dyn ViewClass {
        if let Some(result) = self.class(name) {
            result
        } else {
            re_log::error_once!("Unknown view class {:?}", name);
            self.placeholder.class.as_ref()
        }
    }

    /// Iterates over all registered View class types, sorted by name.
    pub fn iter_registry(&self) -> impl Iterator<Item = &ViewClassRegistryEntry> {
        self.view_classes
            .values()
            .sorted_by_key(|entry| entry.class.display_name())
    }

    /// For each visualizer, return the set of entities that may be visualizable with it.
    ///
    /// The list is kept up to date by store subscribers.
    pub fn maybe_visualizable_entities_for_visualizer_systems(
        &self,
        store_id: &re_log_types::StoreId,
    ) -> PerVisualizer<MaybeVisualizableEntities> {
        re_tracing::profile_function!();

        PerVisualizer::<MaybeVisualizableEntities>(
            self.visualizers
                .iter()
                .map(|(id, entry)| {
                    (
                        *id,
                        ChunkStore::with_subscriber::<VisualizerEntitySubscriber, _, _>(
                            entry.entity_subscriber_handle,
                            |subscriber| subscriber.maybe_visualizable_entities(store_id).cloned(),
                        )
                        .flatten()
                        .unwrap_or_default(),
                    )
                })
                .collect(),
        )
    }

    /// For each visualizer, the set of entities that have at least one matching indicator component.
    pub fn indicated_entities_per_visualizer(
        &self,
        store_id: &re_log_types::StoreId,
    ) -> PerVisualizer<IndicatedEntities> {
        re_tracing::profile_function!();

        PerVisualizer::<IndicatedEntities>(
            self.visualizers
                .iter()
                .map(|(id, entry)| {
                    (
                        *id,
                        ChunkStore::with_subscriber::<VisualizerEntitySubscriber, _, _>(
                            entry.entity_subscriber_handle,
                            |subscriber| subscriber.indicated_entities(store_id).cloned(),
                        )
                        .flatten()
                        .unwrap_or_default(),
                    )
                })
                .collect(),
        )
    }

    pub fn new_context_collection(
        &self,
        view_class_identifier: ViewClassIdentifier,
    ) -> ViewContextCollection {
        re_tracing::profile_function!();

        let Some(class) = self.view_classes.get(&view_class_identifier) else {
            return ViewContextCollection {
                systems: Default::default(),
                view_class_identifier,
            };
        };

        ViewContextCollection {
            systems: class
                .context_system_ids
                .iter()
                .filter_map(|name| {
                    self.context_systems.get(name).map(|entry| {
                        let system = (entry.factory_method)();
                        (*name, system)
                    })
                })
                .collect(),
            view_class_identifier,
        }
    }

    pub fn new_visualizer_collection(
        &self,
        view_class_identifier: ViewClassIdentifier,
    ) -> VisualizerCollection {
        re_tracing::profile_function!();

        let Some(class) = self.view_classes.get(&view_class_identifier) else {
            return VisualizerCollection {
                systems: Default::default(),
            };
        };

        VisualizerCollection {
            systems: class
                .visualizer_system_ids
                .iter()
                .filter_map(|name| {
                    self.visualizers.get(name).map(|entry| {
                        let system = (entry.factory_method)();
                        (*name, system)
                    })
                })
                .collect(),
        }
    }

    pub fn instantiate_visualizer(
        &self,
        visualizer_identifier: ViewSystemIdentifier,
    ) -> Option<Box<dyn VisualizerSystem>> {
        self.visualizers
            .get(&visualizer_identifier)
            .map(|entry| (entry.factory_method)())
    }
}
