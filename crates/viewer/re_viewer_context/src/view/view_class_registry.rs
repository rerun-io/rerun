use std::sync::Arc;

use ahash::{HashMap, HashSet};
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};
use re_chunk::{ComponentIdentifier, ComponentType};
use re_chunk_store::{ChunkStore, ChunkStoreSubscriberHandle};
use re_sdk_types::ViewClassIdentifier;

use super::view_class_placeholder::ViewClassPlaceholder;
use super::visualizer_entity_subscriber::VisualizerEntitySubscriber;
use crate::view::view_context_system::ViewContextSystemOncePerFrameResult;
use crate::{
    IdentifiedViewSystem, IndicatedEntities, PerVisualizerType, QueryContext, ViewClass,
    ViewContextCollection, ViewContextSystem, ViewSystemIdentifier, ViewerContext,
    VisualizableEntities, VisualizerCollection, VisualizerSystem,
};
use crate::{
    component_fallbacks::FallbackProviderRegistry, view::view_context_system::ViewSystemState,
};

#[derive(Debug, thiserror::Error)]
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
    fallback_registry: &'a mut FallbackProviderRegistry,
    identifier: ViewClassIdentifier,
    context_systems: HashSet<ViewSystemIdentifier>,
    visualizers: HashSet<ViewSystemIdentifier>,
    pub app_options: &'a crate::AppOptions,
    known_builtin_enum_components: Arc<IntSet<ComponentType>>,
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
                    once_per_frame_execution_method: T::execute_once_per_frame,
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
            let app_options = self.app_options;
            let known_builtin_enum_components = Arc::clone(&self.known_builtin_enum_components);
            self.registry
                .visualizers
                .entry(T::identifier())
                .or_insert_with(move || {
                    let visualizer = T::default();
                    let entity_subscriber_handle =
                        ChunkStore::register_subscriber(Box::new(VisualizerEntitySubscriber::new(
                            &visualizer,
                            known_builtin_enum_components,
                            app_options,
                        )));

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

    /// Register a fallback provider specific to the current view
    /// and given component.
    pub fn register_fallback_provider<C: re_sdk_types::Component>(
        &mut self,
        component: ComponentIdentifier,
        provider: impl Fn(&QueryContext<'_>) -> C + Send + Sync + 'static,
    ) {
        self.fallback_registry.register_view_fallback_provider(
            self.identifier,
            component,
            provider,
        );
    }

    /// Register a fallback provider specific to the current view
    /// and given component.
    pub fn register_array_fallback_provider<
        C: re_sdk_types::Component,
        I: IntoIterator<Item = C>,
    >(
        &mut self,
        component: ComponentIdentifier,
        provider: impl Fn(&QueryContext<'_>) -> I + Send + Sync + 'static,
    ) {
        self.fallback_registry
            .register_view_array_fallback_provider(self.identifier, component, provider);
    }
}

/// View class entry in [`ViewClassRegistry`].
pub struct ViewClassRegistryEntry {
    pub class: Box<dyn ViewClass>,
    pub identifier: ViewClassIdentifier,
    pub context_system_ids: HashSet<ViewSystemIdentifier>,
    pub visualizer_system_ids: HashSet<ViewSystemIdentifier>,
}

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
    once_per_frame_execution_method: fn(&ViewerContext<'_>) -> ViewContextSystemOncePerFrameResult,
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
    ///
    /// Note that changes to app options later down the line may not be taken into account for already
    /// registered views & visualizers.
    pub fn add_class<T: ViewClass + Default + 'static>(
        &mut self,
        reflection: &re_types_core::reflection::Reflection,
        app_options: &crate::AppOptions,
        fallback_registry: &mut FallbackProviderRegistry,
    ) -> Result<(), ViewClassRegistryError> {
        let class = Box::<T>::default();

        let known_builtin_enum_components: Arc<IntSet<ComponentType>> = Arc::new(
            reflection
                .components
                .iter()
                .filter(|(_, r)| r.is_enum)
                .map(|(ct, _)| *ct)
                .collect(),
        );

        let mut registrator = ViewSystemRegistrator {
            registry: self,
            identifier: T::identifier(),
            context_systems: Default::default(),
            visualizers: Default::default(),
            fallback_registry,
            app_options,
            known_builtin_enum_components,
        };

        class.on_register(&mut registrator)?;

        let ViewSystemRegistrator {
            registry: _,
            identifier,
            context_systems,
            visualizers,
            fallback_registry: _,
            app_options: _,
            known_builtin_enum_components: _,
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

    /// Queries a View registry entry by class name, returning `None` if it is not registered.
    pub fn class_entry(&self, name: ViewClassIdentifier) -> Option<&ViewClassRegistryEntry> {
        self.view_classes.get(&name)
    }

    /// Queries a View registry entry type by class name and logs if it fails, returning a placeholder class.
    pub fn get_class_entry_or_log_error(
        &self,
        name: ViewClassIdentifier,
    ) -> &ViewClassRegistryEntry {
        if let Some(result) = self.class_entry(name) {
            result
        } else {
            re_log::error_once!("Unknown view class {:?}", name);
            &self.placeholder
        }
    }

    /// Queries a View type by class name, returning `None` if it is not registered.
    pub fn class(&self, name: ViewClassIdentifier) -> Option<&dyn ViewClass> {
        self.class_entry(name).map(|e| e.class.as_ref())
    }

    /// Queries a View type by class name and logs if it fails, returning a placeholder class.
    pub fn get_class_or_log_error(&self, name: ViewClassIdentifier) -> &dyn ViewClass {
        self.get_class_entry_or_log_error(name).class.as_ref()
    }

    /// Returns the user-facing name for the given view class.
    ///
    /// If the class is unknown, returns a placeholder name.
    pub fn display_name(&self, name: ViewClassIdentifier) -> &'static str {
        self.view_classes
            .get(&name)
            .map_or("<unknown view class>", |boxed| boxed.class.display_name())
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
    pub fn visualizable_entities_for_visualizer_systems(
        &self,
        store_id: &re_log_types::StoreId,
    ) -> PerVisualizerType<VisualizableEntities> {
        re_tracing::profile_function!();

        PerVisualizerType::<VisualizableEntities>(
            self.visualizers
                .iter()
                .map(|(id, entry)| {
                    (
                        *id,
                        ChunkStore::with_subscriber::<VisualizerEntitySubscriber, _, _>(
                            entry.entity_subscriber_handle,
                            |subscriber| subscriber.visualizable_entities(store_id).cloned(),
                        )
                        .flatten()
                        .unwrap_or_default(),
                    )
                })
                .collect(),
        )
    }

    /// For each visualizer, the set of entities that have at least one component with a matching archetype name.
    pub fn indicated_entities_per_visualizer(
        &self,
        store_id: &re_log_types::StoreId,
    ) -> PerVisualizerType<IndicatedEntities> {
        re_tracing::profile_function!();

        PerVisualizerType::<IndicatedEntities>(
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

    /// Runs the once-per-frame execution method for each context system once for each view that needs it.
    ///
    /// Passing the same view class identifier multiple times is fine,
    /// as context systems are deduplicated based on their identifiers regardless.
    pub fn run_once_per_frame_context_systems(
        &self,
        viewer_ctx: &ViewerContext<'_>,
        view_classes: impl Iterator<Item = ViewClassIdentifier>,
    ) -> IntMap<ViewSystemIdentifier, ViewContextSystemOncePerFrameResult> {
        re_tracing::profile_function!();

        use rayon::iter::{IntoParallelIterator as _, ParallelIterator as _};

        let context_system_ids = view_classes
            .filter_map(|view_class_identifier| self.view_classes.get(&view_class_identifier))
            .flat_map(|view_class| view_class.context_system_ids.iter().copied())
            .unique()
            .collect_vec();

        // TODO(andreas): Executing with rayon here is a bit of a deviation from our usual pattern.
        // It would be nicer to return something with which the user can decide on how to execute.
        context_system_ids
            .into_par_iter()
            .filter_map(|context_system_id| {
                self.context_systems.get(&context_system_id).map(|entry| {
                    (
                        context_system_id,
                        (entry.once_per_frame_execution_method)(viewer_ctx),
                    )
                })
            })
            .collect()
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
                        (*name, (system, ViewSystemState::default()))
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
}
