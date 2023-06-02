use crate::{
    Scene, SceneContext, ScenePartCollection, SpaceViewClass, SpaceViewClassName, SpaceViewState,
    ViewerContext,
};

/// Utility for implementing [`SpaceViewClass`] with concrete [`SpaceViewState`] and [`crate::ScenePart`] type.
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
pub trait SpaceViewClassImpl {
    /// State of a space view.
    type SpaceViewState: SpaceViewState + Default + 'static;

    /// Context of the scene, which is passed to all [`crate::ScenePart`]s and ui drawing on population.
    type SceneContext: SceneContext + Default + 'static;

    /// A tuple of [`crate::ScenePart`] types that are supported by this space view class.
    type ScenePartTuple: Into<ScenePartCollection> + Default + 'static;

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
        scene: Scene,
    );
}

impl<T: SpaceViewClassImpl> SpaceViewClass for T {
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
    fn new_scene(&self) -> Scene {
        Scene {
            context: Box::<T::SceneContext>::default(),
            elements: T::ScenePartTuple::default().into(),
            highlights: Default::default(),
        }
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
        scene: Scene,
    ) {
        typed_state_wrapper(state, |state| self.ui(ctx, ui, state, scene));
    }
}

fn typed_state_wrapper<T: SpaceViewState, F: FnOnce(&mut T)>(
    state: &mut dyn SpaceViewState,
    fun: F,
) {
    if let Some(state) = state.as_any_mut().downcast_mut() {
        fun(state);
    } else {
        re_log::error_once!("Incorrect type of space view state.");
    }
}
