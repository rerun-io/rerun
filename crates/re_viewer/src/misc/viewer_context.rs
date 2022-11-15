use macaw::Ray3;

use re_data_store::{log_db::LogDb, InstanceId, ObjTypePath};
use re_log_types::{DataPath, MsgId, ObjPath, TimeInt, Timeline};

use crate::ui::SpaceViewId;

/// Common things needed by many parts of the viewer.
pub(crate) struct ViewerContext<'a> {
    /// Global options for the whole viewer.
    #[allow(unused)] // only used with 'glow' feature
    pub options: &'a mut Options,

    /// Things that need caching.
    pub cache: &'a mut Caches,

    /// The current recording
    pub log_db: &'a LogDb,

    /// UI config for the current recording (found in [`LogDb`]).
    pub rec_cfg: &'a mut RecordingConfig,

    /// The look and feel of the UI
    pub design_tokens: &'a crate::design_tokens::DesignTokens,

    #[cfg(feature = "wgpu")]
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
        let response = ui.selectable_label(self.rec_cfg.selection.is_type_path(type_path), text);
        if response.clicked() {
            self.rec_cfg.selection = Selection::ObjTypePath(type_path.clone());
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
        let response = ui.selectable_label(self.rec_cfg.selection.is_obj_path(obj_path), text);
        if response.clicked() {
            self.rec_cfg.selection = Selection::Instance(InstanceId {
                obj_path: obj_path.clone(),
                instance_index: None,
            });
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
        let response =
            ui.selectable_label(self.rec_cfg.selection.is_instance_id(instance_id), text);
        if response.clicked() {
            self.rec_cfg.selection = Selection::Instance(instance_id.clone());
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
        let response = ui.selectable_label(self.rec_cfg.selection.is_data_path(data_path), text);
        if response.clicked() {
            self.rec_cfg.selection = Selection::DataPath(data_path.clone());
        }
        response
    }

    pub fn space_view_button_to(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<egui::WidgetText>,
        space_view_id: SpaceViewId,
    ) -> egui::Response {
        let is_selected = self.rec_cfg.selection == Selection::SpaceView(space_view_id);
        let response = ui.selectable_label(is_selected, text);
        if response.clicked() {
            self.rec_cfg.selection = Selection::SpaceView(space_view_id);
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

        let response = ui.selectable_label(is_selected, timeline.name().as_str());
        if response.clicked() {
            self.rec_cfg.time_ctrl.set_timeline(*timeline);
            self.rec_cfg.time_ctrl.pause();
        }
        response
    }

    pub fn random_color(&mut self, obj_path: &ObjPath) -> [u8; 3] {
        self.cache.random_color(obj_path)
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Default, Debug, PartialEq)]
pub enum HoveredSpace {
    #[default]
    None,
    /// Hovering in a 2D space.
    TwoD {
        space_2d: Option<ObjPath>,
        /// Where in this 2D space (+ depth)?
        pos: glam::Vec3,
    },
    /// Hovering in a 3D space.
    #[allow(unused)] // only used with 'glow' feature
    ThreeD {
        /// The 3D space with the camera(s)
        space_3d: Option<ObjPath>,

        /// 2D spaces and pixel coordinates (with Z=depth)
        target_spaces: Vec<(ObjPath, Option<Ray3>, Option<glam::Vec3>)>,
    },
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`LogDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: crate::TimeControl,

    /// Currently selected thing; shown in the [`crate::selection_panel::SelectionPanel`].
    pub selection: Selection,

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
}

// ----------------------------------------------------------------------------

#[derive(Default)]
pub(crate) struct Caches {
    /// For displaying images efficiently in immediate mode.
    pub image: crate::misc::ImageCache,

    /// For displaying meshes efficiently in immediate mode.
    pub cpu_mesh: crate::ui::view_3d::CpuMeshCache,

    /// Auto-generated colors.
    object_colors: nohash_hasher::IntMap<u64, [u8; 3]>,
}

impl Caches {
    /// Call once per frame to potentially flush the cache(s).
    pub fn new_frame(&mut self) {
        let max_image_cache_use = 1_000_000_000;
        self.image.new_frame(max_image_cache_use);
    }

    pub fn random_color(&mut self, obj_path: &ObjPath) -> [u8; 3] {
        // TODO(emilk): ignore "temporary" indices when calculating the hash.
        let hash = obj_path.hash64();

        let color = *self
            .object_colors
            .entry(hash)
            .or_insert_with(|| crate::misc::random_rgb(hash));

        color
    }

    pub fn prune_memory(&mut self) {
        // image cache already self-prunes.
        // cpu_mesh cache _shouldn't_ fill up.
        self.object_colors.clear();
    }
}

// ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Options {
    pub show_camera_mesh_in_3d: bool,
    pub show_camera_axes_in_3d: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_camera_mesh_in_3d: true,
            show_camera_axes_in_3d: true,
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Selection {
    None,
    MsgId(MsgId),
    ObjTypePath(ObjTypePath),
    Instance(InstanceId),
    DataPath(DataPath),
    Space(ObjPath),
    SpaceView(crate::ui::SpaceViewId),
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
