use std::collections::BTreeMap;

use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtComponentResults, EntityDb, EntityPath};
use re_log_types::{DataCell, Instance};
use re_types::ComponentName;

use crate::ViewerContext;

/// Specifies the context in which the UI is used and the constraints it should follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiLayout {
    /// Display a short summary. Used in lists.
    ///
    /// Keep it small enough to fit on half a row (i.e. the second column of a
    /// [`re_ui::list_item2::ListItem`] with [`re_ui::list_item2::PropertyContent`]. Text should
    /// truncate.
    List,

    /// Display as much information as possible in a compact way. Used for hovering/tooltips.
    ///
    /// Keep it under a half-dozen lines. Text may wrap. Avoid interactive UI. When using a table,
    /// use the `re_data_ui::table_for_ui_layout` function.
    Tooltip,

    /// Display everything as wide as available but limit height. Used in the selection panel when
    /// multiple items are selected.
    ///
    /// When displaying lists, wrap them in a height-limited [`egui::ScrollArea`]. When using a
    /// table, use the `re_data_ui::table_for_ui_layout` function.
    SelectionPanelLimitHeight,

    /// Display everything as wide as available, without height restriction. Used in the selection
    /// panel when a single item is selected.
    ///
    /// The UI will be wrapped in a [`egui::ScrollArea`], so data should be fully displayed with no
    /// restriction. When using a table, use the `re_data_ui::table_for_ui_layout` function.
    SelectionPanelFull,
}

type ComponentUiCallback = Box<
    dyn Fn(
            &ViewerContext<'_>,
            &mut egui::Ui,
            UiLayout,
            &LatestAtQuery,
            &EntityDb,
            &EntityPath,
            &LatestAtComponentResults,
            &Instance,
        ) + Send
        + Sync,
>;

type ComponentEditCallback = Box<
    dyn Fn(
            &ViewerContext<'_>,
            &mut egui::Ui,
            UiLayout,
            &LatestAtQuery,
            &EntityDb,
            &EntityPath,
            &EntityPath,
            &LatestAtComponentResults,
            &Instance,
        ) + Send
        + Sync,
>;

type DefaultValueCallback = Box<
    dyn Fn(&ViewerContext<'_>, &LatestAtQuery, &EntityDb, &EntityPath) -> DataCell + Send + Sync,
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
    /// Requires two callbacks: one to provide an initial default value, and one to show the editor
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
        ui_layout: UiLayout,
        query: &LatestAtQuery,
        db: &EntityDb,
        entity_path: &EntityPath,
        component: &LatestAtComponentResults,
        instance: &Instance,
    ) {
        let Some(component_name) = component.component_name(db.resolver()) else {
            // TODO(#5607): what should happen if the promise is still pending?
            return;
        };

        re_tracing::profile_function!(component_name.full_name());

        let ui_callback = self
            .component_uis
            .get(&component_name)
            .unwrap_or(&self.fallback_ui);
        (*ui_callback)(
            ctx,
            ui,
            ui_layout,
            query,
            db,
            entity_path,
            component,
            instance,
        );
    }

    /// Show an editor for this instance of this component.
    #[allow(clippy::too_many_arguments)]
    pub fn edit_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &LatestAtQuery,
        db: &EntityDb,
        entity_path: &EntityPath,
        override_path: &EntityPath,
        component: &LatestAtComponentResults,
        instance: &Instance,
    ) {
        let Some(component_name) = component.component_name(db.resolver()) else {
            // TODO(#5607): what should happen if the promise is still pending?
            return;
        };

        re_tracing::profile_function!(component_name.full_name());

        if let Some((_, edit_callback)) = self.component_editors.get(&component_name) {
            (*edit_callback)(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                entity_path,
                override_path,
                component,
                instance,
            );
        } else {
            // Even if we can't edit the component, it's still helpful to show what the value is.
            self.ui(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                entity_path,
                component,
                instance,
            );
        }
    }

    /// Return a default value for this component.
    #[allow(clippy::too_many_arguments)]
    pub fn default_value(
        &self,
        ctx: &ViewerContext<'_>,
        query: &LatestAtQuery,
        db: &EntityDb,
        entity_path: &EntityPath,
        component: &ComponentName,
    ) -> Option<DataCell> {
        re_tracing::profile_function!(component);

        self.component_editors
            .get(component)
            .map(|(default_value, _)| (*default_value)(ctx, query, db, entity_path))
    }
}
