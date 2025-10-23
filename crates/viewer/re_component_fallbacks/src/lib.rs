//! This crate implements various component fallbacks.
//!
//! The only entry point is [`create_component_fallback_registry`], which registers all component type, and
//! component identifier fallbacks to a new [`FallbackProviderRegistry`].
//! This should be called by `re_viewer` on startup.
//!
//! Component fallbacks should be registered here **if**:
//! - They're not the same as the `provided_fallback` from
//!   reflection.
//! - And They're used in more than one view.
//! - And doesn't require specific dependencies from views that makes it not possible to add here without
//!   adding dependencies.
//!
//! If a fallback has dependencies in a specific visualizer, or that visualizer requires a fallback and is shared
//! between multiple views, it should be registered in that visualizers `on_register` method.
//!
//! Otherwise the fallback should be registered in the view class it's used in, on that view's `on_register` method.

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
