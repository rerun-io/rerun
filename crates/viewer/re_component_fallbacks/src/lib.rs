//! This crate implements various component fallbacks.
//!
//! The only entry point is [`create_component_fallback_registry`], which registers all component type, and
//! component identifier fallbacks to a new [`FallbackProviderRegistry`].
//! This should be called by `re_viewer` on startup.

use re_viewer_context::FallbackProviderRegistry;

mod blueprint_component_fallbacks;
mod component_fallbacks;

/// Creates a new [`FallbackProviderRegistry`] and registers built-in
/// type and archetype field fallbacks.
pub fn create_component_fallback_registry() -> FallbackProviderRegistry {
    let mut registry = FallbackProviderRegistry::default();

    blueprint_component_fallbacks::type_fallbacks(&mut registry);
    blueprint_component_fallbacks::archetype_field_fallbacks(&mut registry);

    component_fallbacks::type_fallbacks(&mut registry);
    component_fallbacks::archetype_field_fallbacks(&mut registry);

    registry
}
