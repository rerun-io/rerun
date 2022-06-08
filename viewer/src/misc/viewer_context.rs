use log_types::{DataPath, DataPathComponent, LogId, TimeSource, TimeValue};

use crate::{log_db::LogDb, misc::log_db::DataTree};

/// Common things needed by many parts of the viewer.
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct ViewerContext {
    /// For displaying images efficiently in immediate mode.
    #[serde(skip)]
    pub image_cache: crate::misc::ImageCache,

    /// For displaying meshes efficiently in immediate mode.
    #[serde(skip)]
    pub cpu_mesh_cache: crate::ui::view3d::CpuMeshCache,

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

    /// cached auto-generated colors.
    object_colors: nohash_hasher::IntMap<u64, [u8; 3]>,

    pub options: Options,
}

impl ViewerContext {
    /// Called at the start of each frame
    pub fn on_frame_start(&mut self, log_db: &LogDb) {
        crate::profile_function!();

        fn project_tree(
            context: &mut ViewerContext,
            path: &mut Vec<DataPathComponent>,
            prop: ObjectProps,
            tree: &DataTree,
        ) {
            // TODO: we need to speed up and simplify this a lot.
            let data_path = DataPath::new(path.clone());
            let prop = prop.with_child(&context.individual_object_properties.get(&data_path));
            context.projected_object_properties.set(data_path, prop);

            for (name, child) in &tree.string_children {
                // leafs such as "color" and "radius" doesn't have properties.
                // only objects do.
                if !child.is_leaf() {
                    path.push(DataPathComponent::String(*name));
                    project_tree(context, path, prop, child);
                    path.pop();
                }
            }
            for (index, child) in &tree.index_children {
                if !child.is_leaf() {
                    path.push(DataPathComponent::Index(index.clone()));
                    project_tree(context, path, prop, child);
                    path.pop();
                }
            }
        }

        let mut path = vec![];
        project_tree(self, &mut path, ObjectProps::default(), &log_db.data_tree);
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
        // TODO: common hover-effect of all buttons for the same data_path!
        let response = ui.selectable_label(self.selection.is_data_path(data_path), text);
        if response.clicked() {
            self.selection = Selection::ObjectPath(data_path.clone());
        }
        response
    }

    /// Button to select the current space.
    pub fn space_button(&mut self, ui: &mut egui::Ui, space: &DataPath) -> egui::Response {
        // TODO: common hover-effect of all buttons for the same space!
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
        value: TimeValue,
    ) -> egui::Response {
        let is_selected = self.time_control.is_time_selected(time_source, value);

        let response = ui.selectable_label(is_selected, value.to_string());
        if response.clicked() {
            self.time_control
                .set_source_and_time(time_source.clone(), value);
            self.time_control.pause();
        }
        response
    }

    pub fn random_color(&mut self, hash: u64) -> [u8; 3] {
        let color = *self
            .object_colors
            .entry(hash)
            .or_insert_with(|| crate::misc::random_rgb(hash));
        color
    }
}

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
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

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) enum Selection {
    None,
    LogId(LogId),
    ObjectPath(DataPath),
    Space(DataPath),
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

    pub fn is_data_path(&self, needle: &DataPath) -> bool {
        if let Self::ObjectPath(hay) = self {
            hay == needle
        } else {
            false
        }
    }

    pub fn is_space(&self, needle: &DataPath) -> bool {
        if let Self::Space(hay) = self {
            hay == needle
        } else {
            false
        }
    }
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ObjectsProperties {
    props: nohash_hasher::IntMap<DataPath, ObjectProps>,
}

impl ObjectsProperties {
    pub fn get(&self, data_path: &DataPath) -> ObjectProps {
        self.props.get(data_path).copied().unwrap_or_default()
    }

    pub fn set(&mut self, data_path: DataPath, prop: ObjectProps) {
        if prop == ObjectProps::default() {
            self.props.remove(&data_path); // save space
        } else {
            self.props.insert(data_path, prop);
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
