use re_types::{
    blueprint::{archetypes::Background, components::BackgroundKind},
    components::Color,
    Archetype,
};
use re_viewer_context::TypedComponentFallbackProvider;

use crate::SpatialSpaceView3D;

impl TypedComponentFallbackProvider<Color> for SpatialSpaceView3D {
    fn fallback_for(&self, ctx: &re_viewer_context::QueryContext<'_>) -> Color {
        // Color is a fairly common component, make sure this is the right context.
        if ctx.archetype_name == Some(Background::name()) {
            Color::WHITE
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

re_viewer_context::impl_component_fallback_provider!(SpatialSpaceView3D => [BackgroundKind, Color]);
