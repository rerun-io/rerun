use re_types::{
    blueprint::{archetypes::Background, components::BackgroundKind},
    components::Color,
    Archetype,
};
use re_viewer_context::TypedComponentFallbackProvider;

use crate::SpatialSpaceView2D;

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

re_viewer_context::impl_component_fallback_provider!(SpatialSpaceView2D => [BackgroundKind, Color]);
