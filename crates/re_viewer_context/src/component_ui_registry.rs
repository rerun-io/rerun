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
    /// [`re_ui::list_item::ListItem`] with [`re_ui::list_item::PropertyContent`]. Text should
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

impl UiLayout {
    /// Build an egui table and configure it for the given UI layout.
    ///
    /// Note that the caller is responsible for strictly limiting the number of displayed rows for
    /// [`Self::List`] and [`Self::Tooltip`], as the table will not scroll.
    pub fn table(self, ui: &mut egui::Ui) -> egui_extras::TableBuilder<'_> {
        let table = egui_extras::TableBuilder::new(ui);
        match self {
            Self::List | Self::Tooltip => {
                // Be as small as possible in the hover tooltips. No scrolling related configuration, as
                // the content itself must be limited (scrolling is not possible in tooltips).
                table.auto_shrink([true, true])
            }
            Self::SelectionPanelLimitHeight => {
                // Don't take too much vertical space to leave room for other selected items.
                table
                    .auto_shrink([false, true])
                    .vscroll(true)
                    .max_scroll_height(100.0)
            }
            Self::SelectionPanelFull => {
                // We're alone in the selection panel. Let the outer ScrollArea do the work.
                table.auto_shrink([false, true]).vscroll(false)
            }
        }
    }

    /// Show a label while respecting the given UI layout.
    ///
    /// Important: for label only, data should use [`crate::data_label_for_ui_layout`] instead.
    // TODO(#6315): must be merged with `Self::data_label` and have an improved API
    pub fn label(self, ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
        let mut label = egui::Label::new(text);

        match self {
            Self::List => label = label.truncate(true),
            Self::Tooltip | Self::SelectionPanelLimitHeight | Self::SelectionPanelFull => {
                label = label.wrap(true);
            }
        }

        ui.add(label)
    }

    /// Show data while respecting the given UI layout.
    ///
    /// Import: for data only, labels should use [`crate::label_for_ui_layout`] instead.
    // TODO(#6315): must be merged with `Self::label` and have an improved API
    pub fn data_label(self, ui: &mut egui::Ui, string: impl AsRef<str>) {
        let string = string.as_ref();
        let font_id = egui::TextStyle::Monospace.resolve(ui.style());
        let color = ui.visuals().text_color();
        let wrap_width = ui.available_width();
        let mut layout_job =
            egui::text::LayoutJob::simple(string.to_owned(), font_id, color, wrap_width);

        let mut needs_scroll_area = false;

        match self {
            Self::List => {
                // Elide
                layout_job.wrap.max_rows = 1;
                layout_job.wrap.break_anywhere = true;
            }
            Self::Tooltip => {
                layout_job.wrap.max_rows = 3;
            }
            Self::SelectionPanelLimitHeight => {
                let num_newlines = string.chars().filter(|&c| c == '\n').count();
                needs_scroll_area = 10 < num_newlines || 300 < string.len();
            }
            Self::SelectionPanelFull => {
                needs_scroll_area = false;
            }
        }

        let galley = ui.fonts(|f| f.layout_job(layout_job)); // We control the text layout; not the label

        if needs_scroll_area {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(galley);
            });
        } else {
            ui.label(galley);
        }
    }
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
        component_name: &ComponentName,
        instance: &Instance,
    ) {
        re_tracing::profile_function!(component_name.full_name());

        if let Some((_, edit_callback)) = self.component_editors.get(component_name) {
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
