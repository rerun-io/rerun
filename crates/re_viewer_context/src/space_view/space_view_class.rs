use re_data_store::{EntityProperties, EntityPropertyMap};
use re_log_types::EntityPath;
use re_types::ComponentName;

use crate::{
    AutoSpawnHeuristic, DynSpaceViewClass, PerSystemEntities, SpaceViewClassName,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewState, SpaceViewSystemExecutionError,
    SpaceViewSystemRegistry, ViewContextCollection, ViewPartCollection, ViewQuery, ViewerContext,
};

/// Defines a class of space view.
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
pub trait SpaceViewClass: std::marker::Sized {
    /// State of a space view.
    type State: SpaceViewState + Default + 'static;

    /// Name for this space view class.
    ///
    /// Used as identifier.
    const NAME: &'static str;

    /// User-facing name for this space view class
    const DISPLAY_NAME: &'static str;

    /// Name of this space view class.
    ///
    /// Used for identification. Must be unique within a viewer session.
    fn name(&self) -> SpaceViewClassName {
        Self::NAME.into()
    }

    /// User-facing name for this space view class.
    ///
    /// Used for UI display.
    fn display_name(&self) -> &'static str {
        Self::DISPLAY_NAME
    }

    /// Icon used to identify this space view class.
    fn icon(&self) -> &'static re_ui::Icon;

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText;

    /// Called once upon registration of the class
    ///
    /// This can be used to register all built-in [`crate::ViewContextSystem`] and [`crate::ViewPartSystem`].
    fn on_register(
        &self,
        system_registry: &mut SpaceViewSystemRegistry,
    ) -> Result<(), SpaceViewClassRegistryError>;

    /// Preferred aspect ratio for the ui tiles of this space view.
    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    /// Controls how likely this space view will get a large tile in the ui.
    fn layout_priority(&self) -> crate::SpaceViewClassLayoutPriority;

    /// Heuristic used to determine which space view is the best fit for a set of paths.
    ///
    /// For each path in `ent_paths`, at least one of the registered [`crate::ViewPartSystem`] for this class
    /// returned true when calling [`crate::ViewPartSystem::heuristic_filter`].
    fn auto_spawn_heuristic(
        &self,
        _ctx: &ViewerContext<'_>,
        _space_origin: &EntityPath,
        ent_paths: &PerSystemEntities,
    ) -> AutoSpawnHeuristic {
        AutoSpawnHeuristic::SpawnClassWithHighestScoreForRoot(ent_paths.len() as f32)
    }

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
        _ctx: &mut ViewerContext<'_>,
        _state: &Self::State,
        _ent_paths: &PerSystemEntities,
        _entity_properties: &mut re_data_store::EntityPropertyMap,
    ) {
    }

    /// Ui shown when the user selects a space view of this class.
    ///
    /// TODO(andreas): Should this be instead implemented via a registered `data_ui` of all blueprint relevant types?
    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
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
    /// The passed systems (`view_ctx` and `parts`) are only valid for the duration of this frame and
    /// were already executed upon entering this method.
    ///
    /// `draw_data` is all draw data gathered by executing the view part systems.
    /// TODO(wumpf): Right now the ui methods control when and how to create [`re_renderer::ViewBuilder`]s.
    ///              In the future, we likely want to move view builder handling to `re_viewport` with
    ///              minimal configuration options exposed via [`crate::SpaceViewClass`].
    #[allow(clippy::too_many_arguments)]
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        root_entity_properties: &EntityProperties,
        view_ctx: &ViewContextCollection,
        parts: &ViewPartCollection,
        query: &ViewQuery<'_>,
        draw_data: Vec<re_renderer::QueueableDrawData>,
    ) -> Result<(), SpaceViewSystemExecutionError>;
}

impl<T: SpaceViewClass + 'static> DynSpaceViewClass for T {
    #[inline]
    fn name(&self) -> SpaceViewClassName {
        self.name()
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
        system_registry: &mut SpaceViewSystemRegistry,
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
    fn auto_spawn_heuristic(
        &self,
        ctx: &ViewerContext<'_>,
        space_origin: &EntityPath,
        ent_paths: &PerSystemEntities,
    ) -> AutoSpawnHeuristic {
        self.auto_spawn_heuristic(ctx, space_origin, ent_paths)
    }

    #[inline]
    fn blueprint_archetype(&self) -> Option<Vec<ComponentName>> {
        self.blueprint_archetype()
    }

    fn on_frame_start(
        &self,
        ctx: &mut ViewerContext<'_>,
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
        ctx: &mut ViewerContext<'_>,
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
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        root_entity_properties: &EntityProperties,
        systems: &SpaceViewSystemRegistry,
        query: &ViewQuery<'_>,
    ) {
        re_tracing::profile_function!();

        // TODO(andreas): We should be able to parallelize both of these loops
        let view_ctx = {
            re_tracing::profile_scope!("ViewContextSystem::execute");
            let mut view_ctx = systems.new_context_collection(self.name());
            for (_name, system) in &mut view_ctx.systems {
                re_tracing::profile_scope!(_name.as_str());
                system.execute(ctx, query);
            }
            view_ctx
        };
        let (parts, draw_data) = {
            re_tracing::profile_scope!("ViewPartSystem::execute");
            let mut parts = systems.new_part_collection();
            let mut draw_data = Vec::new();
            for (name, part) in &mut parts.systems {
                re_tracing::profile_scope!(name.as_str());
                match part.execute(ctx, query, &view_ctx) {
                    Ok(part_draw_data) => draw_data.extend(part_draw_data),
                    Err(err) => {
                        re_log::error_once!("Error executing view part system {name:?}: {err}");
                    }
                }
            }
            (parts, draw_data)
        };

        typed_state_wrapper_mut(state, |state| {
            if let Err(err) = self.ui(
                ctx,
                ui,
                state,
                root_entity_properties,
                &view_ctx,
                &parts,
                query,
                draw_data,
            ) {
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
