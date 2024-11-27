use re_types::{
    blueprint::{
        archetypes::{Background, LineGrid3D},
        components::{BackgroundKind, PlaneOrientation, UiRadius},
    },
    components::Color,
    Archetype,
};
use re_viewer_context::{SpaceViewStateExt as _, TypedComponentFallbackProvider};

use crate::{ui::SpatialSpaceViewState, SpatialSpaceView3D};

impl TypedComponentFallbackProvider<Color> for SpatialSpaceView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Color {
        // Color is a fairly common component, make sure this is the right context.
        if ctx.archetype_name == Some(Background::name()) {
            Color::WHITE
        } else if ctx.archetype_name == Some(LineGrid3D::name()) {
            Color::from_unmultiplied_rgba(200, 200, 200, 200)
        } else {
            Color::default()
        }
    }
}

impl TypedComponentFallbackProvider<BackgroundKind> for SpatialSpaceView3D {
    fn fallback_for(&self, _ctx: &re_viewer_context::QueryContext<'_>) -> BackgroundKind {
        BackgroundKind::GradientDark
    }
}

impl TypedComponentFallbackProvider<UiRadius> for SpatialSpaceView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> UiRadius {
        if ctx.archetype_name == Some(LineGrid3D::name()) {
            // 1 ui unit thickness by default.
            0.5.into()
        } else {
            1.0.into()
        }
    }
}

impl TypedComponentFallbackProvider<PlaneOrientation> for SpatialSpaceView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> PlaneOrientation {
        let Ok(view_state) = ctx.view_state.downcast_ref::<SpatialSpaceViewState>() else {
            return PlaneOrientation::default();
        };

        view_state
            .state_3d
            .scene_view_coordinates
            .and_then(|view_coordinates| view_coordinates.up())
            .map_or(PlaneOrientation::default(), |up| match up.axis {
                re_types::view_coordinates::Axis3::X => PlaneOrientation::Yz,
                re_types::view_coordinates::Axis3::Y => PlaneOrientation::Xz,
                re_types::view_coordinates::Axis3::Z => PlaneOrientation::Xy,
            })
    }
}

re_viewer_context::impl_component_fallback_provider!(SpatialSpaceView3D => [BackgroundKind, Color, UiRadius, PlaneOrientation]);
