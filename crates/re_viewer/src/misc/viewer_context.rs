use re_data_store::{log_db::LogDb, InstanceId};
use re_log_types::{DataPath, MsgId, ObjPath, TimeInt, Timeline};

use crate::ui::{
    data_ui::{ComponentUiRegistry, DataUi},
    DataBlueprintGroupHandle, Preview, SpaceViewId,
};

use super::selection::{MultiSelection, Selection};

/// Common things needed by many parts of the viewer.
pub struct ViewerContext<'a> {
    /// Global options for the whole viewer.
    pub options: &'a mut Options,

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
        // TODO(emilk): common hover-effect
        let response = ui
            .selectable_label(
                self.selection().check_msg_id(msg_id).is_exact(),
                msg_id.to_string(),
            )
            .on_hover_ui(|ui| {
                ui.label(format!("Message ID: {msg_id}"));
                ui.separator();
                msg_id.data_ui(self, ui, Preview::Small);
            });
        self.cursor_interact_with_selectable(&response, Selection::MsgId(msg_id));
        response
    }

    /// Show a obj path and make it selectable.
    pub fn obj_path_button(&mut self, ui: &mut egui::Ui, obj_path: &ObjPath) -> egui::Response {
        self.obj_path_button_to(ui, obj_path.to_string(), obj_path)
    }

    /// Show an object path and make it selectable.
    pub fn obj_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        obj_path: &ObjPath,
    ) -> egui::Response {
        let response = ui
            .selectable_label(
                self.selection().check_obj_path(obj_path.hash()).is_exact(),
                text,
            )
            .on_hover_ui(|ui| {
                ui.strong("Object");
                ui.label(format!("Path: {obj_path}"));
                obj_path.data_ui(self, ui, crate::ui::Preview::Large);
            });
        self.cursor_interact_with_selectable(
            &response,
            Selection::Instance(InstanceId {
                obj_path: obj_path.clone(),
                instance_index: None,
            }),
        );
        response
    }

    /// Show a instance id and make it selectable.
    pub fn instance_id_button(
        &mut self,
        ui: &mut egui::Ui,
        instance_id: &InstanceId,
    ) -> egui::Response {
        self.instance_id_button_to(ui, instance_id.to_string(), instance_id)
    }

    /// Show an instance id and make it selectable.
    pub fn instance_id_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        instance_id: &InstanceId,
    ) -> egui::Response {
        // TODO(emilk): common hover-effect of all buttons for the same instance_id!
        let response = ui
            .selectable_label(
                self.selection()
                    .check_instance(instance_id.hash())
                    .is_exact(),
                text,
            )
            .on_hover_ui(|ui| {
                ui.strong("Object Instance");
                ui.label(format!("Path: {instance_id}"));
                instance_id.data_ui(self, ui, crate::ui::Preview::Large);
            });

        self.cursor_interact_with_selectable(&response, Selection::Instance(instance_id.clone()));
        response
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
        // TODO(emilk): common hover-effect of all buttons for the same data_path!
        let response = ui.selectable_label(
            self.rec_cfg
                .selection_state
                .current()
                .check_data_path(data_path)
                .is_exact(),
            text,
        );
        self.cursor_interact_with_selectable(&response, Selection::DataPath(data_path.clone()));
        response
    }

    pub fn space_view_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
    ) -> egui::Response {
        let is_selected = self.selection().check_space_view(space_view_id).is_exact();
        let response = ui
            .selectable_label(is_selected, text)
            .on_hover_text("Space View");
        self.cursor_interact_with_selectable(&response, Selection::SpaceView(space_view_id));
        response
    }

    pub fn datablueprint_group_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        group_handle: DataBlueprintGroupHandle,
    ) -> egui::Response {
        let response = ui
            .selectable_label(
                self.selection()
                    .check_data_blueprint_group(space_view_id, group_handle)
                    .is_exact(),
                text,
            )
            .on_hover_text("Group");
        self.cursor_interact_with_selectable(
            &response,
            Selection::DataBlueprintGroup(space_view_id, group_handle),
        );
        response
    }

    pub fn space_view_obj_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        obj_path: &ObjPath,
    ) -> egui::Response {
        let selection = Selection::DataBlueprint(space_view_id, obj_path.clone());
        let response = ui
            .selectable_label(
                self.selection().check_obj_path(obj_path.hash()).is_exact(),
                text,
            )
            .on_hover_ui(|ui| {
                ui.strong("Space View Object");
                ui.label(format!("Path: {obj_path}"));
                obj_path.data_ui(self, ui, Preview::Large);
            });
        self.cursor_interact_with_selectable(&response, selection);
        response
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
            if response.ctx.input().modifiers.command {
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
        response: &egui::Response,
        selectable: Selection,
    ) {
        if response.hovered() {
            self.rec_cfg
                .selection_state
                .set_hovered(std::iter::once(selectable.clone()));
        }
        if response.clicked() {
            self.rec_cfg
                .selection_state
                .set_single_selection(selectable);
            // TODO(andreas): How to deal with shift click for selecting ranges?
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

// ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Options {
    pub show_camera_axes_in_3d: bool,

    pub low_latency: f32,
    pub warn_latency: f32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_camera_axes_in_3d: true,

            low_latency: 0.100,
            warn_latency: 0.200,
        }
    }
}

// ----------------------------------------------------------------------------
