use ahash::{HashMap, HashSet};
use nohash_hasher::{IntMap, IntSet};
use re_arrow_store::DataStore;
use re_log_types::EntityPathHash;

use crate::{
    ApplicableEntitiesPerVisualizer, DynSpaceViewClass, IdentifiedViewSystem,
    SpaceViewClassIdentifier, ViewContextCollection, ViewContextSystem, ViewPartCollection,
    ViewPartSystem, ViewSystemIdentifier,
};

use super::{
    space_view_class_placeholder::SpaceViewClassPlaceholder,
    visualizer_entity_subscriber::VisualizerEntitySubscriber, VisualizerApplicableEntities,
};

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

/// Utility for registering space view systems, passed on to [`crate::SpaceViewClass::on_register`].
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
                .or_insert_with(|| ContextSystemTypeRegistryEntry {
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
                .or_insert_with(|| {
                    let entity_subscriber_handle = DataStore::register_subscriber(Box::new(
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
            Err(
                SpaceViewClassRegistryError::IdentifierAlreadyInUseForVisualizer(
                    T::identifier().as_str(),
                ),
            )
        }
    }
}

/// Space view class entry in [`SpaceViewClassRegistry`].
pub struct SpaceViewClassRegistryEntry {
    pub class: Box<dyn DynSpaceViewClass>,
    pub context_system_ids: HashSet<ViewSystemIdentifier>,
    pub visualizer_system_ids: HashSet<ViewSystemIdentifier>,
}

#[allow(clippy::derivable_impls)] // Clippy gets this one wrong.
impl Default for SpaceViewClassRegistryEntry {
    fn default() -> Self {
        Self {
            class: Box::<SpaceViewClassPlaceholder>::default(),
            context_system_ids: Default::default(),
            visualizer_system_ids: Default::default(),
        }
    }
}

/// Context system type entry in [`SpaceViewClassRegistry`].
struct ContextSystemTypeRegistryEntry {
    factory_method: Box<dyn Fn() -> Box<dyn ViewContextSystem> + Send + Sync>,
    used_by: HashSet<SpaceViewClassIdentifier>,
}

/// Visualizer entry in [`SpaceViewClassRegistry`].
struct VisualizerTypeRegistryEntry {
    factory_method: Box<dyn Fn() -> Box<dyn ViewPartSystem> + Send + Sync>,
    used_by: HashSet<SpaceViewClassIdentifier>,

    /// Handle to subscription of [`VisualizerEntitySubscriber`] for this visualizer.
    entity_subscriber_handle: re_arrow_store::StoreSubscriberHandle,
}

impl Drop for VisualizerTypeRegistryEntry {
    fn drop(&mut self) {
        // TODO(andreas): DataStore unsubscribe is not yet implemented!
        //DataStore::unregister_subscriber(self.entity_subscriber_handle);
    }
}

/// Registry of all known space view types.
///
/// Expected to be populated on viewer startup.
#[derive(Default)]
pub struct SpaceViewClassRegistry {
    space_view_classes: HashMap<SpaceViewClassIdentifier, SpaceViewClassRegistryEntry>,
    context_systems: HashMap<ViewSystemIdentifier, ContextSystemTypeRegistryEntry>,
    visualizers: HashMap<ViewSystemIdentifier, VisualizerTypeRegistryEntry>,
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
                    context_system_ids: context_systems,
                    visualizer_system_ids: visualizers,
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
    pub fn iter_registry(&self) -> impl Iterator<Item = &SpaceViewClassRegistryEntry> {
        self.space_view_classes.values()
    }

    /// For each visualizer, return the set of entities that is applicable to it.
    ///
    /// The list is kept up to date by store subscribers.
    pub fn applicable_entities_for_visualizer_systems(
        &self,
        store_id: &re_log_types::StoreId,
    ) -> ApplicableEntitiesPerVisualizer {
        re_tracing::profile_function!();

        ApplicableEntitiesPerVisualizer(
            self.visualizers
                .iter()
                .map(|(id, entry)| {
                    let mut entities = VisualizerApplicableEntities::default();
                    DataStore::with_subscriber::<VisualizerEntitySubscriber, _, _>(
                        entry.entity_subscriber_handle,
                        |subscriber| {
                            if let Some(applicable_entities) =
                                subscriber.applicable_entities(store_id)
                            {
                                entities = applicable_entities.clone();
                            }
                        },
                    );
                    (*id, entities)
                })
                .collect(),
        )
    }

    /// For each visualizer, the set of entities that have at least one matching indicator component.
    pub fn entities_with_matching_indicator_per_visualizer(
        &self,
        store_id: &re_log_types::StoreId,
    ) -> IntMap<ViewSystemIdentifier, IntSet<EntityPathHash>> {
        self.visualizers
            .iter()
            .map(|(id, entry)| {
                (
                    *id,
                    DataStore::with_subscriber::<VisualizerEntitySubscriber, _, _>(
                        entry.entity_subscriber_handle,
                        |subscriber| {
                            subscriber
                                .entities_with_matching_indicator(store_id)
                                .cloned()
                        },
                    )
                    .flatten()
                    .unwrap_or_default(),
                )
            })
            .collect()
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
                .context_system_ids
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
                .visualizer_system_ids
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
