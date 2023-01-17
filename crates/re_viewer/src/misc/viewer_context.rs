use re_data_store::{log_db::LogDb, InstanceId, ObjTypePath};
use re_log_types::{DataPath, MsgId, ObjPath, TimeInt, Timeline};

use crate::ui::{
    data_ui::DataUi, DataBlueprintGroupHandle, Preview, SelectionHistory, SpaceViewId,
};

/// Common things needed by many parts of the viewer.
pub struct ViewerContext<'a> {
    /// Global options for the whole viewer.
    pub options: &'a mut Options,

    /// Things that need caching.
    pub cache: &'a mut super::Caches,

    /// The current recording
    pub log_db: &'a LogDb,

    /// UI config for the current recording (found in [`LogDb`]).
    pub rec_cfg: &'a mut RecordingConfig,

    pub selection_history: &'a mut SelectionHistory,

    /// The look and feel of the UI
    pub re_ui: &'a re_ui::ReUi,

    pub render_ctx: &'a mut re_renderer::RenderContext,
}

impl<'a> ViewerContext<'a> {
    /// Show a type path and make it selectable.
    pub fn type_path_button(
        &mut self,
        ui: &mut egui::Ui,
        type_path: &ObjTypePath,
    ) -> egui::Response {
        self.type_path_button_to(ui, type_path.to_string(), type_path)
    }

    /// Show a type path and make it selectable.
    pub fn type_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        type_path: &ObjTypePath,
    ) -> egui::Response {
        // TODO(emilk): common hover-effect of all buttons for the same type_path!
        let response = ui.selectable_label(self.selection().is_type_path(type_path), text);
        if response.clicked() {
            self.set_selection(Selection::ObjTypePath(type_path.clone()));
        }
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
        // TODO(emilk): common hover-effect of all buttons for the same obj_path!
        let response = ui
            .selectable_label(self.selection().is_obj_path(obj_path), text)
            .on_hover_ui(|ui| {
                ui.strong("Object");
                ui.label(format!("Path: {obj_path}"));
                obj_path.data_ui(self, ui, crate::ui::Preview::Medium);
            });
        if response.clicked() {
            self.set_selection(Selection::Instance(InstanceId {
                obj_path: obj_path.clone(),
                instance_index: None,
            }));
        }
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
            .selectable_label(self.selection().is_instance_id(instance_id), text)
            .on_hover_ui(|ui| {
                ui.strong("Object Instance");
                ui.label(format!("Path: {instance_id}"));
                instance_id.data_ui(self, ui, crate::ui::Preview::Medium);
            });
        if response.clicked() {
            self.set_selection(Selection::Instance(instance_id.clone()));
        }
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
        let response = ui.selectable_label(self.selection().is_data_path(data_path), text);
        if response.clicked() {
            self.set_selection(Selection::DataPath(data_path.clone()));
        }
        response
    }

    pub fn space_view_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
    ) -> egui::Response {
        let is_selected = self.selection() == Selection::SpaceView(space_view_id);
        let response = ui
            .selectable_label(is_selected, text)
            .on_hover_text("Space View");
        if response.clicked() {
            self.set_selection(Selection::SpaceView(space_view_id));
        }
        response
    }

    pub fn datablueprint_group_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        group_handle: DataBlueprintGroupHandle,
    ) -> egui::Response {
        let selection = Selection::DataBlueprintGroup(space_view_id, group_handle);
        let is_selected = self.selection() == selection;
        let response = ui
            .selectable_label(is_selected, text)
            .on_hover_text("Group");
        if response.clicked() {
            self.set_selection(selection);
        }
        response
    }

    pub fn space_view_obj_path_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
        obj_path: &ObjPath,
    ) -> egui::Response {
        let selection = Selection::SpaceViewObjPath(space_view_id, obj_path.clone());
        let response = ui
            .selectable_label(self.selection() == selection, text)
            .on_hover_ui(|ui| {
                ui.strong("Space View Object");
                ui.label(format!("Path: {obj_path}"));
                obj_path.data_ui(self, ui, Preview::Medium);
            });
        if response.clicked() {
            self.set_selection(selection);
        }
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
        let is_selected = self.rec_cfg.time_ctrl.timeline() == timeline;

        let response = ui
            .selectable_label(is_selected, timeline.name().as_str())
            .on_hover_text("Click to switch to this timeline");
        if response.clicked() {
            self.rec_cfg.time_ctrl.set_timeline(*timeline);
            self.rec_cfg.time_ctrl.pause();
        }
        response
    }

    /// Sets the current selection, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_selection(&mut self, selection: Selection) -> Selection {
        self.rec_cfg
            .set_selection(self.selection_history, selection)
    }

    /// Clears the current selection.
    ///
    /// Returns the previous selection.
    pub fn clear_selection(&mut self) -> Selection {
        self.rec_cfg.clear_selection()
    }

    /// Returns the current selection.
    pub fn selection(&self) -> Selection {
        self.rec_cfg.selection()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Default, Debug, PartialEq)]
