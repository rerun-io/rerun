use std::collections::BTreeMap;

use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtComponentResults, EntityDb, EntityPath};
use re_log::ResultExt;
use re_log_types::Instance;
use re_types::{
    external::arrow2::{self},
    ComponentName,
};
use re_ui::UiExt as _;

use crate::{ComponentFallbackProvider, QueryContext, ViewerContext};

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
    /// Important: for label only, data should use [`UiLayout::data_label`] instead.
    // TODO(#6315): must be merged with `Self::data_label` and have an improved API
    pub fn label(self, ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
        let mut label = egui::Label::new(text);

        match self {
            Self::List => label = label.truncate(),
            Self::Tooltip | Self::SelectionPanelLimitHeight | Self::SelectionPanelFull => {
                label = label.wrap();
            }
        }

        ui.add(label)
    }

    /// Show data while respecting the given UI layout.
    ///
    /// Import: for data only, labels should use [`UiLayout::data_label`] instead.
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

bitflags::bitflags! {
    /// Specifies which UI callbacks are available for a component.
    #[derive(PartialEq, Eq, Debug, Copy, Clone)]
    pub struct ComponentUiTypes: u8 {
        /// Display the component in a read-only way.
        const DisplayUi = 0b0000001;

        /// Edit the component in a single [`re_ui::list_item::ListItem`] line.
        const SingleLineEditor = 0b0000010;

        /// Edit the component over multiple [`re_ui::list_item::ListItem`]s.
        const MultiLineEditor = 0b0000100;
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

/// Callback for editing a component via ui.
///
/// Draws a ui showing the current value and allows the user to edit it.
/// If any edit was made, should return `Some` with the updated value.
/// If no edit was made, should return `None`.
type UntypedComponentEditCallback = Box<
    dyn Fn(
            &ViewerContext<'_>,
            &mut egui::Ui,
            &dyn arrow2::array::Array,
        ) -> Option<Box<dyn arrow2::array::Array>>
        + Send
        + Sync,
>;

/// How to display components in a Ui.
pub struct ComponentUiRegistry {
    /// Ui method to use if there was no specific one registered for a component.
    fallback_ui: ComponentUiCallback,

    component_uis: BTreeMap<ComponentName, ComponentUiCallback>,
    component_singleline_editors: BTreeMap<ComponentName, UntypedComponentEditCallback>,
    component_multiline_editors: BTreeMap<ComponentName, UntypedComponentEditCallback>,
}

impl ComponentUiRegistry {
    pub fn new(fallback_ui: ComponentUiCallback) -> Self {
        Self {
            fallback_ui,
            component_uis: Default::default(),
            component_singleline_editors: Default::default(),
            component_multiline_editors: Default::default(),
        }
    }

    /// Registers how to show a given component in the ui.
    ///
    /// If the component has already a display ui registered, the new callback replaces the old one.
    pub fn add_display_ui(&mut self, name: ComponentName, callback: ComponentUiCallback) {
        self.component_uis.insert(name, callback);
    }

    /// Registers how to edit a given component in the ui in a single line.
    ///
    /// If the component has already a single- or multiline editor registered respectively,
    /// the new callback replaces the old one.
    /// Prefer [`ComponentUiRegistry::add_singleline_editor_ui`] whenever possible
    pub fn add_untyped_editor_ui(
        &mut self,
        name: ComponentName,
        editor_callback: UntypedComponentEditCallback,
        multiline: bool,
    ) {
        if multiline {
            &mut self.component_multiline_editors
        } else {
            &mut self.component_singleline_editors
        }
        .insert(name, editor_callback);
    }

    /// Registers how to edit a given component in the ui.
    ///
    /// If the component has already a multi line editor registered, the new callback replaces the old one.
    /// Prefer [`ComponentUiRegistry::add_multiline_editor_ui`] whenever possible
    pub fn add_untyped_multiline_editor_ui(
        &mut self,
        name: ComponentName,
        editor_callback: UntypedComponentEditCallback,
    ) {
        self.component_multiline_editors
            .insert(name, editor_callback);
    }

    fn add_typed_editor_ui<C: re_types::Component>(
        &mut self,
        editor_callback: impl Fn(&ViewerContext<'_>, &mut egui::Ui, &mut C) -> egui::Response
            + Send
            + Sync
            + 'static,
        multiline: bool,
    ) {
        let untyped_callback: UntypedComponentEditCallback =
            Box::new(move |ui, ui_layout, value| {
                try_deserialize(value).and_then(|mut deserialized_value| {
                    editor_callback(ui, ui_layout, &mut deserialized_value)
                        .changed()
                        .then(|| {
                            use re_types::LoggableBatch;
                            deserialized_value.to_arrow().ok_or_log_error_once()
                        })
                        .flatten()
                })
            });

        self.add_untyped_editor_ui(C::name(), untyped_callback, multiline);
    }

    /// Registers how to edit a given component in the ui in a single list item line.
    ///
    /// If the component already has a singleline editor registered, the new callback replaces the old one.
    ///
    /// Typed editors do not handle absence of a value as well as lists of values and will be skipped in these cases.
    /// (This means that there must always be at least a fallback value available.)
    ///
    /// The value is only updated if the editor callback returns a `egui::Response::changed`.
    /// On the flip side, this means that even if the data has not changed it may be written back to the store.
    /// This can be relevant for transitioning from a fallback or default value to a custom value even if they are equal.
    ///
    /// Design principles for writing editors:
    /// * This is the value column function for a [`re_ui::list_item::PropertyContent`], behave accordingly!
    ///     * Unless you introduce hierarchy yourself, use [`re_ui::list_item::ListItem::show_flat`].
    /// * Don't show a tooltip, this is solved at a higher level.
    /// * Try not to assume context of the component beyond its inherent semantics
    ///   (e.g. if you get a `Color` you can't assume whether it's a background color or a point color)
    /// * The returned [`egui::Response`] should be for the widget that has the tooltip, not any pop-up content.
    ///     * Make sure that changes are propagated via [`egui::Response::mark_changed`] if necessary.
    pub fn add_singleline_editor_ui<C: re_types::Component>(
        &mut self,
        editor_callback: impl Fn(&ViewerContext<'_>, &mut egui::Ui, &mut C) -> egui::Response
            + Send
            + Sync
            + 'static,
    ) {
        let multiline = false;
        self.add_typed_editor_ui(editor_callback, multiline);
    }

    /// Registers how to edit a given component in the ui with multiple list items.
    ///
    /// If the component already has a singleline editor registered, the new callback replaces the old one.
    ///
    /// Typed editors do not handle absence of a value as well as lists of values and will be skipped in these cases.
    /// (This means that there must always be at least a fallback value available.)
    ///
    /// The value is only updated if the editor callback returns a `egui::Response::changed`.
    /// On the flip side, this means that even if the data has not changed it may be written back to the store.
    /// This can be relevant for transitioning from a fallback or default value to a custom value even if they are equal.
    ///
    /// Design principles for writing editors:
    /// * This is the content function for hierarchical [`re_ui::list_item::ListItem`], behave accordingly!
    /// * Try not to assume context of the component beyond its inherent semantics
    ///   (e.g. if you get a `Color` you can't assume whether it's a background color or a point color)
    /// * The returned [`egui::Response`] should be for the widget that has the tooltip, not any pop-up content.
    ///     * Make sure that changes are propagated via [`egui::Response::mark_changed`] if necessary.
    pub fn add_multiline_editor_ui<C: re_types::Component>(
        &mut self,
        editor_callback: impl Fn(&ViewerContext<'_>, &mut egui::Ui, &mut C) -> egui::Response
            + Send
            + Sync
            + 'static,
    ) {
        let multiline = true;
        self.add_typed_editor_ui(editor_callback, multiline);
    }

    /// Queries which ui types are registered for a component.
    ///
    /// Note that there's always a fallback display ui.
    pub fn registered_ui_types(&self, name: ComponentName) -> ComponentUiTypes {
        let mut types = ComponentUiTypes::empty();

        if self.component_uis.contains_key(&name) {
            types |= ComponentUiTypes::DisplayUi;
        }
        if self.component_singleline_editors.contains_key(&name) {
            types |= ComponentUiTypes::SingleLineEditor;
        }
        if self.component_multiline_editors.contains_key(&name) {
            types |= ComponentUiTypes::MultiLineEditor;
        }

        types
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

    /// Show a multi-line editor for this instance of this component.
    ///
    /// Changes will be written to the blueprint store at the given override path.
    /// Any change is expected to be effective next frame and passed in via the `component_query_result` parameter.
    /// (Otherwise, this method is agnostic to where the component data is stored.)
    #[allow(clippy::too_many_arguments)]
    pub fn multiline_edit_ui(
        &self,
        ctx: &QueryContext<'_>,
        ui: &mut egui::Ui,
        origin_db: &EntityDb,
        blueprint_write_path: &EntityPath,
        component_name: ComponentName,
        component_query_result: &LatestAtComponentResults,
        fallback_provider: &dyn ComponentFallbackProvider,
    ) {
        let multiline = true;
        self.edit_ui(
            ctx,
            ui,
            origin_db,
            blueprint_write_path,
            component_name,
            component_query_result,
            fallback_provider,
            multiline,
        );
    }

    /// Show a single-line editor for this instance of this component.
    ///
    /// Changes will be written to the blueprint store at the given override path.
    /// Any change is expected to be effective next frame and passed in via the `component_query_result` parameter.
    /// (Otherwise, this method is agnostic to where the component data is stored.)
    #[allow(clippy::too_many_arguments)]
    pub fn singleline_edit_ui(
        &self,
        ctx: &QueryContext<'_>,
        ui: &mut egui::Ui,
        origin_db: &EntityDb,
        blueprint_write_path: &EntityPath,
        component_name: ComponentName,
        component_query_result: &LatestAtComponentResults,
        fallback_provider: &dyn ComponentFallbackProvider,
    ) {
        let multiline = false;
        self.edit_ui(
            ctx,
            ui,
            origin_db,
            blueprint_write_path,
            component_name,
            component_query_result,
            fallback_provider,
            multiline,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn edit_ui(
        &self,
        ctx: &QueryContext<'_>,
        ui: &mut egui::Ui,
        origin_db: &EntityDb,
        blueprint_write_path: &EntityPath,
        component_name: ComponentName,
        component_query_result: &LatestAtComponentResults,
        fallback_provider: &dyn ComponentFallbackProvider,
        multiline: bool,
    ) {
        re_tracing::profile_function!(component_name.full_name());

        // TODO(andreas, jleibs): Editors only show & edit the first instance of a component batch.
        let instance: Instance = 0.into();

        let editors = if multiline {
            &self.component_multiline_editors
        } else {
            &self.component_singleline_editors
        };

        if let Some(edit_callback) = editors.get(&component_name) {
            let component_value_or_fallback = match component_value_or_fallback(
                ctx,
                component_query_result,
                component_name,
                instance,
                origin_db.resolver(),
                fallback_provider,
            ) {
                Ok(value) => value,
                Err(error_text) => {
                    re_log::error_once!("{error_text}");
                    ui.error_label(&error_text);
                    return;
                }
            };

            if let Some(updated) = (*edit_callback)(
                ctx.view_ctx.viewer_ctx,
                ui,
                component_value_or_fallback.as_ref(),
            ) {
                ctx.view_ctx.viewer_ctx.save_blueprint_data_cell(
                    blueprint_write_path,
                    re_log_types::DataCell::from_arrow(component_name, updated),
                );
            }
        } else {
            // Even if we can't edit the component, it's still helpful to show what the value is.
            self.ui(
                ctx.view_ctx.viewer_ctx,
                ui,
                UiLayout::List,
                ctx.query,
                origin_db,
                ctx.target_entity_path,
                component_query_result,
                &instance,
            );
        }
    }
}

fn component_value_or_fallback(
    ctx: &QueryContext<'_>,
    component_query_result: &LatestAtComponentResults,
    component_name: ComponentName,
    instance: Instance,
    resolver: &re_query::PromiseResolver,
    fallback_provider: &dyn ComponentFallbackProvider,
) -> Result<Box<dyn arrow2::array::Array>, String> {
    match component_query_result.resolved(resolver) {
        re_query::PromiseResult::Pending => {
            if component_query_result.num_instances() == 0 {
                // This can currently also happen when there's no data at all.
                None
            } else {
                // In the future, we might want to show a loading indicator here,
                // but right now this is always an error.
                return Err(format!("Promise for {component_name} is still pending."));
            }
        }
        re_query::PromiseResult::Ready(cell) => {
            let index = instance.get();
            if cell.num_instances() > index as u32 {
                Some(cell.as_arrow_ref().sliced(index as usize, 1))
            } else {
                None
            }
        }
        re_query::PromiseResult::Error(err) => {
            return Err(format!("Couldn't get {component_name}: {err}"));
        }
    }
    .map_or_else(
        || {
            fallback_provider
                .fallback_for(ctx, component_name)
                .map_err(|_err| format!("No fallback value available for {component_name}."))
        },
        Ok,
    )
}

fn try_deserialize<C: re_types::Component>(value: &dyn arrow2::array::Array) -> Option<C> {
    let component_name = C::name();
    let deserialized = C::from_arrow(value);
    match deserialized {
        Ok(values) => {
            if values.len() > 1 {
                // Whatever we did prior to calling this should have taken care if it!
                re_log::error_once!(
                    "Can only edit a single value at a time, got {} values for editing {component_name}",
                    values.len()
                );
            }
            if let Some(v) = values.into_iter().next() {
                Some(v)
            } else {
                re_log::warn_once!(
                    "Editor ui for {component_name} needs a start value to operate on."
                );
                None
            }
        }
        Err(err) => {
            re_log::error_once!("Failed to deserialize component of type {component_name}: {err}",);
            None
        }
    }
}
