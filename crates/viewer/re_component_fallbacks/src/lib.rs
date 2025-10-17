use re_viewer_context::FallbackProviderRegistry;

mod blueprint_component_fallbacks;
mod component_fallbacks;

pub fn create_component_fallback_registry() -> FallbackProviderRegistry {
    let mut registry = FallbackProviderRegistry::default();

    blueprint_component_fallbacks::type_fallbacks(&mut registry);
    blueprint_component_fallbacks::archetype_fallbacks(&mut registry);

    component_fallbacks::type_fallbacks(&mut registry);
    component_fallbacks::archetype_fallbacks(&mut registry);

    registry
}
