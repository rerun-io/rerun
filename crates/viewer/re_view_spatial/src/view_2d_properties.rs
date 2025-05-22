use re_types::{
    Archetype as _,
    blueprint::{
        archetypes::Background,
        components::{BackgroundKind, VisualBounds2D},
    },
    components::Color,
};
use re_viewer_context::{TypedComponentFallbackProvider, ViewStateExt as _};

use crate::{SpatialView2D, ui::SpatialViewState};

impl TypedComponentFallbackProvider<Color> for SpatialView2D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Color {
        // Color is a fairly common component, make sure this is the right context.
        if ctx.archetype_name == Some(Background::name()) {
            ctx.viewer_ctx
                .egui_ctx()
                .style()
                .visuals
                .extreme_bg_color
                .into()
        } else {
            Color::default()
        }
    }
}

impl TypedComponentFallbackProvider<BackgroundKind> for SpatialView2D {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> BackgroundKind {
        BackgroundKind::SolidColor
    }
}

fn valid_bound(rect: &egui::Rect) -> bool {
    rect.is_finite() && rect.is_positive()
}

impl TypedComponentFallbackProvider<VisualBounds2D> for SpatialView2D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> VisualBounds2D {
        let Ok(view_state) = ctx.view_state.downcast_ref::<SpatialViewState>() else {
            return VisualBounds2D::default();
        };

        // TODO(andreas): It makes sense that we query the bounding box from the view_state,
        // but the pinhole should be an ad-hoc query instead. For this we need a little bit more state information on the QueryContext.
        let default_scene_rect = view_state
            .pinhole_at_origin
            .as_ref()
            .map(|pinhole| pinhole.resolution_rect())
            .unwrap_or_else(|| {
                // TODO(emilk): if there is a single image in this view, use that as the default bounds
                let scene_rect_smoothed = view_state.bounding_boxes.smoothed;
                egui::Rect::from_min_max(
                    scene_rect_smoothed.min.truncate().to_array().into(),
                    scene_rect_smoothed.max.truncate().to_array().into(),
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

re_viewer_context::impl_component_fallback_provider!(SpatialView2D => [BackgroundKind, Color, VisualBounds2D]);
