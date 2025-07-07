use re_types::{
    Archetype as _,
    blueprint::{
        archetypes::{Background, LineGrid3D},
        components::BackgroundKind,
    },
    components::{Color, Plane3D, StrokeWidth},
};
use re_viewer_context::{TypedComponentFallbackProvider, ViewStateExt as _};

use crate::{SpatialView3D, ui::SpatialViewState};

impl TypedComponentFallbackProvider<Color> for SpatialView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Color {
        // Color is a fairly common component, make sure this is the right context.
        if ctx.archetype_name == Some(Background::name()) {
            Color::WHITE
        } else if ctx.archetype_name == Some(LineGrid3D::name()) {
            Color::from_unmultiplied_rgba(128, 128, 128, 60)
        } else {
            Color::default()
        }
    }
}

impl TypedComponentFallbackProvider<BackgroundKind> for SpatialView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> BackgroundKind {
        match ctx.egui_ctx().theme() {
            egui::Theme::Dark => BackgroundKind::GradientDark,
            egui::Theme::Light => BackgroundKind::GradientBright,
        }
    }
}

impl TypedComponentFallbackProvider<StrokeWidth> for SpatialView3D {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> StrokeWidth {
        1.0.into()
    }
}

impl TypedComponentFallbackProvider<Plane3D> for SpatialView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Plane3D {
        const DEFAULT_PLANE: Plane3D = Plane3D::XY;

        let Ok(view_state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
            return DEFAULT_PLANE;
        };

        view_state
            .state_3d
            .scene_view_coordinates
            .and_then(|view_coordinates| view_coordinates.up())
            .map_or(DEFAULT_PLANE, |up| Plane3D::new(up.as_vec3(), 0.0))
    }
}
use re_types::{blueprint::components::Eye3DKind, components::LinearSpeed};

// Logic should be similar to impl TypedComponentFallbackProvider<LinearSpeed> for ViewEye
impl TypedComponentFallbackProvider<LinearSpeed> for SpatialView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> LinearSpeed {
        {
            let maybe_state = re_viewer_context::ViewStateExt::downcast_ref::<
                crate::SpatialViewState,
            >(ctx.view_ctx.view_state);
            let speed = match maybe_state {
                Ok(spatial_view_state) => {
                    let bounding_boxes = &spatial_view_state.bounding_boxes;
                    match spatial_view_state.state_3d.view_eye {
                        Some(eye) => eye.speed(bounding_boxes) as f64,
                        None => 1.0_f64,
                    }
                }
                Err(view_system_execution_error) => {
                    re_log::error!("Error while downcasting {}", view_system_execution_error);
                    1.0_f64
                }
            };
            LinearSpeed(re_types::datatypes::Float64(speed))
        }
    }
}

impl TypedComponentFallbackProvider<Eye3DKind> for SpatialView3D {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> Eye3DKind {
        Eye3DKind::default()
    }
}

re_viewer_context::impl_component_fallback_provider!(SpatialView3D => [BackgroundKind, Color, StrokeWidth, Plane3D, LinearSpeed, Eye3DKind]);
