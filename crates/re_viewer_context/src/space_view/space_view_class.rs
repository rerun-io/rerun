use nohash_hasher::IntSet;

use re_entity_db::{EntityDb, EntityProperties, EntityPropertyMap};
use re_log_types::EntityPath;
use re_types::ComponentName;

use crate::{
    DynSpaceViewClass, IndicatedEntities, PerSystemEntities, PerVisualizer, SmallVisualizerSet,
    SpaceViewClassIdentifier, SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics,
    SpaceViewState, SpaceViewSystemExecutionError, SpaceViewSystemRegistrator,
    SystemExecutionOutput, ViewQuery, ViewerContext, VisualizableEntities,
    VisualizableFilterContext,
};

/// Defines a class of space view.
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
pub trait SpaceViewClass: std::marker::Sized + Send + Sync {
    /// State of a space view.
    type State: SpaceViewState + Default + 'static;

    /// Name for this space view class.
    ///
    /// Used as identifier.
    const IDENTIFIER: &'static str;

    /// User-facing name for this space view class
    const DISPLAY_NAME: &'static str;

    /// Name of this space view class.
    ///
    /// Used for identification. Must be unique within a viewer session.
    fn identifier(&self) -> SpaceViewClassIdentifier {
        Self::IDENTIFIER.into()
    }

