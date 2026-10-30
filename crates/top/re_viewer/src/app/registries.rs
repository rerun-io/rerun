use std::sync::Arc;

use parking_lot::RwLock;
use re_types_core::reflection::ComponentReflectionMap;
use re_viewer_context::{
    AppOptions, ComponentUiRegistry, FallbackProviderRegistry, SharedReflection, ViewClassRegistry,
};

/// What the viewer knows about views and components, and how to show them.
///
/// Embedders extend these before the first frame; see [`super::AppCore::add_view_class`].
pub struct Registries {
    /// All known view types.
    pub view_classes: ViewClassRegistry,

    /// How to show and edit each component in the UI.
    pub component_uis: ComponentUiRegistry,

    /// Default values for components that are not logged.
    pub component_fallbacks: FallbackProviderRegistry,

    /// Type metadata for archetypes, components, and views.
    pub reflection: SharedReflection,
}

impl Registries {
    /// The built-in registries.
    ///
    /// Also returns a snapshot of the component reflection, for the blueprint validator.
    pub fn new(app_options: &AppOptions) -> (Self, Arc<ComponentReflectionMap>) {
        let mut reflection = re_sdk_types::reflection::reflection().clone();
        if let Err(err) =
            re_lenses::register_heuristic_mappings(&mut reflection.component_identifiers)
        {
            re_log::error!("Failed to register heuristic mappings: {err}");
        }

        // `Reflection::components` is never mutated after this point (unlike the archetype
        // reflection, which embedders can extend), so a snapshot is safe to share.
        let component_reflection = Arc::new(reflection.components.clone());
        let reflection = Arc::new(RwLock::new(reflection));

        let mut component_fallbacks = re_component_fallbacks::create_component_fallback_registry();

        let view_classes = crate::default_views::create_view_class_registry(
            &reflection,
            app_options,
            &mut component_fallbacks,
        )
        .unwrap_or_else(|err| {
            re_log::error!("Failed to create view class registry: {err}");
            Default::default()
        });

        let mut component_uis = re_component_ui::create_component_ui_registry();
        re_data_ui::register_component_uis(&mut component_uis);

        let registries = Self {
            view_classes,
            component_uis,
            component_fallbacks,
            reflection,
        };
        (registries, component_reflection)
    }
}
