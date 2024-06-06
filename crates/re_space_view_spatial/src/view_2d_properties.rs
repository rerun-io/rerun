use re_types::{
    archetypes::Pinhole,
    blueprint::{
        archetypes::Background,
        components::{BackgroundKind, VisualBounds2D},
    },
    components::Color,
    Archetype,
};
use re_viewer_context::{SpaceViewStateExt, TypedComponentFallbackProvider};

use crate::{ui::SpatialSpaceViewState, SpatialSpaceView2D};

impl TypedComponentFallbackProvider<Color> for SpatialSpaceView2D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Color {
        // Color is a fairly common component, make sure this is the right context.
        if ctx.archetype_name == Some(Background::name()) {
            Color::BLACK
        } else {
            Color::default()
        }
    }
}

impl TypedComponentFallbackProvider<BackgroundKind> for SpatialSpaceView2D {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> BackgroundKind {
        BackgroundKind::SolidColor
    }
}

fn valid_bound(rect: &egui::Rect) -> bool {
    rect.is_finite() && rect.is_positive()
}

/// The pinhole sensor rectangle: [0, 0] - [width, height],
/// ignoring principal point.
fn pinhole_resolution_rect(pinhole: &Pinhole) -> Option<egui::Rect> {
    pinhole
        .resolution()
        .map(|res| egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(res.x, res.y)))
}

impl TypedComponentFallbackProvider<VisualBounds2D> for SpatialSpaceView2D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> VisualBounds2D {
        let Ok(view_state) = ctx
            .view_ctx
            .view_state
            .downcast_ref::<SpatialSpaceViewState>()
        else {
            return VisualBounds2D::default();
        };

        // TODO(andreas): It makes sense that we query the bounding box from the view_state,
        // but the pinhole should be an ad-hoc query instead. For this we need a little bit more state information on the QueryContext.
        let default_scene_rect = view_state
            .pinhole_at_origin
            .as_ref()
            .and_then(pinhole_resolution_rect)
            .unwrap_or_else(|| {
                // TODO(emilk): if there is a single image in this view, use that as the default bounds
                let scene_rect_accum = view_state.bounding_boxes.accumulated;
                egui::Rect::from_min_max(
                    scene_rect_accum.min.truncate().to_array().into(),
                    scene_rect_accum.max.truncate().to_array().into(),
                )
            });

        if valid_bound(&default_scene_rect) {
            default_scene_rect.into()
        } else {
            // Nothing in scene, probably.
            VisualBounds2D::default()
        }
    }
}

re_viewer_context::impl_component_fallback_provider!(SpatialSpaceView2D => [BackgroundKind, Color, VisualBounds2D]);
