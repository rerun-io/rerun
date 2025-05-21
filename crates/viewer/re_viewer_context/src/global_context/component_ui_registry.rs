use std::collections::BTreeMap;

use re_chunk::{RowId, TimePoint, UnitChunkShared};
use re_chunk_store::LatestAtQuery;
use re_entity_db::{EntityDb, EntityPath};
use re_log::ResultExt as _;
use re_log_types::{Instance, StoreId};
use re_types::{ComponentDescriptor, ComponentName};
use re_ui::{UiExt as _, UiLayout};

use crate::{ComponentFallbackProvider, MaybeMutRef, QueryContext, ViewerContext};

/// Describes where an edit should be written to if any
pub struct EditTarget {
    pub store_id: StoreId,
    pub timepoint: TimePoint,
    pub entity_path: EntityPath,
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

type LegacyDisplayComponentUiCallback = Box<
    dyn Fn(
            &ViewerContext<'_>,
            &mut egui::Ui,
            UiLayout,
            &LatestAtQuery,
            &EntityDb,
            &EntityPath,
            &ComponentDescriptor,
            Option<RowId>,
            &dyn arrow::array::Array,
        ) + Send
        + Sync,
>;

enum EditOrView {
    /// Allow the user to view and mutate the value
    Edit,

    /// No mutation allowed
    View,
}

/// Callback for viewing, and maybe editing, a component via UI.
///
/// Draws a UI showing the current value and allows the user to edit it.
/// If any edit was made, should return `Some` with the updated value.
/// If no edit was made, should return `None`.
type UntypedComponentEditOrViewCallback = Box<
    dyn Fn(
            &ViewerContext<'_>,
            &mut egui::Ui,
            &dyn arrow::array::Array,
            EditOrView,
        ) -> Option<arrow::array::ArrayRef>
        + Send
        + Sync,
>;

/// How to display components in a Ui.
pub struct ComponentUiRegistry {
    /// Older component uis - TODO(#6661): we're in the process of removing these.
    ///
    /// The main issue with these is that they take a lot of parameters:
    /// Not only does it make them more verbose to implement,
    /// it also makes them on overly flexible (they know a lot about the context of a component)
    /// on one hand and too inflexible on the other - these additional parameters are not always be meaningful in all contexts.
    /// -> They are unsuitable for interacting with blueprint overrides & defaults,
    /// as there are several entity paths associated with single component
    /// (the blueprint entity path where the component is stored and the entity path in the store that they apply to).
    ///
    /// Other issues:
    /// * duality of edit & view:
    ///   In this old system we didn't take into account that most types should also be editable in the UI.
    ///   This makes implementations of view & edit overly asymmetric when instead they are often rather similar.
    /// * unawareness of `ListItem` context:
    ///   We often want to display components as list items and in the older callbacks we don't know whether we're in a list item or not.
    legacy_display_component_uis: BTreeMap<ComponentName, LegacyDisplayComponentUiCallback>,

    /// Implements viewing and probably editing
    component_singleline_edit_or_view: BTreeMap<ComponentName, UntypedComponentEditOrViewCallback>,

    /// Implements viewing and probably editing
    component_multiline_edit_or_view: BTreeMap<ComponentName, UntypedComponentEditOrViewCallback>,
}

impl Default for ComponentUiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentUiRegistry {
    pub fn new() -> Self {
        Self {
            legacy_display_component_uis: Default::default(),
            component_singleline_edit_or_view: Default::default(),
            component_multiline_edit_or_view: Default::default(),
        }
    }

    /// Registers how to show a given component in the UI.
    ///
    /// If the component has already a display UI registered, the new callback replaces the old one.
    pub fn add_legacy_display_ui(
        &mut self,
        name: ComponentName,
        callback: LegacyDisplayComponentUiCallback,
    ) {
        self.legacy_display_component_uis.insert(name, callback);
    }

    /// Registers how to view, and maybe edit, a given component in the UI in a single list item line.
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
    pub fn add_singleline_edit_or_view<C: re_types::Component>(
        &mut self,
        callback: impl Fn(&ViewerContext<'_>, &mut egui::Ui, &mut MaybeMutRef<'_, C>) -> egui::Response
        + Send
        + Sync
        + 'static,
    ) {
        let multiline = false;
        self.add_editor_ui(multiline, callback);
    }

    /// Registers how to view, and maybe edit, a given component in the UI with multiple list items.
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
    pub fn add_multiline_edit_or_view<C: re_types::Component>(
        &mut self,
        callback: impl Fn(&ViewerContext<'_>, &mut egui::Ui, &mut MaybeMutRef<'_, C>) -> egui::Response
        + Send
        + Sync
        + 'static,
    ) {
        let multiline = true;
        self.add_editor_ui(multiline, callback);
    }

