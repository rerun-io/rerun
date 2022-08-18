use re_data_store::ObjTypePath;
use re_log_types::{
    DataPath, LogId, ObjPath, ObjPathBuilder, ObjPathComp, TimeInt, TimeSource, TimeValue,
};

use crate::{log_db::LogDb, misc::log_db::ObjectTree};

/// Common things needed by many parts of the viewer.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ViewerContext {
    #[serde(skip)]
    pub cache: Caches,

    /// The current time.
    pub time_control: crate::TimeControl,

    /// Currently selected thing, shown in the context menu.
    pub selection: Selection,

    /// Individual settings. Mutate this.
    pub individual_object_properties: ObjectsProperties,

    /// Properties, as inherited from parent.
    /// Read from this.
    ///
    /// Recalculated at the start of each frame form [`Self::individual_object_properties`].
    #[serde(skip)]
    pub projected_object_properties: ObjectsProperties,

    pub options: Options,
}

impl ViewerContext {
    /// Called at the start of each frame
    pub fn on_frame_start(&mut self, log_db: &LogDb) {
        crate::profile_function!();

        fn project_tree(
            context: &mut ViewerContext,
            path: &mut Vec<ObjPathComp>,
            prop: ObjectProps,
            tree: &ObjectTree,
        ) {
            // TODO(emilk): we need to speed up and simplify this a lot.
            let obj_path = ObjPath::from(ObjPathBuilder::new(path.clone()));
            let prop = prop.with_child(&context.individual_object_properties.get(&obj_path));
            context.projected_object_properties.set(obj_path, prop);

            for (name, child) in &tree.string_children {
                path.push(ObjPathComp::String(*name));
                project_tree(context, path, prop, child);
                path.pop();
            }
            for (index, child) in &tree.index_children {
                path.push(ObjPathComp::Index(index.clone()));
                project_tree(context, path, prop, child);
                path.pop();
            }
        }

        let mut path = vec![];
        project_tree(self, &mut path, ObjectProps::default(), &log_db.data_tree);
    }

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
        let response = ui.selectable_label(self.selection.is_type_path(type_path), text);
        if response.clicked() {
            self.selection = Selection::ObjTypePath(type_path.clone());
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
        let response = ui.selectable_label(self.selection.is_obj_path(obj_path), text);
        if response.clicked() {
            self.selection = Selection::ObjPath(obj_path.clone());
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
        let response = ui.selectable_label(self.selection.is_data_path(data_path), text);
        if response.clicked() {
            self.selection = Selection::DataPath(data_path.clone());
        }
        response
    }

    /// Button to select the current space.
    pub fn space_button(&mut self, ui: &mut egui::Ui, space: &ObjPath) -> egui::Response {
        // TODO(emilk): common hover-effect of all buttons for the same space!
        let response = ui.selectable_label(self.selection.is_space(space), space.to_string());
        if response.clicked() {
            self.selection = Selection::Space(space.clone());
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
            .time_control
            .is_time_selected(time_source, value.into());

        let response = ui.selectable_label(
            is_selected,
            TimeValue::new(time_source.typ(), value).to_string(),
        );
        if response.clicked() {
            self.time_control.set_source_and_time(*time_source, value);
            self.time_control.pause();
        }
        response
    }

    pub fn random_color(&mut self, props: &re_data_store::ObjectProps<'_>) -> [u8; 3] {
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

#[derive(Default)]
pub(crate) struct Caches {
    /// For displaying images efficiently in immediate mode.
    pub image: crate::misc::ImageCache,

    /// For displaying meshes efficiently in immediate mode.
    pub cpu_mesh: crate::ui::view3d::CpuMeshCache,

    /// Auto-generated colors.
    object_colors: nohash_hasher::IntMap<u64, [u8; 3]>,
}

// ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Options {
    pub show_camera_mesh_in_3d: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_camera_mesh_in_3d: true,
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Selection {
    None,
    LogId(LogId),
    ObjTypePath(ObjTypePath),
    ObjPath(ObjPath),
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

    pub fn is_obj_path(&self, needle: &ObjPath) -> bool {
        if let Self::ObjPath(hay) = self {
            hay == needle
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

#[derive(Default, serde::Deserialize, serde::Serialize)]
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
