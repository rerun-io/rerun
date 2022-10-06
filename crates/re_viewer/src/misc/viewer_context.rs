use macaw::Ray3;

use re_data_store::{
    log_db::{LogDb, ObjectTree},
    InstanceId, ObjTypePath,
};
use re_log_types::{DataPath, MsgId, ObjPath, ObjPathComp, TimeInt, TimeSource};

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

    /// Show an typeect path and make it selectable.
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

    /// Button to select the current space.
    pub fn space_button(&mut self, ui: &mut egui::Ui, space: &ObjPath) -> egui::Response {
        // TODO(emilk): common hover-effect of all buttons for the same space!
        let response =
            ui.selectable_label(self.rec_cfg.selection.is_space(space), space.to_string());
        if response.clicked() {
            self.rec_cfg.selection = Selection::Space(space.clone());
        }
        response
    }

    pub fn time_button(
        &mut self,
        ui: &mut egui::Ui,
        time_source: &TimeSource,
        value: TimeInt,
    ) -> egui::Response {
        let is_selected = self
            .rec_cfg
            .time_ctrl
            .is_time_selected(time_source, value.into());

        let response = ui.selectable_label(is_selected, time_source.typ().format(value));
        if response.clicked() {
            self.rec_cfg
                .time_ctrl
                .set_source_and_time(*time_source, value);
            self.rec_cfg.time_ctrl.pause();
        }
        response
    }

    pub fn random_color<Time>(
        &mut self,
        props: &re_data_store::InstanceProps<'_, Time>,
    ) -> [u8; 3] {
        // TODO(emilk): ignore "temporary" indices when calculating the hash.
        let hash = props.obj_path.hash64();

        let color = *self
            .cache
            .object_colors
            .entry(hash)
            .or_insert_with(|| crate::misc::random_rgb(hash));
        color
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

impl HoveredSpace {
    pub fn space(&self) -> Option<&ObjPath> {
        match self {
            Self::None => None,
            Self::TwoD { space_2d, .. } => space_2d.as_ref(),
            Self::ThreeD { space_3d, .. } => space_3d.as_ref(),
        }
    }
}

// ----------------------------------------------------------------------------

/// UI config for the current recording (found in [`LogDb`]).
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct RecordingConfig {
    /// The current time of the time panel, how fast it is moving, etc.
    pub time_ctrl: crate::TimeControl,

    /// Currently selected thing; shown in the context menu.
    pub selection: Selection,

    /// Individual settings. Mutate this.
    pub individual_object_properties: ObjectsProperties,

    /// Properties, as inherited from parent. Read from this.
    ///
    /// Recalculated at the start of each frame form [`Self::individual_object_properties`].
    #[serde(skip)]
    pub projected_object_properties: ObjectsProperties,

    /// So we only re-calculate `projected_object_properties` when it changes.
    individual_object_properties_last_frame: ObjectsProperties,

    /// What space is the pointer hovering over?
    #[serde(skip)]
    pub hovered_space: HoveredSpace,
}

impl RecordingConfig {
    /// Called at the start of each frame
    pub fn on_frame_start(&mut self, log_db: &LogDb) {
        crate::profile_function!();

        self.project_object_properties(log_db);
    }

    fn project_object_properties(&mut self, log_db: &LogDb) {
        crate::profile_function!();

        if self.individual_object_properties == self.individual_object_properties_last_frame {
            // when we have objects with a lot of children (e.g. a batch of points),
            // the project gets slow, so this memoization is important.
            return;
        }
        self.individual_object_properties_last_frame = self.individual_object_properties.clone();

        fn project_tree(
            rec_cfg: &mut RecordingConfig,
            path: &mut Vec<ObjPathComp>,
            prop: ObjectProps,
            tree: &ObjectTree,
        ) {
            let obj_path = ObjPath::from(path.clone());
            let prop = prop.with_child(&rec_cfg.individual_object_properties.get(&obj_path));
            rec_cfg.projected_object_properties.set(obj_path, prop);

            for (name, child) in &tree.named_children {
                path.push(ObjPathComp::Name(*name));
                project_tree(rec_cfg, path, prop, child);
                path.pop();
            }
            for (index, child) in &tree.index_children {
                path.push(ObjPathComp::Index(index.clone()));
                project_tree(rec_cfg, path, prop, child);
                path.pop();
            }
        }

        let mut path = vec![];
        project_tree(self, &mut path, ObjectProps::default(), &log_db.data_tree);
    }
}

// ----------------------------------------------------------------------------

#[derive(Default)]
pub(crate) struct Caches {
    /// For displaying images efficiently in immediate mode.
    pub image: crate::misc::ImageCache,

    /// For displaying meshes efficiently in immediate mode.
    #[cfg(feature = "glow")]
    pub cpu_mesh: crate::ui::view3d::CpuMeshCache,

    /// Auto-generated colors.
    object_colors: nohash_hasher::IntMap<u64, [u8; 3]>,
}

impl Caches {
    /// Call once per frame to potentially flush the cache(s).
    pub fn new_frame(&mut self) {
        let max_image_cache_use = 1_000_000_000;
        self.image.new_frame(max_image_cache_use);
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

    pub fn is_space(&self, needle: &ObjPath) -> bool {
        if let Self::Space(hay) = self {
            hay == needle
        } else {
            false
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) struct ObjectsProperties {
    props: nohash_hasher::IntMap<ObjPath, ObjectProps>,
}

impl ObjectsProperties {
    pub fn get(&self, obj_path: &ObjPath) -> ObjectProps {
        self.props.get(obj_path).copied().unwrap_or_default()
    }

    pub fn set(&mut self, obj_path: ObjPath, prop: ObjectProps) {
        if prop == ObjectProps::default() {
            self.props.remove(&obj_path); // save space
        } else {
            self.props.insert(obj_path, prop);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ObjectProps {
    pub visible: bool,
}

impl Default for ObjectProps {
    fn default() -> Self {
        Self { visible: true }
    }
}

impl ObjectProps {
    /// Multiply/and these together.
    fn with_child(&self, child: &ObjectProps) -> ObjectProps {
        ObjectProps {
            visible: self.visible && child.visible,
        }
    }
}
