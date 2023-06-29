use nohash_hasher::IntSet;
use re_data_store::EntityPropertyMap;
use re_log_types::EntityPath;

use crate::{
    ArchetypeDefinition, DynSpaceViewClass, Scene, SceneContext, ScenePartCollection,
    SpaceViewClassName, SpaceViewId, SpaceViewState, ViewerContext,
};

use super::scene::TypedScene;

/// Defines a class of space view.
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
pub trait SpaceViewClass: std::marker::Sized {
    /// State of a space view.
    type State: SpaceViewState + Default + 'static;

    /// Context of the scene, which is passed to all [`crate::ScenePart`]s and ui drawing on population.
    type Context: SceneContext + Default + 'static;

    /// Collection of [`crate::ScenePart`]s that this scene populates.
    type SceneParts: ScenePartCollection<Self> + Default + 'static;

    /// A piece of data that all scene parts have in common, useful for iterating over them.
    ///
    /// This is useful for retrieving data that is common to all scene parts of a [`SpaceViewClass`].
    /// For example, if most scene parts produce ui elements, a concrete [`SpaceViewClass`]
    /// can pick those up in its [`SpaceViewClass::ui`] method by iterating over all scene parts.
    type ScenePartData;

    /// Name of this space view class.
    ///
    /// Used for both ui display and identification.
    /// Must be unique within a viewer session.
    fn name(&self) -> SpaceViewClassName;

    /// Icon used to identify this space view class.
    fn icon(&self) -> &'static re_ui::Icon;

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi, state: &Self::State) -> egui::WidgetText;

    /// Preferred aspect ratio for the ui tiles of this space view.
    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        None
    }

    /// Controls how likely this space view will get a large tile in the ui.
    fn layout_priority(&self) -> crate::SpaceViewClassLayoutPriority;

    /// Optional archetype of the Space View's blueprint properties.
    ///
    /// Blueprint components that only apply to the space view itself, not to the entities it displays.
    fn blueprint_archetype(&self) -> Option<ArchetypeDefinition> {
        None
    }

    /// Executed before the scene is populated, can be use for heuristic & state updates before populating the scene.
    ///
    /// Is only allowed to access archetypes defined by [`Self::blueprint_archetype`].
    /// Passed entity properties are individual properties without propagated values.
    fn prepare_populate(
        &self,
        _ctx: &mut ViewerContext<'_>,
        _state: &Self::State,
        _entity_paths: &IntSet<EntityPath>,
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
    );

    /// Draws the ui for this space view class and handles ui events.
    ///
    /// The passed scene is already populated for this frame
    /// The passed state is kept frame-to-frame.
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        scene: &mut TypedScene<Self>,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    );
}

impl<T: SpaceViewClass + 'static> DynSpaceViewClass for T {
    #[inline]
    fn name(&self) -> SpaceViewClassName {
        self.name()
    }

    #[inline]
    fn icon(&self) -> &'static re_ui::Icon {
        self.icon()
    }

    #[inline]
    fn help_text(&self, re_ui: &re_ui::ReUi, state: &dyn SpaceViewState) -> egui::WidgetText {
        typed_state_wrapper(state, |state| self.help_text(re_ui, state))
    }

    #[inline]
    fn new_scene(&self) -> Box<dyn Scene> {
        Box::<TypedScene<Self>>::default()
    }

    #[inline]
    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<T::State>::default()
    }

    fn preferred_tile_aspect_ratio(&self, state: &dyn SpaceViewState) -> Option<f32> {
        typed_state_wrapper(state, |state| self.preferred_tile_aspect_ratio(state))
    }

    #[inline]
    fn layout_priority(&self) -> crate::SpaceViewClassLayoutPriority {
        self.layout_priority()
    }

    fn blueprint_archetype(&self) -> Option<ArchetypeDefinition> {
        self.blueprint_archetype()
    }

    fn prepare_populate(
        &self,
        ctx: &mut ViewerContext<'_>,
        state: &mut dyn SpaceViewState,
        entity_paths: &IntSet<EntityPath>,
        entity_properties: &mut EntityPropertyMap,
    ) {
        typed_state_wrapper_mut(state, |state| {
            self.prepare_populate(ctx, state, entity_paths, entity_properties);
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
    ) {
        typed_state_wrapper_mut(state, |state| {
            self.selection_ui(ctx, ui, state, space_origin, space_view_id);
        });
    }

    #[inline]
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        mut scene: Box<dyn Scene>,
        space_origin: &EntityPath,
        space_view_id: SpaceViewId,
    ) {
        let Some(typed_scene) = scene.as_any_mut().downcast_mut()
            else {
                re_log::error_once!("Unexpected space view state type. Expected {}",
                                    std::any::type_name::<TypedScene<T>>());
                return;
            };

        typed_state_wrapper_mut(state, |state| {
            self.ui(ctx, ui, state, typed_scene, space_origin, space_view_id);
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