pub enum HoveredSpace {
    #[default]
    None,
    /// Hovering in a 2D space.
    TwoD {
        space_2d: ObjPath,
        /// Where in this 2D space (+ depth)?
        pos: glam::Vec3,
    },
    /// Hovering in a 3D space.
    ThreeD {
        /// The 3D space with the camera(s)
        space_3d: ObjPath,

        /// 2D spaces and pixel coordinates (with Z=depth)
        target_spaces: Vec<(ObjPath, Option<glam::Vec3>)>,
    },
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`LogDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: crate::TimeControl,

    /// Currently selected thing; shown in the [`crate::selection_panel::SelectionPanel`].
    ///
    /// Do not access this field directly! Use the helper methods instead, which will make sure
    /// to properly maintain the undo/redo history.
    selection: Selection,

    /// What space is the pointer hovering over? Read from this.
    #[serde(skip)]
    pub hovered_space_previous_frame: HoveredSpace,

    /// What space is the pointer hovering over? Write to this.
    #[serde(skip)]
    pub hovered_space_this_frame: HoveredSpace,
}

impl RecordingConfig {
    /// Called at the start of each frame
    pub fn on_frame_start(&mut self) {
        crate::profile_function!();

        self.hovered_space_previous_frame =
            std::mem::replace(&mut self.hovered_space_this_frame, HoveredSpace::None);
    }

    /// Sets the current selection, updating history as needed.
    ///
    /// Returns the previous selection.
    pub fn set_selection(
        &mut self,
        history: &mut SelectionHistory,
        selection: Selection,
    ) -> Selection {
        history.update_selection(&selection);
        std::mem::replace(&mut self.selection, selection)
    }

    /// Clears the current selection.
    ///
    /// Returns the previous selection.
    pub fn clear_selection(&mut self) -> Selection {
        // NOTE: at least for now, we consider a lack of selection irrelevant history-wise.
        std::mem::replace(&mut self.selection, Selection::None)
    }

    /// Returns the current selection.
    pub fn selection(&self) -> Selection {
        self.selection.clone()
    }
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Selection {
    None,
    MsgId(MsgId),
    ObjTypePath(ObjTypePath),
    Instance(InstanceId),
    DataPath(DataPath),
    SpaceView(crate::ui::SpaceViewId),
    /// An object within a space-view.
    SpaceViewObjPath(crate::ui::SpaceViewId, ObjPath),
    DataBlueprintGroup(crate::ui::SpaceViewId, crate::ui::DataBlueprintGroupHandle),
}

impl std::fmt::Display for Selection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Selection::None => write!(f, "<empty>"),
            Selection::MsgId(s) => s.fmt(f),
            Selection::ObjTypePath(s) => s.fmt(f),
            Selection::Instance(s) => s.fmt(f),
            Selection::DataPath(s) => s.fmt(f),
            Selection::SpaceView(s) => write!(f, "{s:?}"),
            Selection::SpaceViewObjPath(sid, path) => write!(f, "({sid:?}, {path})"),
            Selection::DataBlueprintGroup(sid, handle) => write!(f, "({sid:?}, {handle:?})"),
        }
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::None
    }
}

impl Selection {
    // pub fn is_none(&self) -> bool {
    //     matches!(self, Self::None)
    // }

    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_type_path(&self, needle: &ObjTypePath) -> bool {
        if let Self::ObjTypePath(hay) = self {
            hay == needle
        } else {
            false
        }
    }

    pub fn is_instance_id(&self, needle: &InstanceId) -> bool {
        if let Self::Instance(hay) = self {
            hay == needle
        } else {
            false
        }
    }

    pub fn is_obj_path(&self, needle: &ObjPath) -> bool {
        if let Self::Instance(hay) = self {
            &hay.obj_path == needle
        } else {
            false
        }
    }

    pub fn is_data_path(&self, needle: &DataPath) -> bool {
        if let Self::DataPath(hay) = self {
            hay == needle
        } else {
            false
        }
    }
}