    fn add_editor_ui<C: re_types::Component>(
        &mut self,
        multiline: bool,
        callback: impl Fn(&ViewerContext<'_>, &mut egui::Ui, &mut MaybeMutRef<'_, C>) -> egui::Response
        + Send
        + Sync
        + 'static,
    ) {
        let untyped_callback: UntypedComponentEditOrViewCallback =
            Box::new(move |ui, ui_layout, value, edit_or_view| {
                try_deserialize(value).and_then(|mut deserialized_value| match edit_or_view {
                    EditOrView::View => {
                        callback(ui, ui_layout, &mut MaybeMutRef::Ref(&deserialized_value));
                        None
                    }
                    EditOrView::Edit => {
                        let response = callback(
                            ui,
                            ui_layout,
                            &mut MaybeMutRef::MutRef(&mut deserialized_value),
                        );

                        if response.changed() {
                            use re_types::LoggableBatch as _;
                            deserialized_value.to_arrow().ok_or_log_error_once()
                        } else {
                            None
                        }
                    }
                })
            });

        if multiline {
            &mut self.component_multiline_edit_or_view
        } else {
            &mut self.component_singleline_edit_or_view
        }
        .insert(C::name(), untyped_callback);
    }

    /// Queries which UI types are registered for a component.
    ///
    /// Note that there's always a fallback display UI.
    pub fn registered_ui_types(&self, name: ComponentName) -> ComponentUiTypes {
        let mut types = ComponentUiTypes::empty();

        if self.legacy_display_component_uis.contains_key(&name) {
            types |= ComponentUiTypes::DisplayUi;
        }
        if self.component_singleline_edit_or_view.contains_key(&name) {
            types |= ComponentUiTypes::DisplayUi | ComponentUiTypes::SingleLineEditor;
        }
        if self.component_multiline_edit_or_view.contains_key(&name) {
            types |= ComponentUiTypes::DisplayUi | ComponentUiTypes::MultiLineEditor;
        }

        types
    }

    /// Show a UI for a component instance.
    ///
    /// Has a fallback to show an info text if the instance is not specific,
    /// but in these cases `LatestAtComponentResults::data_ui` should be used instead!
    #[allow(clippy::too_many_arguments)]
    pub fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &LatestAtQuery,
        db: &EntityDb,
        entity_path: &EntityPath,
        component_descr: &ComponentDescriptor,
        unit: &UnitChunkShared,
        instance: &Instance,
    ) {
        // Don't use component.raw_instance here since we want to handle the case where there's several
        // elements differently.
        // Also, it allows us to slice the array without cloning any elements.
        let Some(array) = unit.component_batch_raw(component_descr) else {
            re_log::error_once!("Couldn't get {component_descr}: missing");
            ui.error_with_details_on_hover(format!("Couldn't get {component_descr}: missing"));
            return;
        };

        // Component UI can only show a single instance.
        if array.len() == 0 || (instance.is_all() && array.len() > 1) {
            fallback_ui(ui, ui_layout, array.as_ref());
            return;
        }

        let index = if instance.is_all() {
            // Per above check, there's a single instance, show it.
            0
        } else {
            instance.get() as usize
        };

        // Enforce clamp-to-border semantics.
        // TODO(andreas): Is that always what we want?
        let index = index.clamp(0, array.len().saturating_sub(1));
        let component_raw = array.slice(index, 1);

        self.ui_raw(
            ctx,
            ui,
            ui_layout,
            query,
            db,
            entity_path,
            component_descr,
            unit.row_id(),
            component_raw.as_ref(),
        );
    }

