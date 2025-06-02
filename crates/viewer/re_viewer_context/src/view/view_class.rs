use nohash_hasher::IntSet;

use re_entity_db::EntityDb;
use re_log_types::EntityPath;
use re_types::{ComponentName, ViewClassIdentifier};

use crate::{
    IndicatedEntities, MaybeVisualizableEntities, PerVisualizer, QueryRange, SmallVisualizerSet,
    SystemExecutionOutput, ViewClassRegistryError, ViewId, ViewQuery, ViewSpawnHeuristics,
    ViewSystemExecutionError, ViewSystemRegistrator, ViewerContext, VisualizableEntities,
};

use super::ViewContext;

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Ord, Eq)]
pub enum ViewClassLayoutPriority {
    /// This view can share space with others
    ///
    /// Used for boring things like text and plots.
    Low,

    #[default]
    Medium,

    /// Give this view lots of space.
    /// Used for spatial views (2D/3D).
    High,
}

/// Context object returned by [`crate::ViewClass::visualizable_filter_context`].
pub trait VisualizableFilterContext {
    fn as_any(&self) -> &dyn std::any::Any;
}

impl VisualizableFilterContext for () {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Defines a class of view without any concrete types making it suitable for storage and interfacing.
///
/// Each View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
//
// TODO(andreas): Consider formulating a view instance context object that is passed to all
// methods that operate on concrete views as opposed to be about general information on the class.
pub trait ViewClass: Send + Sync {
    /// Identifier string of this view class.
    ///
    /// By convention we use `PascalCase`.
    fn identifier() -> ViewClassIdentifier
    where
        Self: Sized;

    /// User-facing name of this view class.
    ///
    /// Used for UI display.
    fn display_name(&self) -> &'static str;

    /// Icon used to identify this view class.
    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_GENERIC
    }

    fn help(&self, os: egui::os::OperatingSystem) -> re_ui::Help;

    /// Called once upon registration of the class
    ///
    /// This can be used to register all built-in [`crate::ViewContextSystem`] and [`crate::VisualizerSystem`].
    fn on_register(
        &self,
        system_registry: &mut ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError>;

    /// Called once for every new view instance of this class.
    ///
    /// The state is *not* persisted across viewer sessions, only shared frame-to-frame.
    fn new_state(&self) -> Box<dyn ViewState>;

    /// Optional archetype of the View's blueprint properties.
    ///
    /// Blueprint components that only apply to the view itself, not to the entities it displays.
    fn blueprint_archetype(&self) -> Option<Vec<ComponentName>> {
        None
    }

    /// Preferred aspect ratio for the ui tiles of this view.
    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        None
    }

    /// Controls how likely this view will get a large tile in the ui.
    fn layout_priority(&self) -> ViewClassLayoutPriority;

    /// Controls whether the visible time range UI should be displayed for this view.
    fn supports_visible_time_range(&self) -> bool {
        false
    }

    /// Default query range for this view.
    //TODO(#6918): also provide ViewerContext and ViewId, to enable reading view properties.
    fn default_query_range(&self, _state: &dyn ViewState) -> QueryRange {
        QueryRange::LatestAt
    }

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
        _maybe_visualizable_entities_per_visualizer: &PerVisualizer<MaybeVisualizableEntities>,
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
                    .is_some_and(|matching_list| matching_list.contains(entity_path))
                {
                    Some(*visualizer)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Determines which views should be spawned by default for this class.
    ///
    /// Only entities matching `include_entity` should be considered,
    /// though this is only a suggestion and may be
    /// overwritten if a view decides to display more data.
    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics;

    /// Ui shown when the user selects a view of this class.
    fn selection_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        Ok(())
    }

    /// Additional UI displayed in the tab title bar, between the "maximize" and "help" buttons.
    ///
    /// Note: this is a right-to-left layout.
    fn extra_title_bar_ui(
        &self,
        _ctx: &ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        _view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        Ok(())
    }

    /// Draws the ui for this view class and handles ui events.
    ///
    /// The passed state is kept frame-to-frame.
    ///
    /// TODO(wumpf): Right now the ui methods control when and how to create [`re_renderer::ViewBuilder`]s.
    ///              In the future, we likely want to move view builder handling to `re_viewport` with
    ///              minimal configuration options exposed via [`crate::ViewClass`].
    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError>;

    /// Determines the set of visible entities for a given view.
    // TODO(andreas): This should be part of the View's (non-blueprint) state.
    // Updated whenever `maybe_visualizable_entities_per_visualizer` or the view blueprint changes.
    fn determine_visualizable_entities(
        &self,
        maybe_visualizable_entities_per_visualizer: &PerVisualizer<MaybeVisualizableEntities>,
        entity_db: &EntityDb,
        visualizers: &crate::VisualizerCollection,
        space_origin: &EntityPath,
    ) -> PerVisualizer<VisualizableEntities> {
        re_tracing::profile_function!();

        let filter_ctx = self.visualizable_filter_context(space_origin, entity_db);

        PerVisualizer::<VisualizableEntities>(
            visualizers
                .iter_with_identifiers()
                .map(|(visualizer_identifier, visualizer_system)| {
                    let entities = if let Some(maybe_visualizable_entities) =
                        maybe_visualizable_entities_per_visualizer.get(&visualizer_identifier)
                    {
                        visualizer_system.filter_visualizable_entities(
                            maybe_visualizable_entities.clone(),
                            filter_ctx.as_ref(),
                        )
                    } else {
                        VisualizableEntities::default()
                    };

                    (visualizer_identifier, entities)
                })
                .collect(),
        )
    }
}

pub trait ViewClassExt<'a>: ViewClass + 'a {
    fn view_context<'b>(
        &self,
        viewer_ctx: &'b ViewerContext<'b>,
        view_id: ViewId,
        view_state: &'b dyn ViewState,
    ) -> ViewContext<'b>;
}

impl<'a, T> ViewClassExt<'a> for T
where
    T: ViewClass + 'a,
{
    fn view_context<'b>(
        &self,
        viewer_ctx: &'b ViewerContext<'b>,
        view_id: ViewId,
        view_state: &'b dyn ViewState,
    ) -> ViewContext<'b> {
        ViewContext {
            viewer_ctx,
            view_id,
            view_class_identifier: T::identifier(),
            view_state,
            query_result: viewer_ctx.lookup_query_result(view_id),
        }
    }
}

/// Unserialized frame to frame state of a view.
///
/// For any state that should be persisted, use the Blueprint!
/// This state is used for transient state, such as animation or uncommitted ui state like dragging a camera.
/// (on mouse release, the camera would be committed to the blueprint).
pub trait ViewState: std::any::Any + Sync + Send {
    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Converts itself to a reference of [`std::any::Any`], which enables downcasting to concrete types.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Implementation of an empty view state.
impl ViewState for () {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

pub trait ViewStateExt: ViewState {
    /// Downcasts this state to a reference of a concrete type.
    fn downcast_ref<T: ViewState>(&self) -> Result<&T, ViewSystemExecutionError> {
        self.as_any()
            .downcast_ref()
            .ok_or(ViewSystemExecutionError::StateCastError(
                std::any::type_name::<T>(),
            ))
    }

    /// Downcasts this state to a mutable reference of a concrete type.
    fn downcast_mut<T: ViewState>(&mut self) -> Result<&mut T, ViewSystemExecutionError> {
        self.as_any_mut()
            .downcast_mut()
            .ok_or(ViewSystemExecutionError::StateCastError(
                std::any::type_name::<T>(),
            ))
    }
}

impl ViewStateExt for dyn ViewState {}
