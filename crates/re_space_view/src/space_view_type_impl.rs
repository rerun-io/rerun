use re_viewer_context::ViewerContext;

use crate::space_view_type::{
    Scene, SceneElement, SpaceViewState, SpaceViewType, SpaceViewTypeName,
};

/// Utility for implementing [`SpaceViewType`] with a concrete state type.
pub trait SpaceViewTypeImpl {
    type State: SpaceViewState + Default + 'static;

    /// Name of this space view type.
    ///
    /// Used for both ui display and identification.
    /// Must be unique within a viewer session.
    fn type_name(&self) -> SpaceViewTypeName;

    /// Icon used to identify this space view type.
    fn type_icon(&self) -> &'static str;

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText;

    /// Returns a new scene for this space view type.
    ///
    /// Called both to determine the supported archetypes and
    /// to populate a scene every frame.
    fn new_scene(&self) -> Scene;

    /// Draws the ui for this space view type and handles ui events.
    ///
    /// The scene passed in was previously created by [`Self::new_scene`] and got populated.
    /// The state passed in was previously created by [`Self::new_state`] and is kept frame-to-frame.
    ///
    /// TODO(andreas): This is called after `re_renderer` driven content has been passed to the ui.
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        scene: Scene,
    );
}

impl<T: SpaceViewTypeImpl> SpaceViewType for T {
    #[inline]
    fn type_name(&self) -> SpaceViewTypeName {
        self.type_name()
    }

    #[inline]
    fn type_icon(&self) -> &'static str {
        self.type_icon()
    }

    #[inline]
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        self.help_text(re_ui)
    }

    #[inline]
    fn new_scene(&self) -> Scene {
        self.new_scene()
    }

    #[inline]
    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::new(T::State::default())
    }

    #[inline]
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
        scene: Scene,
    ) {
        self.ui(ctx, ui, state.as_any_mut().downcast_mut().unwrap(), scene);
    }
}
