use re_data_store::{log_db::LogDb, InstanceId};
use re_log_types::{DataPath, MsgId, ObjPath, TimeInt, Timeline};

use crate::ui::{
    data_ui::{ComponentUiRegistry, DataUi},
    DataBlueprintGroupHandle, SpaceViewId, UiVerbosity,
};

use super::{
    selection::{MultiSelection, Selection},
    HoverHighlight,
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
    /// Show an [`MsgId`] and make it selectable
    pub fn msg_id_button(&mut self, ui: &mut egui::Ui, msg_id: MsgId) -> egui::Response {
        let selection = Selection::MsgId(msg_id);
        let response = ui
            .selectable_label(self.selection().contains(&selection), msg_id.to_string())
            .on_hover_ui(|ui| {
                ui.label(format!("Message ID: {msg_id}"));
                ui.separator();
                msg_id.data_ui(self, ui, UiVerbosity::Small, &self.current_query());
            });
        self.cursor_interact_with_selectable(response, selection)
    }

    /// Show an obj path and make it selectable.
    pub fn obj_path_button(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        obj_path: &ObjPath,
    ) -> egui::Response {
        self.instance_id_button_to(
            ui,
            space_view_id,
            &InstanceId::new(obj_path.clone(), None),
            obj_path.to_string(),
        )
    }

    /// Show an obj path and make it selectable.
    pub fn obj_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        obj_path: &ObjPath,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        self.instance_id_button_to(
            ui,
            space_view_id,
            &InstanceId::new(obj_path.clone(), None),
            text,
        )
    }

    /// Show an instance id and make it selectable.
    pub fn instance_id_button(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        instance_id: &InstanceId,
    ) -> egui::Response {
        self.instance_id_button_to(ui, space_view_id, instance_id, instance_id.to_string())
    }

    /// Show an instance id and make it selectable.
    pub fn instance_id_button_to(
        &mut self,
        ui: &mut egui::Ui,
        space_view_id: Option<SpaceViewId>,
        instance_id: &InstanceId,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        let selection = Selection::Instance(space_view_id, instance_id.clone());
        let subtype_string = match instance_id.instance_index {
            Some(_) => "Object Instance",
            None => "Object",
        };

        let response = ui
            .selectable_label(self.selection().contains(&selection), text)
            .on_hover_ui(|ui| {
                ui.strong(subtype_string);
                ui.label(format!("Path: {instance_id}"));
                instance_id.data_ui(
                    self,
                    ui,
                    crate::ui::UiVerbosity::Large,
                    &self.current_query(),
                );
            });

        self.cursor_interact_with_selectable(response, selection)
    }

    /// Show a data path and make it selectable.
    pub fn data_path_button(&mut self, ui: &mut egui::Ui, data_path: &DataPath) -> egui::Response {
        self.data_path_button_to(ui, data_path.to_string(), data_path)
    }

    /// Show a data path and make it selectable.
    pub fn data_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        data_path: &DataPath,
    ) -> egui::Response {
        let selection = Selection::DataPath(data_path.clone());
        let response = ui.selectable_label(self.selection().contains(&selection), text);
        self.cursor_interact_with_selectable(response, selection)
    }

    pub fn space_view_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
    ) -> egui::Response {
        let selection = Selection::SpaceView(space_view_id);
        let is_selected = self.selection().contains(&selection);
        let response = ui
            .selectable_label(is_selected, text)
            .on_hover_text("Space View");
        self.cursor_interact_with_selectable(response, selection)
    }

    pub fn data_blueprint_group_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        group_handle: DataBlueprintGroupHandle,
    ) -> egui::Response {
        let selection = Selection::DataBlueprintGroup(space_view_id, group_handle);
        let response = ui
            .selectable_label(self.selection().contains(&selection), text)
            .on_hover_text("Group");
        self.cursor_interact_with_selectable(response, selection)
    }

    pub fn data_blueprint_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        obj_path: &ObjPath,
    ) -> egui::Response {
        let selection =
            Selection::Instance(Some(space_view_id), InstanceId::new(obj_path.clone(), None));
        let response = ui
            .selectable_label(self.selection().contains(&selection), text)
            .on_hover_ui(|ui| {
                ui.strong("Space View Object");
                ui.label(format!("Path: {obj_path}"));
                obj_path.data_ui(self, ui, UiVerbosity::Large, &self.current_query());
            });
        self.cursor_interact_with_selectable(response, selection)
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
    pub fn set_single_selection(&mut self, item: Selection) -> MultiSelection {
        self.rec_cfg.selection_state.set_single_selection(item)
    }

    /// Sets several objects to be selected, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_multi_selection(
        &mut self,
        items: impl Iterator<Item = Selection>,
    ) -> MultiSelection {
        self.rec_cfg.selection_state.set_multi_selection(items)
    }

    /// Selects (or toggles selection if modifier is clicked) currently hovered elements on click.
    pub fn select_hovered_on_click(&mut self, response: &egui::Response) {
        if response.clicked() {
            let hovered = self.rec_cfg.selection_state.hovered().clone();
            if response.ctx.input(|i| i.modifiers.command) {
                self.rec_cfg
                    .selection_state
                    .toggle_selection(hovered.into_iter());
            } else {
                self.set_multi_selection(hovered.into_iter());
            }
        }
    }

    pub fn cursor_interact_with_selectable(
        &mut self,
        response: egui::Response,
        selectable: Selection,
    ) -> egui::Response {
        let is_item_hovered =
            self.selection_state().highlight_for_ui_element(&selectable) == HoverHighlight::Hovered;

        if response.hovered() {
            self.rec_cfg
                .selection_state
                .set_hovered(std::iter::once(selectable));
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
    pub fn selection(&self) -> &MultiSelection {
        self.rec_cfg.selection_state.current()
    }

    /// Returns the currently hovered objects.
    pub fn hovered(&self) -> &MultiSelection {
        self.rec_cfg.selection_state.hovered()
    }

    /// Set the hovered objects. Will be in [`Self::hovered`] on the next frame.
    pub fn set_hovered(&mut self, hovered_objects: impl Iterator<Item = Selection>) {
        self.rec_cfg.selection_state.set_hovered(hovered_objects);
    }

    pub fn selection_state(&self) -> &super::SelectionState {
        &self.rec_cfg.selection_state
    }

    pub fn selection_state_mut(&mut self) -> &mut super::SelectionState {
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
    pub selection_state: super::SelectionState,
}
