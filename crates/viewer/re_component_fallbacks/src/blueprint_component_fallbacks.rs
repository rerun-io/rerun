use re_sdk_types::{blueprint, components};
use re_viewer_context::FallbackProviderRegistry;

pub fn type_fallbacks(registry: &mut FallbackProviderRegistry) {
    registry.register_type_fallback_provider(|_| blueprint::components::ForceDistance::from(60.));
    registry.register_type_fallback_provider(|_| blueprint::components::ForceStrength::from(1.));
}

pub fn archetype_field_fallbacks(registry: &mut FallbackProviderRegistry) {
    // Background
    registry.register_component_fallback_provider(
        blueprint::archetypes::Background::descriptor_color().component,
        |ctx| components::Color::from(ctx.viewer_ctx().tokens().viewport_background),
    );

    // PlotBackground
    registry.register_component_fallback_provider(
        blueprint::archetypes::PlotBackground::descriptor_color().component,
        |ctx| components::Color::from(ctx.viewer_ctx().tokens().viewport_background),
    );

    registry.register_component_fallback_provider(
        blueprint::archetypes::PlotBackground::descriptor_show_grid().component,
        |_| blueprint::components::Enabled::from(true),
    );

    // GraphBackground
    registry.register_component_fallback_provider(
        blueprint::archetypes::GraphBackground::descriptor_color().component,
        |ctx| components::Color::from(ctx.viewer_ctx().tokens().viewport_background),
    );
}
