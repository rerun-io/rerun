use std::collections::BTreeMap;

use re_data_store::LatestAtQuery;
use re_entity_db::{external::re_query::LatestAtComponentResults, EntityDb, EntityPath};
use re_log::ResultExt;
use re_log_types::Instance;
use re_types::{external::arrow2, ComponentName};

use crate::{ComponentFallbackProvider, ComponentFallbackResult, QueryContext, ViewerContext};

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
    component_editors: BTreeMap<ComponentName, UntypedComponentEditCallback>,
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
    pub fn add_untyped_editor(
        &mut self,
        name: ComponentName,
        editor_callback: UntypedComponentEditCallback,
    ) {
        self.component_editors.insert(name, editor_callback);
    }

    /// Registers how to edit a given component in the ui.
    ///
    /// If the component was already registered, the new callback replaces the old one.
    ///
    /// Typed editors do not handle absence of a value as well as lists of values and will be skipped in these cases.
    /// (This means that there must always be at least a fallback value available.)
    ///
    /// The value is only updated if the editor callback returns a `egui::Response::changed`.
    /// On the flip side, this means that even if the data has not changed it may be written back to the store.
    /// This can be relevant for transitioning from a fallback or default value to a custom value even if they are equal.
    ///
    /// Design principles for writing editors:
    /// * Don't show a tooltip, this is solved at a higher level.
    /// * Try not to assume context of the component beyond its inherent semantics
    ///   (e.g. if you get a `Color` you can't assume whether it's a background color or a point color)
    ///
    /// TODO(andreas): Implement handling for ui elements that are expandable (e.g. 2d bounds is too complex for a single line).
    pub fn add_editor<C: re_types::Component>(
        &mut self,
        editor_callback: impl Fn(&ViewerContext<'_>, &mut egui::Ui, &mut C) -> egui::Response
            + Send
            + Sync
            + 'static,
    ) {
        let untyped_callback: UntypedComponentEditCallback =
            Box::new(move |ui, ui_layout, value| {
                let deserialized = C::from_arrow(value);
                let mut deserialized_value = match deserialized {
                    Ok(values) => {
                        if values.len() > 1 {
                            // Whatever we did prior to calling this should have taken care if it!
                            re_log::error_once!(
                            "Can only edit a single value at a time, got {} values for editing {}",
                            values.len(),
                            C::name()
                        );
                        }
                        if let Some(v) = values.into_iter().next() {
                            v
                        } else {
                            re_log::warn_once!(
                                "Editor ui for {} needs a start value to operate on.",
                                C::name()
                            );
                            return None;
                        }
                    }
                    Err(err) => {
                        re_log::error_once!(
                            "Failed to deserialize component of type {}: {:?}",
                            C::name(),
                            err
                        );
                        return None;
                    }
                };

                editor_callback(ui, ui_layout, &mut deserialized_value)
                    .changed()
                    .then(|| {
                        use re_types::LoggableBatch;
                        deserialized_value.to_arrow().ok_or_log_error_once()
                    })
                    .flatten()
            });

        self.add_untyped_editor(C::name(), untyped_callback);
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
    ///
    /// Changes will be written to the blueprint store at the given override path.
    /// Any change is expected to be effective next frame and passed in via the `component_query_result` parameter.
    /// (Otherwise, this method is agnostic to where the component data is stored.)
    #[allow(clippy::too_many_arguments)]
    pub fn edit_ui(
        &self,
        ctx: &QueryContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        origin_db: &EntityDb,
        entity_path: &EntityPath,
        blueprint_override_path: &EntityPath,
        component_name: ComponentName,
        component_query_result: &LatestAtComponentResults,
        fallback_provider: &dyn ComponentFallbackProvider,
    ) {
        re_tracing::profile_function!(component_name.full_name());

        // TODO(andreas, jleibs): Editors only show & edit the first instance of a component batch.
        let instance: Instance = 0.into();

        if let Some(edit_callback) = self.component_editors.get(&component_name) {
            let component_query_result = match component_query_result.resolved(origin_db.resolver())
            {
                re_query::PromiseResult::Pending => {
                    ui.label("Loading component data...");
                    return;
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
                    let error_text = re_error::format_ref(err.as_ref());
                    re_log::error_once!("Couldn't get {component_name}: {error_text}");
                    ui.label(ctx.viewer_ctx.re_ui.error_text("Error"))
                        .on_hover_text(error_text);
                    return;
                }
            };

            let component_value_or_fallback = component_query_result.unwrap_or_else(|| {
                match fallback_provider.fallback_value(ctx, component_name) {
                    ComponentFallbackResult::Value(value) => value,
                    ComponentFallbackResult::SerializationError(err) => {
                        re_log::error_once!(
                            "Failed to deserialize component of type {}: {:?}",
                            component_name,
                            err
                        );
                        empty_arrow_component_array(origin_db, component_name)
                    }
                    ComponentFallbackResult::ComponentNotHandled => {
                        empty_arrow_component_array(origin_db, component_name)
                    }
                }
            });

            if let Some(updated) =
                (*edit_callback)(ctx.viewer_ctx, ui, component_value_or_fallback.as_ref())
            {
                ctx.viewer_ctx.save_blueprint_data_cell(
                    blueprint_override_path,
                    re_log_types::DataCell::from_arrow(component_name, updated),
                );
            }
        } else {
            // Even if we can't edit the component, it's still helpful to show what the value is.
            self.ui(
                ctx.viewer_ctx,
                ui,
                ui_layout,
                ctx.query,
                origin_db,
                entity_path,
                component_query_result,
                &instance,
            );
        }
    }
}

fn empty_arrow_component_array(
    origin_db: &EntityDb,
    component_name: ComponentName,
) -> Box<dyn arrow2::array::Array> {
    let datatype = origin_db
        .data_store()
        .lookup_datatype(&component_name)
        .cloned()
        .unwrap_or_else(|| {
            re_log::error!("Unknown component type {component_name}");
            arrow2::datatypes::DataType::Null
        });

    arrow2::array::new_empty_array(datatype)
}