    /// Show a UI for a single raw component.
    #[allow(clippy::too_many_arguments)]
    pub fn ui_raw(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &LatestAtQuery,
        db: &EntityDb,
        entity_path: &EntityPath,
        component_descr: &ComponentDescriptor,
        row_id: Option<RowId>,
        component_raw: &dyn arrow::array::Array,
    ) {
        re_tracing::profile_function!(component_descr.display_name());

        if component_raw.len() != 1 {
            fallback_ui(ui, ui_layout, component_raw);
            return;
        }

        // Prefer the versatile UI callback if there is one.
        if let Some(ui_callback) = self
            .legacy_display_component_uis
            .get(&component_descr.component_name)
        {
            (*ui_callback)(
                ctx,
                ui,
                ui_layout,
                query,
                db,
                entity_path,
                component_descr,
                row_id,
                component_raw,
            );
            return;
        }

        // Fallback to the more specialized UI callbacks.
        let edit_or_view_ui = if ui_layout == UiLayout::SelectionPanel {
            self.component_multiline_edit_or_view
                .get(&component_descr.component_name)
                .or_else(|| {
                    self.component_singleline_edit_or_view
                        .get(&component_descr.component_name)
                })
        } else {
            self.component_singleline_edit_or_view
                .get(&component_descr.component_name)
        };
        if let Some(edit_or_view_ui) = edit_or_view_ui {
            // Use it in view mode (no mutation).
            (*edit_or_view_ui)(ctx, ui, component_raw, EditOrView::View);
            return;
        }

        fallback_ui(ui, ui_layout, component_raw);
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
        blueprint_write_path: EntityPath,
        component_descr: &ComponentDescriptor,
        row_id: Option<RowId>,
        component_array: Option<&dyn arrow::array::Array>,
        fallback_provider: &dyn ComponentFallbackProvider,
    ) {
        let multiline = true;
        self.edit_ui(
            ctx,
            ui,
            origin_db,
            blueprint_write_path,
            component_descr,
            row_id,
            component_array,
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
        blueprint_write_path: EntityPath,
        component_descr: &ComponentDescriptor,
        row_id: Option<RowId>,
        component_query_result: Option<&dyn arrow::array::Array>,
        fallback_provider: &dyn ComponentFallbackProvider,
    ) {
        let multiline = false;
        self.edit_ui(
            ctx,
            ui,
            origin_db,
            blueprint_write_path,
            component_descr,
            row_id,
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
        blueprint_write_path: EntityPath,
        component_descr: &ComponentDescriptor,
        row_id: Option<RowId>,
        component_array: Option<&dyn arrow::array::Array>,
        fallback_provider: &dyn ComponentFallbackProvider,
        allow_multiline: bool,
    ) {
        re_tracing::profile_function!(component_descr.display_name());

        let component_name_for_fallback = component_descr.component_name;

        let run_with = |array| {
            self.edit_ui_raw(
                ctx,
                ui,
                origin_db,
                blueprint_write_path,
                component_descr,
                row_id,
                array,
                allow_multiline,
            );
        };

        // Use a fallback if there's either no component data at all or the component array is empty.
        if let Some(component_array) = component_array.filter(|array| !array.is_empty()) {
            run_with(component_array);
        } else {
            let fallback = fallback_provider.fallback_for(ctx, component_name_for_fallback);
            run_with(fallback.as_ref());
        }
    }

    /// For blueprint editing
    #[allow(clippy::too_many_arguments)]
    pub fn edit_ui_raw(
        &self,
        ctx: &QueryContext<'_>,
        ui: &mut egui::Ui,
        origin_db: &EntityDb,
        blueprint_write_path: EntityPath,
        component_descr: &ComponentDescriptor,
        row_id: Option<RowId>,
        component_raw: &dyn arrow::array::Array,
        allow_multiline: bool,
    ) {
        if !self.try_show_edit_ui(
            ctx.viewer_ctx,
            ui,
            EditTarget {
                store_id: ctx.viewer_ctx.store_context.blueprint.store_id().clone(),
                timepoint: ctx
                    .viewer_ctx
                    .store_context
                    .blueprint_timepoint_for_writes(),
                entity_path: blueprint_write_path,
            },
            component_raw,
            component_descr.clone(),
            allow_multiline,
        ) {
            // Even if we can't edit the component, it's still helpful to show what the value is.
            self.ui_raw(
                ctx.viewer_ctx,
                ui,
                UiLayout::List,
                ctx.query,
                origin_db,
                ctx.target_entity_path,
                component_descr,
                row_id,
                component_raw,
            );
        }
    }

    /// Tries to show a UI for editing a component.
    ///
    /// Returns `true` if the passed component is a single value and has a registered
    /// editor for multiline or singleline editing respectively.
    pub fn try_show_edit_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        target: EditTarget,
        raw_current_value: &dyn arrow::array::Array,
        component_descr: ComponentDescriptor,
        allow_multiline: bool,
    ) -> bool {
        re_tracing::profile_function!(component_descr.display_name());
        let EditTarget {
            store_id,
            timepoint,
            entity_path,
        } = target;

        // We use the component name to identify which UI to show.
        // (but for saving back edit results, we need the full descriptor)
        let ui_identifier = component_descr.component_name;

        if raw_current_value.len() != 1 {
            return false;
        }

        let edit_or_view = if allow_multiline {
            self.component_multiline_edit_or_view
                .get(&ui_identifier)
                .or_else(|| self.component_singleline_edit_or_view.get(&ui_identifier))
        } else {
            self.component_singleline_edit_or_view.get(&ui_identifier)
        };

        if let Some(edit_or_view) = edit_or_view {
            if let Some(updated) = (*edit_or_view)(ctx, ui, raw_current_value, EditOrView::Edit) {
                ctx.append_array_to_store(
                    store_id,
                    timepoint,
                    entity_path,
                    component_descr,
                    updated,
                );
            }
            return true;
        }

        false
    }
}

fn try_deserialize<C: re_types::Component>(value: &dyn arrow::array::Array) -> Option<C> {
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
                    "Editor UI for {component_name} needs a start value to operate on."
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

/// The ui we fall back to if everything else fails.
fn fallback_ui(ui: &mut egui::Ui, ui_layout: UiLayout, component: &dyn arrow::array::Array) {
    re_ui::arrow_ui(ui, ui_layout, component);
}