    /// User-facing name for this space view class.
    ///
    /// Used for UI display.
    fn display_name(&self) -> &'static str {
        Self::DISPLAY_NAME
    }

    /// Icon used to identify this space view class.
    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_GENERIC
    }

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText;

    /// Called once upon registration of the class
    ///
    /// This can be used to register all built-in [`crate::ViewContextSystem`] and [`crate::VisualizerSystem`].
    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError>;

    /// Preferred aspect ratio for the ui tiles of this space view.
    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    /// Controls how likely this space view will get a large tile in the ui.
    fn layout_priority(&self) -> crate::SpaceViewClassLayoutPriority;

    /// Determines a suitable origin given the provided set of entities.
    ///
    /// This function only considers the transform topology, disregarding the actual visualizability
    /// of the entities (for this, use [`Self::visualizable_filter_context`]).  
    fn recommended_root_for_entities(
        &self,
        _entities: &IntSet<EntityPath>,
        _entity_db: &re_entity_db::EntityDb,
    ) -> Option<EntityPath> {
        Some(EntityPath::root())
    }

    /// Create context object that is passed to all of this classes visualizers
    /// to determine whether they can be visualized.
    ///
    /// See [`crate::VisualizerSystem::filter_visualizable_entities`].
    fn visualizable_filter_context(
        &self,
        _space_origin: &EntityPath,
        _entity_db: &re_entity_db::EntityDb,
    ) -> Box<dyn VisualizableFilterContext> {
        Box::new(())
    }

    /// Choose the default visualizers to enable for this entity.
    ///
    /// Helpful for customizing fallback behavior for types that are insufficient
    /// to determine indicated on their own.
    ///
    /// Will only be called for entities where the selected visualizers have not
    /// been overridden by the blueprint.
    ///
    /// This interface provides a default implementation which will return all visualizers
    /// which are both visualizable and indicated for the given entity.
    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizer<VisualizableEntities>,
        indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> SmallVisualizerSet {
        let available_visualizers =
            visualizable_entities_per_visualizer
                .iter()
                .filter_map(|(visualizer, ents)| {
                    if ents.contains(entity_path) {
                        Some(visualizer)
                    } else {
                        None
                    }
                });

        available_visualizers
            .filter_map(|visualizer| {
                if indicated_entities_per_visualizer
                    .get(visualizer)
                    .map_or(false, |matching_list| matching_list.contains(entity_path))
                {
                    Some(*visualizer)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Determines which space views should be spawned by default for this class.
    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics;

    /// Optional archetype of the Space View's blueprint properties.
    ///
    /// Blueprint components that only apply to the space view itself, not to the entities it displays.
    fn blueprint_archetype(&self) -> Option<Vec<ComponentName>> {
        None
    }

    /// Executed for all active space views on frame start (before any ui is drawn),
    /// can be use for heuristic & state updates before populating the scene.
    ///
    /// Is only allowed to access archetypes defined by [`Self::blueprint_archetype`]
    /// Passed entity properties are individual properties without propagated values.
    fn on_frame_start(
        &self,
        _ctx: &ViewerContext<'_>,
        _state: &Self::State,
        _ent_paths: &PerSystemEntities,
        _entity_properties: &mut re_entity_db::EntityPropertyMap,
    ) {
    }

    /// Ui shown when the user selects a space view of this class.
    ///
    /// TODO(andreas): Should this be instead implemented via a registered `data_ui` of all blueprint relevant types?
    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
        root_entity_properties: &mut EntityProperties,
    );

    /// Draws the ui for this space view class and handles ui events.
    ///
    /// The passed state is kept frame-to-frame.
    ///
    /// TODO(wumpf): Right now the ui methods control when and how to create [`re_renderer::ViewBuilder`]s.
    ///              In the future, we likely want to move view builder handling to `re_viewport` with
    ///              minimal configuration options exposed via [`crate::SpaceViewClass`].
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError>;
}

impl<T: SpaceViewClass + 'static> DynSpaceViewClass for T {
    #[inline]
    fn identifier(&self) -> SpaceViewClassIdentifier {
        self.identifier()
    }

    #[inline]
    fn identifier_str() -> &'static str {
        Self::IDENTIFIER
    }

    #[inline]
    fn display_name(&self) -> &'static str {
        self.display_name()
    }

    #[inline]
    fn icon(&self) -> &'static re_ui::Icon {
        self.icon()
    }

    #[inline]
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        self.help_text(re_ui)
    }

    #[inline]
    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<T::State>::default()
    }

    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        self.on_register(system_registry)
    }

    fn preferred_tile_aspect_ratio(&self, state: &dyn SpaceViewState) -> Option<f32> {
        typed_state_wrapper(state, |state| self.preferred_tile_aspect_ratio(state))
    }

    #[inline]
    fn layout_priority(&self) -> crate::SpaceViewClassLayoutPriority {
        self.layout_priority()
    }

    #[inline]
    fn recommended_root_for_entities(
        &self,
        entities: &IntSet<EntityPath>,
        entity_db: &EntityDb,
    ) -> Option<EntityPath> {
        self.recommended_root_for_entities(entities, entity_db)
    }

    #[inline]
    fn visualizable_filter_context(
        &self,
        space_origin: &EntityPath,
        entity_db: &re_entity_db::EntityDb,
    ) -> Box<dyn VisualizableFilterContext> {
        self.visualizable_filter_context(space_origin, entity_db)
    }

    #[inline]
    fn choose_default_visualizers(
        &self,
        entity_path: &EntityPath,
        visualizable_entities_per_visualizer: &PerVisualizer<VisualizableEntities>,
        indicated_entities_per_visualizer: &PerVisualizer<IndicatedEntities>,
    ) -> SmallVisualizerSet {
        self.choose_default_visualizers(
            entity_path,
            visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer,
        )
    }

    #[inline]
    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> SpaceViewSpawnHeuristics {
        self.spawn_heuristics(ctx)
    }

    #[inline]
    fn blueprint_archetype(&self) -> Option<Vec<ComponentName>> {
        self.blueprint_archetype()
    }

    fn on_frame_start(
        &self,
        ctx: &ViewerContext<'_>,
        state: &mut dyn SpaceViewState,
        ent_paths: &PerSystemEntities,
        entity_properties: &mut EntityPropertyMap,
    ) {
        typed_state_wrapper_mut(state, |state| {
            self.on_frame_start(ctx, state, ent_paths, entity_properties);
        });
    }

    #[inline]
    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
        root_entity_properties: &mut EntityProperties,
    ) {
        typed_state_wrapper_mut(state, |state| {
            self.selection_ui(
                ctx,
                ui,
                state,
                space_origin,
                space_view_id,
                root_entity_properties,
            );
        });
    }

    #[allow(clippy::for_kv_map)]
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) {
        re_tracing::profile_function!();

        typed_state_wrapper_mut(state, |state| {
            if let Err(err) = self.ui(ctx, ui, state, root_entity_properties, query, system_output)
            {
                // TODO(andreas): Draw an error message on top of the space view ui instead of logging.
                re_log::error_once!("Error drawing ui for space view: {err}");
            }
        });
    }
}

fn typed_state_wrapper_mut<T: SpaceViewState, R: Default, F: FnOnce(&mut T) -> R>(
    state: &mut dyn SpaceViewState,
    fun: F,
) -> R {
    if let Some(state) = state.as_any_mut().downcast_mut() {
        fun(state)
    } else {
        re_log::error_once!(
            "Unexpected space view state type. Expected {}",
            std::any::type_name::<T>()
        );
        R::default()
    }
}

fn typed_state_wrapper<T: SpaceViewState, R: Default, F: FnOnce(&T) -> R>(
    state: &dyn SpaceViewState,
    fun: F,
) -> R {
    if let Some(state) = state.as_any().downcast_ref() {
        fun(state)
    } else {
        re_log::error_once!(
            "Unexpected space view state type. Expected {}",
            std::any::type_name::<T>()
        );
        R::default()
    }
}
