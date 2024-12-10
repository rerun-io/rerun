use re_types::blueprint::components::VisualBounds2D;
use re_ui::zoom_pan_area::fit_to_rect_in_scene;
use re_viewer_context::{SpaceViewStateExt as _, TypedComponentFallbackProvider};

use crate::{ui::GraphSpaceViewState, GraphSpaceView};

fn valid_bound(rect: &egui::Rect) -> bool {
    rect.is_finite() && rect.is_positive()
}

impl TypedComponentFallbackProvider<VisualBounds2D> for GraphSpaceView {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> VisualBounds2D {
        let Ok(state) = ctx.view_state.downcast_ref::<GraphSpaceViewState>() else {
            return VisualBounds2D::default();
        };

        match state.layout_state.bounding_rect() {
            Some(rect) if valid_bound(&rect) => {
                if let Some(rect_in_ui) = state.rect_in_ui {
                    let ui_from_world = fit_to_rect_in_scene(rect_in_ui, rect);
                    (ui_from_world.inverse() * rect_in_ui).into()
                } else {
                    rect.into()
                }
            }
            _ => VisualBounds2D::default(),
        }
    }
}

re_viewer_context::impl_component_fallback_provider!(GraphSpaceView => [VisualBounds2D]);
