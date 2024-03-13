use nohash_hasher::IntSet;
use re_entity_db::{EntityProperties, EntityPropertyMap};
use re_log_types::EntityPath;
use re_types::ComponentName;

use crate::{
    IndicatedEntities, PerSystemEntities, PerVisualizer, SmallVisualizerSet,
    SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics,
    SpaceViewSystemExecutionError, SpaceViewSystemRegistrator, SystemExecutionOutput, ViewQuery,
    ViewerContext, VisualizableEntities,
};

re_string_interner::declare_new_type!(
    /// The unique name of a space view type.
    #[derive(serde::Deserialize, serde::Serialize)]
    pub struct SpaceViewClassIdentifier;
);

impl SpaceViewClassIdentifier {
    pub fn invalid() -> Self {
        Self::from("invalid")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Ord, Eq)]
pub enum SpaceViewClassLayoutPriority {
    /// This space view can share space with others
    ///
    /// Used for boring things like text and plots.
    Low,

    #[default]
    Medium,

    /// Give this space view lots of space.
    /// Used for spatial views (2D/3D).
    High,
}

/// Context object returned by [`crate::SpaceViewClass::visualizable_filter_context`].
pub trait VisualizableFilterContext {
    fn as_any(&self) -> &dyn std::any::Any;
}

impl VisualizableFilterContext for () {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Defines a class of space view without any concrete types making it suitable for storage and interfacing.
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
///
/// TODO(andreas): Consider formulating a space view instance context object that is passed to all
/// methods that operate on concrete space views as opposed to be about general information on the class.
pub trait SpaceViewClass: Send + Sync {
    /// Identifier string of this space view class.
    ///
    /// This is similar to [`Self::identifier`] but it can be used without an actual instance of the
    /// class.
    fn identifier() -> SpaceViewClassIdentifier
    where
        Self: Sized;

    /// User-facing name of this space view class.
    ///
    /// Used for UI display.
    fn display_name(&self) -> &'static str;

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

    /// Called once for every new space view instance of this class.
    ///
    /// The state is *not* persisted across viewer sessions, only shared frame-to-frame.
    fn new_state(&self) -> Box<dyn SpaceViewState>;

    /// Optional archetype of the Space View's blueprint properties.
    ///
    /// Blueprint components that only apply to the space view itself, not to the entities it displays.
    fn blueprint_archetype(&self) -> Option<Vec<ComponentName>> {
        None
    }

    /// Preferred aspect ratio for the ui tiles of this space view.
    fn preferred_tile_aspect_ratio(&self, _state: &dyn SpaceViewState) -> Option<f32> {
        None
    }

    /// Controls how likely this space view will get a large tile in the ui.
    fn layout_priority(&self) -> SpaceViewClassLayoutPriority;

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
    /// to determine whether they can be visualized
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

    /// Executed for all active space views on frame start (before any ui is drawn),
    /// can be use for heuristic & state updates before populating the scene.
    ///
    /// Is only allowed to access archetypes defined by [`Self::blueprint_archetype`]
    /// Passed entity properties are individual properties without propagated values.
    fn on_frame_start(
        &self,
        _ctx: &ViewerContext<'_>,
        _state: &mut dyn SpaceViewState,
        _ent_paths: &PerSystemEntities,
        _entity_properties: &mut EntityPropertyMap,
    ) {
    }

    /// Ui shown when the user selects a space view of this class.
    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut dyn SpaceViewState,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        Ok(())
    }

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
        state: &mut dyn SpaceViewState,
        root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError>;
}

/// Unserialized frame to frame state of a space view.
///
/// For any state that should be persisted, use the Blueprint!
/// This state is used for transient state, such as animation or uncommitted ui state like dragging a camera.
/// (on mouse release, the camera would be committed to the blueprint).
pub trait SpaceViewState: std::any::Any {
    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Implementation of an empty space view state.
impl SpaceViewState for () {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub trait SpaceViewStateExt: SpaceViewState {
    /// Downcasts this state to a reference of a concrete type.
    fn downcast_ref<T: SpaceViewState>(&self) -> Result<&T, SpaceViewSystemExecutionError> {
        self.as_any()
            .downcast_ref()
            .ok_or(SpaceViewSystemExecutionError::StateCastError(
                std::any::type_name::<T>(),
            ))
    }

    /// Downcasts this state to a mutable reference of a concrete type.
    fn downcast_mut<T: SpaceViewState>(&mut self) -> Result<&mut T, SpaceViewSystemExecutionError> {
        self.as_any_mut()
            .downcast_mut()
            .ok_or(SpaceViewSystemExecutionError::StateCastError(
                std::any::type_name::<T>(),
            ))
    }
}

impl SpaceViewStateExt for dyn SpaceViewState {}
