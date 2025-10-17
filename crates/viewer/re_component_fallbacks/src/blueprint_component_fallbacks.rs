use re_types::{blueprint, components};
use re_viewer_context::FallbackProviderRegistry;

pub fn type_fallbacks(registry: &mut FallbackProviderRegistry) {
    registry.register_default_type_fallback_provider::<blueprint::components::Enabled>();
    registry.register_default_type_fallback_provider::<blueprint::components::Corner2D>();
    registry.register_default_type_fallback_provider::<blueprint::components::BackgroundKind>();

    registry
        .register_type_fallback_provider::<blueprint::components::ForceDistance>(|_| (60.).into());
    registry
        .register_type_fallback_provider::<blueprint::components::ForceStrength>(|_| (1.).into());
    registry.register_default_type_fallback_provider::<blueprint::components::ForceIterations>();
}

pub fn archetype_fallbacks(registry: &mut FallbackProviderRegistry) {
    // LineGrid3D
    registry.register_fallback_provider(
        &blueprint::archetypes::LineGrid3D::descriptor_color(),
        |_| components::Color::from_unmultiplied_rgba(128, 128, 128, 60),
    );
    registry.register_fallback_provider(
        &blueprint::archetypes::LineGrid3D::descriptor_stroke_width(),
        |_| components::StrokeWidth::from(1.0),
    );
    registry.register_fallback_provider(
        &blueprint::archetypes::LineGrid3D::descriptor_plane(),
        |_| components::Plane3D::XY,
    );

    // Background
    registry.register_fallback_provider(
        &blueprint::archetypes::Background::descriptor_color(),
        |ctx| components::Color::from(ctx.viewer_ctx().tokens().viewport_background),
    );
    registry.register_fallback_provider(
        &blueprint::archetypes::Background::descriptor_kind(),
        |ctx| match ctx.egui_ctx().theme() {
            egui::Theme::Dark => blueprint::components::BackgroundKind::GradientDark,
            egui::Theme::Light => blueprint::components::BackgroundKind::GradientBright,
        },
    );

    // PlotBackground
    registry.register_fallback_provider(
        &blueprint::archetypes::PlotBackground::descriptor_color(),
        |ctx| components::Color::from(ctx.viewer_ctx().tokens().viewport_background),
    );

    registry.register_fallback_provider(
        &blueprint::archetypes::PlotBackground::descriptor_show_grid(),
        |_| blueprint::components::Enabled::from(true),
    );

    // GraphBackground
    registry.register_fallback_provider(
        &blueprint::archetypes::GraphBackground::descriptor_color(),
        |ctx| components::Color::from(ctx.viewer_ctx().tokens().viewport_background),
    );

    // ForceManyBody
    registry.register_fallback_provider(
        &blueprint::archetypes::ForceManyBody::descriptor_strength(),
        |_| blueprint::components::ForceStrength::from(-60.),
    );
    registry.register_fallback_provider(
        &blueprint::archetypes::ForceManyBody::descriptor_enabled(),
        |_| blueprint::components::Enabled::from(true),
    );

    // ForcePosition
    registry.register_fallback_provider(
        &blueprint::archetypes::ForcePosition::descriptor_strength(),
        |_| blueprint::components::ForceStrength::from(0.01),
    );
    registry.register_fallback_provider(
        &blueprint::archetypes::ForcePosition::descriptor_enabled(),
        |_| blueprint::components::Enabled::from(true),
    );

    // ForceLink
    registry.register_fallback_provider(
        &blueprint::archetypes::ForceLink::descriptor_enabled(),
        |_| blueprint::components::Enabled::from(true),
    );
    registry.register_fallback_provider(
        &blueprint::archetypes::ForceLink::descriptor_iterations(),
        |_| blueprint::components::ForceIterations::from(3),
    );

    // ForceCollisionRadius
    registry.register_fallback_provider(
        &blueprint::archetypes::ForceCollisionRadius::descriptor_iterations(),
        |_| blueprint::components::ForceIterations::from(1),
    );

    // ForceCollisionRadius
    registry.register_fallback_provider(
        &blueprint::archetypes::TensorScalarMapping::descriptor_colormap(),
        |_| components::Colormap::Viridis,
    );
}
