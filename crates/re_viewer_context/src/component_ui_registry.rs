use std::collections::BTreeMap;

use re_arrow_store::LatestAtQuery;
use re_data_store::EntityPath;
use re_log_types::{component_types::InstanceKey, external::arrow2, Component, ComponentName};
use re_query::ComponentWithInstances;

use crate::ViewerContext;

/// Controls how mich space we use to show the data in [`DataUi`].
#[derive(Clone, Copy, Debug)]
pub enum UiVerbosity {
    /// Keep it small enough to fit on one row.
    Small,

    /// Display a reduced set, used for hovering.
    Reduced,

    /// Display everything, as large as you want. Used for selection panel.
    All,
}

type ComponentUiCallback = Box<
    dyn Fn(
        &mut ViewerContext<'_>,
        &mut egui::Ui,
        UiVerbosity,
        &LatestAtQuery,
        &EntityPath,
        &ComponentWithInstances,
        &InstanceKey,
    ),
>;

/// How to display components in a Ui.
#[derive(Default)]
pub struct ComponentUiRegistry {
    components: BTreeMap<ComponentName, ComponentUiCallback>,
}

impl ComponentUiRegistry {
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
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        verbosity: UiVerbosity,
        query: &LatestAtQuery,
        entity_path: &EntityPath,
        component: &ComponentWithInstances,
        instance_key: &InstanceKey,
    ) {
        crate::profile_function!(component.name().full_name());

        if component.name() == InstanceKey::name() {
            // The user wants to show a ui for the `InstanceKey` component - well, that's easy:
            ui.label(instance_key.to_string());
            return;
        }

        if let Some(ui_callback) = self.components.get(&component.name()) {
            (*ui_callback)(
                ctx,
                ui,
                verbosity,
                query,
                entity_path,
                component,
                instance_key,
            );
        } else {
            // No special ui implementation - use a generic one:
            if let Some(value) = component.lookup_arrow(instance_key) {
                let bytes = arrow2::compute::aggregate::estimated_bytes_size(value.as_ref());
                if bytes < 256 {
                    // For small items, print them
                    let mut repr = String::new();
                    let display = arrow2::array::get_display(value.as_ref(), "null");
                    display(&mut repr, 0).unwrap();
                    ui.label(repr);
                } else {
                    ui.label(format!("{bytes} bytes"));
                }
            } else {
                ui.weak("(null)");
            }
        }
    }
}
