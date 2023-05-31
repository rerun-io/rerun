use crate::{Scene, SpaceViewClass, SpaceViewClassName, SpaceViewState, ViewerContext};

use super::scene_element_list::SceneElementListConversionError;

/// Utility for implementing [`SpaceViewClass`] with concrete [`SpaceViewState`] and [`crate::SceneElement`] type.
///
/// Each Space View in the viewer's viewport has a single class assigned immutable at its creation time.
/// The class defines all aspects of its behavior.
/// It determines which entities are queried, how they are rendered, and how the user can interact with them.
pub trait SpaceViewClassImpl {
    /// State of a space view.
    type State: SpaceViewState + Default + 'static;

    /// A tuple of [`crate::SceneElement`] types that are supported by this space view class.
    type SceneElementTuple: Into<Scene>
        + TryFrom<Scene, Error = SceneElementListConversionError>
        + Default
        + 'static;

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
    fn selection_ui(&self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, state: &mut Self::State);

    /// Draws the ui for this space view class and handles ui events.
    ///
    /// The scene passed in was previously created by [`Self::new_scene`] and got populated by the time it is passed.
    /// The state passed is kept frame-to-frame.
    fn ui(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        scene_elements: Self::SceneElementTuple,
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
        T::SceneElementTuple::default().into()
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
        let scene_elements = match T::SceneElementTuple::try_from(scene) {
            Ok(scene_elements) => scene_elements,
            Err(err) => {
                re_log::error_once!("Incorrect scene type for space view class: {}", err);
                return;
            }
        };

        typed_state_wrapper(state, |state| self.ui(ctx, ui, state, scene_elements));
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
