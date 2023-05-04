use re_data_store::{log_db::LogDb, InstancePath};
use re_log_types::{ComponentPath, EntityPath, TimeInt, Timeline};
use re_viewer_context::{
    DataBlueprintGroupHandle, HoverHighlight, Item, ItemCollection, SelectionState, SpaceViewId,
};

use crate::ui::{
    data_ui::{ComponentUiRegistry, DataUi},
    UiVerbosity,
};

/// Common things needed by many parts of the viewer.
pub struct ViewerContext<'a> {
    /// Global options for the whole viewer.
    pub app_options: &'a mut super::AppOptions,

    /// Things that need caching.
    pub cache: &'a mut super::Caches,

    /// How to display components
    pub component_ui_registry: &'a ComponentUiRegistry,

    /// The current recording
    pub log_db: &'a LogDb,

    /// UI config for the current recording (found in [`LogDb`]).
    pub rec_cfg: &'a mut RecordingConfig,

    /// The look and feel of the UI
    pub re_ui: &'a re_ui::ReUi,

    pub render_ctx: &'a mut re_renderer::RenderContext,
}

impl<'a> ViewerContext<'a> {
    /// Show an entity path and make it selectable.
    pub fn entity_path_button(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        entity_path: &EntityPath,
    ) -> egui::Response {
        self.instance_path_button_to(
            ui,
            space_view_id,
            &InstancePath::entity_splat(entity_path.clone()),
            entity_path.to_string(),
        )
    }

