use std::collections::BTreeMap;

use re_data_store::{DataStore, LatestAtQuery};
use re_entity_db::EntityPath;
use re_log_types::DataCell;
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
            &DataStore,
            &EntityPath,
            &ComponentWithInstances,
            &InstanceKey,
        ) + Send
        + Sync,
>;

type ComponentEditCallback = Box<
    dyn Fn(
            &ViewerContext<'_>,
            &mut egui::Ui,
            UiVerbosity,
            &LatestAtQuery,
            &DataStore,
            &EntityPath,
            &EntityPath,
            &ComponentWithInstances,
            &InstanceKey,
        ) + Send
        + Sync,
>;

type DefaultValueCallback = Box<
    dyn Fn(&ViewerContext<'_>, &LatestAtQuery, &DataStore, &EntityPath) -> DataCell + Send + Sync,
>;

/// How to display components in a Ui.
pub struct ComponentUiRegistry {
    /// Ui method to use if there was no specific one registered for a component.
    fallback_ui: ComponentUiCallback,
    component_uis: BTreeMap<ComponentName, ComponentUiCallback>,
    component_editors: BTreeMap<ComponentName, (DefaultValueCallback, ComponentEditCallback)>,
}

impl ComponentUiRegistry {
    pub fn new(fallback_ui: ComponentUiCallback) -> Self {
        Self {
            fallback_ui,
            component_uis: Default::default(),
            component_editors: Default::default(),
        }
    }

    /// Registers how to show a given component in the ui.
    ///
    /// If the component was already registered, the new callback replaces the old one.
    pub fn add(&mut self, name: ComponentName, callback: ComponentUiCallback) {
        self.component_uis.insert(name, callback);
    }

    /// Registers how to edit a given component in the ui.
    ///
    /// Requires two callbacks: one to provided an initial default value, and one to show the editor
    /// UI and save the updated value.
    ///
    /// If the component was already registered, the new callback replaces the old one.
    pub fn add_editor(
        &mut self,
        name: ComponentName,
        default_value: DefaultValueCallback,
        editor_callback: ComponentEditCallback,
    ) {
        self.component_editors
            .insert(name, (default_value, editor_callback));
    }

    /// Check if there is a registered editor for a given component
    pub fn has_registered_editor(&self, name: &ComponentName) -> bool {
        self.component_editors.contains_key(name)
    }

    /// Show a ui for this instance of this component.
    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &LatestAtQuery,
        store: &DataStore,
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
            .component_uis
            .get(&component.name())
            .unwrap_or(&self.fallback_ui);
        (*ui_callback)(
            ctx,
            ui,
            verbosity,
            query,
            store,
            entity_path,
            component,
            instance_key,
        );
    }

    /// Show an editor for this instance of this component.
    #[allow(clippy::too_many_arguments)]
    pub fn edit_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &LatestAtQuery,
        store: &DataStore,
        entity_path: &EntityPath,
        override_path: &EntityPath,
        component: &ComponentWithInstances,
        instance_key: &InstanceKey,
    ) {
        re_tracing::profile_function!(component.name().full_name());

        if let Some((_, edit_callback)) = self.component_editors.get(&component.name()) {
            (*edit_callback)(
                ctx,
                ui,
                verbosity,
                query,
                store,
                entity_path,
                override_path,
                component,
                instance_key,
            );
        } else {
            // Even if we can't edit the component, it's still helpful to show what the value is.
            self.ui(
                ctx,
                ui,
                verbosity,
                query,
                store,
                entity_path,
                component,
                instance_key,
            );
        }
    }

    /// Return a default value for this component.
    #[allow(clippy::too_many_arguments)]
    pub fn default_value(
        &self,
        ctx: &ViewerContext<'_>,
        query: &LatestAtQuery,
        store: &DataStore,
        entity_path: &EntityPath,
        component: &ComponentName,
    ) -> Option<DataCell> {
        re_tracing::profile_function!(component);

        self.component_editors
            .get(component)
            .map(|(default_value, _)| (*default_value)(ctx, query, store, entity_path))
    }
}
