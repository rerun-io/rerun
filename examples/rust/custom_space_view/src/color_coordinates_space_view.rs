use re_viewer::external::{
    egui,
    re_log_types::EntityPath,
    re_space_view, re_ui,
    re_viewer_context::{
        SpaceViewClass, SpaceViewClassName, SpaceViewId, SpaceViewState, TypedScene, ViewerContext,
    },
};

use crate::color_coordinates_scene::SceneColorCoordinates;

// TODO(andreas): This should be a blueprint component.
#[derive(Clone, PartialEq, Eq)]
pub struct CustomSpaceViewState {
    monospace: bool,
    word_wrap: bool,
}

impl Default for CustomSpaceViewState {
    fn default() -> Self {
        Self {
            monospace: false,
            word_wrap: true,
        }
    }
}

impl SpaceViewState for CustomSpaceViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct ColorCoordinatesSpaceView;

impl SpaceViewClass for ColorCoordinatesSpaceView {
    // TODO: document all of these.
    type State = re_space_view::EmptySpaceViewState;
    type SceneParts = SceneColorCoordinates;
    type Context = re_space_view::EmptySceneContext;
    type ScenePartData = ();

    fn name(&self) -> SpaceViewClassName {
        // Name and identifier of this space view.
        "Color Coordinates".into()
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_TEXT
    }

    fn help_text(&self, _re_ui: &re_ui::ReUi, _state: &Self::State) -> egui::WidgetText {
        "A demo space view that shows colors as coordinates on a 2D plane.".into()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &Self::State) -> Option<f32> {
        // Prefer a square tile if possible.
        Some(1.0)
    }

    fn selection_ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        _ui: &mut egui::Ui,
        _state: &mut Self::State,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
        // Additional UI displayed when the space view is selected.
    }

    fn ui(
        &self,
        _ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        _state: &mut Self::State,
        _scene: &mut TypedScene<Self>,
        _space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
    ) {
        // TODO:
        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| ui.label("TODO: "));
    }
}