    /// Show an entity path and make it selectable.
    pub fn entity_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        entity_path: &EntityPath,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        self.instance_path_button_to(
            ui,
            space_view_id,
            &InstancePath::entity_splat(entity_path.clone()),
            text,
        )
    }

    /// Show an instance id and make it selectable.
    pub fn instance_path_button(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        instance_path: &InstancePath,
    ) -> egui::Response {
        self.instance_path_button_to(ui, space_view_id, instance_path, instance_path.to_string())
    }

    /// Show an instance id and make it selectable.
    pub fn instance_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        instance_path: &InstancePath,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        let item = Item::InstancePath(space_view_id, instance_path.clone());
        let subtype_string = if instance_path.instance_key.is_splat() {
            "Entity"
        } else {
            "Entity Instance"
        };

        let response = ui
            .selectable_label(self.selection().contains(&item), text)
            .on_hover_ui(|ui| {
                ui.strong(subtype_string);
                ui.label(format!("Path: {instance_path}"));
                instance_path.data_ui(
                    self,
                    ui,
                    crate::ui::UiVerbosity::Reduced,
                    &self.current_query(),
                );
            });

        self.cursor_interact_with_selectable(response, item)
    }

    /// Show a component path and make it selectable.
    pub fn component_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        component_path: &ComponentPath,
    ) -> egui::Response {
        let item = Item::ComponentPath(component_path.clone());
        let response = ui.selectable_label(self.selection().contains(&item), text);
        self.cursor_interact_with_selectable(response, item)
    }

    pub fn space_view_button(
        &mut self,
        ui: &mut egui::Ui,
        space_view: &crate::ui::SpaceView,
    ) -> egui::Response {
        self.space_view_button_to(
            ui,
            space_view.display_name.clone(),
            space_view.id,
            space_view.category,
        )
    }

    pub fn space_view_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        space_view_category: crate::ui::ViewCategory,
    ) -> egui::Response {
        let item = Item::SpaceView(space_view_id);
        let is_selected = self.selection().contains(&item);

        let response = self
            .re_ui
            .selectable_label_with_icon(ui, space_view_category.icon(), text, is_selected)
            .on_hover_text("Space View");
        self.cursor_interact_with_selectable(response, item)
    }

    pub fn data_blueprint_group_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        group_handle: DataBlueprintGroupHandle,
    ) -> egui::Response {
        let item = Item::DataBlueprintGroup(space_view_id, group_handle);
        let response = self
            .re_ui
            .selectable_label_with_icon(
                ui,
                &re_ui::icons::CONTAINER,
                text,
                self.selection().contains(&item),
            )
            .on_hover_text("Group");
        self.cursor_interact_with_selectable(response, item)
    }

    pub fn data_blueprint_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        entity_path: &EntityPath,
    ) -> egui::Response {
        let item = Item::InstancePath(
            Some(space_view_id),
            InstancePath::entity_splat(entity_path.clone()),
        );
        let response = ui
            .selectable_label(self.selection().contains(&item), text)
            .on_hover_ui(|ui| {
                ui.strong("Space View Entity");
                ui.label(format!("Path: {entity_path}"));
                entity_path.data_ui(self, ui, UiVerbosity::Reduced, &self.current_query());
            });
        self.cursor_interact_with_selectable(response, item)
    }

    pub fn time_button(
        &mut self,
        ui: &mut egui::Ui,
        timeline: &Timeline,
        value: TimeInt,
    ) -> egui::Response {
        let is_selected = self.rec_cfg.time_ctrl.is_time_selected(timeline, value);

        let response = ui.selectable_label(is_selected, timeline.typ().format(value));
        if response.clicked() {
            self.rec_cfg
                .time_ctrl
                .set_timeline_and_time(*timeline, value);
            self.rec_cfg.time_ctrl.pause();
        }
        response
    }

    pub fn timeline_button(&mut self, ui: &mut egui::Ui, timeline: &Timeline) -> egui::Response {
        self.timeline_button_to(ui, timeline.name().to_string(), timeline)
    }

    pub fn timeline_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        timeline: &Timeline,
    ) -> egui::Response {
        let is_selected = self.rec_cfg.time_ctrl.timeline() == timeline;

        let response = ui
            .selectable_label(is_selected, text)
            .on_hover_text("Click to switch to this timeline");
        if response.clicked() {
            self.rec_cfg.time_ctrl.set_timeline(*timeline);
            self.rec_cfg.time_ctrl.pause();
        }
        response
    }

    // ---------------------------------------------------------
    // shortcuts for common selection/hover manipulation

    /// Sets a single selection, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_single_selection(&mut self, item: Item) -> ItemCollection {
        self.rec_cfg.selection_state.set_single_selection(item)
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_multi_selection(&mut self, items: impl Iterator<Item = Item>) -> ItemCollection {
        self.rec_cfg.selection_state.set_multi_selection(items)
    }

    /// Selects (or toggles selection if modifier is clicked) currently hovered elements on click.
    pub fn select_hovered_on_click(&mut self, response: &egui::Response) {
        if response.clicked() {
            let hovered = self.rec_cfg.selection_state.hovered().clone();
            if response.ctx.input(|i| i.modifiers.command) {
                self.rec_cfg
                    .selection_state
                    .toggle_selection(hovered.to_vec());
            } else {
                self.set_multi_selection(hovered.into_iter());
            }
        }
    }

    pub fn cursor_interact_with_selectable(
        &mut self,
        response: egui::Response,
        item: Item,
    ) -> egui::Response {
        let is_item_hovered =
            self.selection_state().highlight_for_ui_element(&item) == HoverHighlight::Hovered;

        if response.hovered() {
            self.rec_cfg
                .selection_state
                .set_hovered(std::iter::once(item));
        }
        self.select_hovered_on_click(&response);
        // TODO(andreas): How to deal with shift click for selecting ranges?

        if is_item_hovered {
            response.highlight()
        } else {
            response
        }
    }

    /// Returns the current selection.
    pub fn selection(&self) -> &ItemCollection {
        self.rec_cfg.selection_state.current()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &ItemCollection {
        self.rec_cfg.selection_state.hovered()
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered(&mut self, hovered: impl Iterator<Item = Item>) {
        self.rec_cfg.selection_state.set_hovered(hovered);
    }

    pub fn selection_state(&self) -> &SelectionState {
        &self.rec_cfg.selection_state
    }

    pub fn selection_state_mut(&mut self) -> &mut SelectionState {
        &mut self.rec_cfg.selection_state
    }

    /// The current time query, based on the current time control.
    pub fn current_query(&self) -> re_arrow_store::LatestAtQuery {
        self.rec_cfg.time_ctrl.current_query()
    }
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`LogDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: crate::TimeControl,

    /// Selection & hovering state.
    pub selection_state: SelectionState,
}
