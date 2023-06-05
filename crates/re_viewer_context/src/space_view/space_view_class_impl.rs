use crate::{
    Scene, SceneContext, ScenePartCollection, SpaceViewClass, SpaceViewClassName, SpaceViewState,
    ViewerContext,
};

use super::scene::TypedScene;

/// Utility for implementing [`SpaceViewClass`] with concrete [`SpaceViewState`] and [`crate::ScenePart`] type.
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
pub trait SpaceViewClassImpl: std::marker::Sized {
    /// State of a space view.
    type SpaceViewState: SpaceViewState + Default + 'static;

    /// Context of the scene, which is passed to all [`crate::ScenePart`]s and ui drawing on population.
    type SceneContext: SceneContext + Default + 'static;

    /// Collection of [`crate::ScenePart`]s that this scene populates.
    type ScenePartCollection: ScenePartCollection<Self> + Default + 'static;

    /// A piece of data that all scene parts have in common, useful for iterating over them.
    ///
    /// This is useful for retrieving data that is common to all scene parts of a [`SpaceViewClass`].
    /// For example, if most scene parts produce ui elements, a concrete [`SpaceViewClassImpl`]
    /// can pick those up in its [`SpaceViewClassImpl::ui`] method by iterating over all scene parts.
    type ScenePartData;

    /// Name of this space view class.
    ///
    /// Used for both ui display and identification.
    /// Must be unique within a viewer session.
    fn name(&self) -> SpaceViewClassName;

    /// Icon used to identify this space view class.
    fn icon(&self) -> &'static re_ui::Icon;

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText;

    /// Ui shown when the user selects a space view of this class.
    ///
    /// TODO(andreas): Should this be instead implemented via a registered `data_ui` of all blueprint relevant types?
    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::SpaceViewState,
    );

    /// Draws the ui for this space view class and handles ui events.
    ///
    /// The passed scene is already populated for this frame
    /// The passed state is kept frame-to-frame.
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::SpaceViewState,
        scene: &TypedScene<Self>,
    );
}

impl<T: SpaceViewClassImpl + 'static> SpaceViewClass for T {
    #[inline]
    fn name(&self) -> SpaceViewClassName {
        self.name()
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
    fn new_scene(&self) -> Box<dyn Scene> {
        Box::<TypedScene<Self>>::default()
    }

    #[inline]
    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<T::SpaceViewState>::default()
    }

    #[inline]
    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
    ) {
        typed_state_wrapper(state, |state| self.selection_ui(ctx, ui, state));
    }

    #[inline]
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        scene: Box<dyn Scene>,
    ) {
        let Some(typed_scene) = scene.as_any().downcast_ref()
            else {
                re_log::error_once!("Unexpected space view state type. Expected {}",
                                    std::any::type_name::<TypedScene<T>>());
                return;
            };

        typed_state_wrapper(state, |state| self.ui(ctx, ui, state, typed_scene));
    }
}

fn typed_state_wrapper<T: SpaceViewState, F: FnOnce(&mut T)>(
    state: &mut dyn SpaceViewState,
    fun: F,
) {
    if let Some(state) = state.as_any_mut().downcast_mut() {
        fun(state);
    } else {
        re_log::error_once!(
            "Unexpected space view state type. Expected {}",
            std::any::type_name::<T>()
        );
    }
}
