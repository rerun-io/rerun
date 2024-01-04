use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_entity_db::EntityPath;
use re_query::ComponentWithInstances;
use re_types::{components::InstanceKey, ComponentName, Loggable as _};

use crate::ViewerContext;

/// Controls how mich space we use to show the data in a component ui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiVerbosity {
    /// Keep it small enough to fit on one row.
    Small,

    /// Display a reduced set, used for hovering.
    ///
    /// Keep it under a half-dozen lines.
    Reduced,

    /// Display everything as wide as available but limit height.
    ///
    /// This is used for example in the selection panel when multiple items are selected. When using
    /// a Table, use the `re_data_ui::table_for_verbosity` function.
    LimitHeight,

    /// Display everything as wide as available, without height restrictions.
    ///
    /// This is used for example in the selection panel when only one item is selected. In this
    /// case, any scrolling is handled by the selection panel itself. When using a Table, use the
    /// `re_data_ui::table_for_verbosity` function.
    Full,
}

type ComponentUiCallback = Box<
    dyn Fn(
            &ViewerContext<'_>,
            &mut egui::Ui,
            UiVerbosity,
            &LatestAtQuery,
            &EntityPath,
            &ComponentWithInstances,
            &InstanceKey,
        ) + Send
        + Sync,
>;

/// How to display components in a Ui.
pub struct ComponentUiRegistry {
    /// Ui method to use if there was no specific one registered for a component.
    fallback_ui: ComponentUiCallback,
    components: BTreeMap<ComponentName, ComponentUiCallback>,
}

impl ComponentUiRegistry {
    pub fn new(fallback_ui: ComponentUiCallback) -> Self {
        Self {
            fallback_ui,
            components: Default::default(),
        }
    }

    /// Registers how to show a given component in the ui.
    ///
    /// If the component was already registered, the new callback replaces the old one.
    pub fn add(&mut self, name: ComponentName, callback: ComponentUiCallback) {
        self.components.insert(name, callback);
    }

    /// Show a ui for this instance of this component.
    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component: &ComponentWithInstances,
        instance_key: &InstanceKey,
    ) {
        re_tracing::profile_function!(component.name().full_name());

        if component.name() == InstanceKey::name() {
            // The user wants to show a ui for the `InstanceKey` component - well, that's easy:
            ui.label(instance_key.to_string());
            return;
        }

        let ui_callback = self
            .components
            .get(&component.name())
            .unwrap_or(&self.fallback_ui);
        (*ui_callback)(
            ctx,
            ui,
            verbosity,
            query,
            entity_path,
            component,
            instance_key,
        );
    }
}
