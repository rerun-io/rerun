use crate::{Scene, SpaceViewClass, SpaceViewClassName, SpaceViewState, ViewerContext};

/// Utility for implementing [`SpaceViewClass`] with a concrete state type.
pub trait SpaceViewClassImpl {
    type State: SpaceViewState + Default + 'static;

    /// Name of this space view class.
    ///
    /// Used for both ui display and identification.
    /// Must be unique within a viewer session.
    fn name(&self) -> SpaceViewClassName;

    /// Icon used to identify this space view class.
    fn icon(&self) -> &'static re_ui::Icon;

    /// Help text describing how to interact with this space view in the ui.
    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText;

    /// Returns a new scene for this space view class.
    ///
    /// Called both to determine the supported archetypes and
    /// to populate a scene every frame.
    fn new_scene(&self) -> Scene;

    /// Ui shown when the user selects a space view of this class.
    fn selection_ui(&self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, state: &mut Self::State);

    /// Draws the ui for this space view class and handles ui events.
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
        self.new_scene()
    }

    #[inline]
    fn new_state(&self) -> Box<dyn SpaceViewState> {
        Box::<T::State>::default()
    }

    #[inline]
    fn selection_ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn SpaceViewState,
    ) {
        self.selection_ui(ctx, ui, state.as_any_mut().downcast_mut().unwrap());
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

/// Space view state without any contents.
#[derive(Default)]
pub struct EmptySpaceViewState;

impl SpaceViewState for EmptySpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
