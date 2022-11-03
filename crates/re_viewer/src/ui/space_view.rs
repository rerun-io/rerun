use ahash::HashSet;
use glam::Vec3;
use re_data_store::{
    InstanceIdHash, ObjPath, ObjectTree, ObjectTreeProperties, Objects, TimeQuery, Timeline,
};
use re_log_types::{MsgId, Transform};

use crate::misc::{space_info::*, ViewerContext};

use super::view3d::{scene::Size, SpaceCamera};

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum ViewCategory {
    TwoD,
    #[default]
    ThreeD,
    Tensor,
    Text,
}

// ----------------------------------------------------------------------------

/// A view of a space.
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct SpaceView {
    pub name: String,
    pub space_path: ObjPath,
    view_state: ViewState,

    /// In case we are a mix of 2d/3d/tensor/text, we show what?
    selected_category: ViewCategory,

    pub obj_tree_properties: ObjectTreeProperties,
}

impl SpaceView {
    pub fn from_path(space_path: ObjPath) -> Self {
        Self {
            name: space_path.to_string(),
            space_path,
            view_state: Default::default(),
            selected_category: Default::default(),
            obj_tree_properties: Default::default(),
        }
    }

    pub fn on_frame_start(&mut self, obj_tree: &ObjectTree) {
        self.obj_tree_properties.on_frame_start(obj_tree);
    }

    pub(crate) fn scene_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        scene: &Scene,
    ) -> Option<egui::Response> {
        let has_2d = !scene.two_d.is_empty();
        let has_3d = !scene.three_d.is_empty();
        let has_text = !scene.text.is_empty();
        let categories = [
            (has_2d).then_some(ViewCategory::TwoD),
            (has_3d).then_some(ViewCategory::ThreeD),
            (has_text).then_some(ViewCategory::Text),
        ]
        .iter()
        .filter_map(|cat| *cat)
        .collect::<Vec<_>>();

        match categories.len() {
            0 => None,
            1 => {
                if has_text {
                    self.view_state.ui_text(ctx, ui, &scene.text).into()
                } else {
                    None
                }
            }
            _ => {
                // Show tabs to let user select which category to view
                ui.vertical(|ui| {
                    if !categories.contains(&mut self.selected_category) {
                        self.selected_category = categories[0];
                    }

                    ui.horizontal(|ui| {
                        for category in categories {
                            let text = match category {
                                ViewCategory::TwoD => "2D",
                                ViewCategory::ThreeD => "3D",
                                ViewCategory::Tensor => "Tensor",
                                ViewCategory::Text => "Text",
                            };
                            ui.selectable_value(&mut self.selected_category, category, text);
                            // TODO(emilk): make it look like tabs
                        }
                    });
                    ui.separator();

                    match self.selected_category {
                        ViewCategory::Text => {
                            self.view_state.ui_text(ctx, ui, &scene.text);
                        }
                        _ => {}
                    }
                })
                .response
                .into()
            }
        }
    }

    pub fn objects_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        time_objects: &Objects<'_>,
        sticky_objects: &Objects<'_>,
    ) -> egui::Response {
        crate::profile_function!();

        let multidim_tensor = multidim_tensor(time_objects);
        let has_2d =
            time_objects.has_any_2d() && (multidim_tensor.is_none() || time_objects.len() > 1);
        let has_3d = time_objects.has_any_3d();

        let mut categories = vec![];
        if has_2d {
            categories.push(ViewCategory::TwoD);
        }
        if has_3d {
            categories.push(ViewCategory::ThreeD);
        }
        if multidim_tensor.is_some() {
            categories.push(ViewCategory::Tensor);
        }

        match categories.len() {
            0 => ui.label("(empty)"),
            1 => {
                if has_2d {
                    self.view_state
                        .ui_2d(ctx, ui, &self.space_path, time_objects)
                } else if has_3d {
                    self.view_state.ui_3d(
                        ctx,
                        ui,
                        &self.space_path,
                        spaces_info,
                        space_info,
                        time_objects,
                    )
                } else if let Some(multidim_tensor) = multidim_tensor {
                    self.view_state.ui_tensor(ui, multidim_tensor)
                } else {
                    panic!("nope, deprecated!"); // TODO
                }
            }
            _ => {
                // Show tabs to let user select which category to view
                ui.vertical(|ui| {
                    if !categories.contains(&self.selected_category) {
                        self.selected_category = categories[0];
                    }

                    ui.horizontal(|ui| {
                        for category in categories {
                            let text = match category {
                                ViewCategory::TwoD => "2D",
                                ViewCategory::ThreeD => "3D",
                                ViewCategory::Tensor => "Tensor",
                                ViewCategory::Text => "Text",
                            };
                            ui.selectable_value(&mut self.selected_category, category, text);
                            // TODO(emilk): make it look like tabs
                        }
                    });
                    ui.separator();

                    match self.selected_category {
                        ViewCategory::TwoD => {
                            self.view_state
                                .ui_2d(ctx, ui, &self.space_path, time_objects);
                        }
                        ViewCategory::ThreeD => {
                            self.view_state.ui_3d(
                                ctx,
                                ui,
                                &self.space_path,
                                spaces_info,
                                space_info,
                                time_objects,
                            );
                        }
                        ViewCategory::Tensor => {
                            self.view_state.ui_tensor(ui, multidim_tensor.unwrap());
                        }
                        ViewCategory::Text => {
                            panic!("nope, deprecated!"); // TODO
                        }
                    }
                })
                .response
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct ViewState {
    // per space
    state_2d: crate::view2d::State2D,

    state_3d: crate::view3d::State3D,

    state_tensor: Option<crate::view_tensor::TensorViewState>,

    state_text_entry: crate::text_entry_view::TextEntryState,
}

impl ViewState {
    fn ui_2d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        objects: &Objects<'_>,
    ) -> egui::Response {
        crate::view2d::view_2d(ctx, ui, &mut self.state_2d, Some(space), objects)
    }

    fn ui_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        objects: &Objects<'_>,
    ) -> egui::Response {
        ui.vertical(|ui| {
            let state = &mut self.state_3d;
            let space_cameras = &space_cameras(spaces_info, space_info);
            let coordinates = space_info.coordinates;
            let space_specs = crate::view3d::SpaceSpecs::from_view_coordinates(coordinates);
            let scene = crate::view3d::scene::Scene::from_objects(ctx, objects);
            crate::view3d::view_3d(
                ctx,
                ui,
                state,
                Some(space),
                &space_specs,
                scene,
                space_cameras,
            );
        })
        .response
    }

    fn ui_tensor(&mut self, ui: &mut egui::Ui, tensor: &re_log_types::Tensor) -> egui::Response {
        let state_tensor = self
            .state_tensor
            .get_or_insert_with(|| crate::ui::view_tensor::TensorViewState::create(tensor));
        ui.vertical(|ui| {
            crate::view_tensor::view_tensor(ui, state_tensor, tensor);
        })
        .response
    }

    fn ui_text(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &SceneText,
    ) -> egui::Response {
        self.state_text_entry.show(ui, ctx, scene)
    }
}

/// Look for camera transform and pinhole in the transform hierarchy
/// and return them as cameras.
fn space_cameras(spaces_info: &SpacesInfo, space_info: &SpaceInfo) -> Vec<SpaceCamera> {
    crate::profile_function!();

    let mut space_cameras = vec![];

    for (child_path, child_transform) in &space_info.child_spaces {
        if let Transform::Rigid3(world_from_camera) = child_transform {
            let world_from_camera = world_from_camera.parent_from_child();

            let view_space = spaces_info
                .spaces
                .get(child_path)
                .and_then(|child| child.coordinates);

            let mut found_any_pinhole = false;

            if let Some(child_space_info) = spaces_info.spaces.get(child_path) {
                for (grand_child_path, grand_child_transform) in &child_space_info.child_spaces {
                    if let Transform::Pinhole(pinhole) = grand_child_transform {
                        space_cameras.push(SpaceCamera {
                            camera_obj_path: child_path.clone(),
                            instance_index_hash: re_log_types::IndexHash::NONE,
                            camera_view_coordinates: view_space,
                            world_from_camera,
                            pinhole: Some(*pinhole),
                            target_space: Some(grand_child_path.clone()),
                        });
                        found_any_pinhole = true;
                    }
                }
            }

            if !found_any_pinhole {
                space_cameras.push(SpaceCamera {
                    camera_obj_path: child_path.clone(),
                    instance_index_hash: re_log_types::IndexHash::NONE,
                    camera_view_coordinates: view_space,
                    world_from_camera,
                    pinhole: None,
                    target_space: None,
                });
            }
        }
    }

    space_cameras
}

fn multidim_tensor<'s>(objects: &Objects<'s>) -> Option<&'s re_log_types::Tensor> {
    // We have a special tensor viewer that (currently) only works
    // when we only have a single tensor (and no bounding boxes etc).
    // It is also not as great for images as the normal 2d view (at least not yet).
    // This is a hacky-way of detecting this special case.
    // TODO(emilk): integrate the tensor viewer into the 2D viewer instead,
    // so we can stack bounding boxes etc on top of it.
    if objects.image.len() == 1 {
        let image = objects.image.first().unwrap().1;
        let tensor = image.tensor;

        // Ignore tensors that likely represent images.
        if tensor.num_dim() > 3 || tensor.num_dim() == 3 && tensor.shape.last().unwrap().size > 4 {
            return Some(tensor);
        }
    }
    None
}

// ----------------------------------------------------------------------------

#[derive(Debug)]
pub struct SceneQuery {
    pub objects: HashSet<ObjPath>,
    pub timeline: Timeline,
    pub time_query: TimeQuery<i64>,
}

#[derive(Default)]
pub struct Scene {
    pub two_d: Scene2d,
    pub three_d: Scene3d,
    // pub tensors: Vec<Tensor>,
    pub text: SceneText,
}

impl Scene {
    // TODO: this is temporary while we transition out of Objects
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        objects: &re_data_store::Objects<'_>,
    ) {
        self.two_d.load_objects(ctx, objects);
        self.three_d.load_objects(ctx, objects);
        self.text.load_objects(ctx, objects);
    }
}

impl Scene {}

// --- 2D ---

#[derive(Default)]
pub struct Scene2d {
    // TODO
}

impl Scene2d {
    // TODO: this is temporary while we transition out of Objects
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        objects: &re_data_store::Objects<'_>,
    ) {
    }
}

impl Scene2d {
    pub fn is_empty(&self) -> bool {
        true
    }
}

// --- 3D ---

// TODO: prob want to make some changes to these sub-types though.

pub struct Point {
    pub instance_id: InstanceIdHash,
    pub pos: [f32; 3],
    pub radius: Size,
    pub color: [u8; 4],
}

pub struct LineSegments {
    pub instance_id: InstanceIdHash,
    pub segments: Vec<[[f32; 3]; 2]>,
    pub radius: Size,
    pub color: [u8; 4],
}

#[cfg(feature = "glow")]
pub enum MeshSourceData {
    Mesh3D(re_log_types::Mesh3D),
    /// e.g. the camera mesh
    StaticGlb(&'static [u8]),
}

pub struct MeshSource {
    pub instance_id: InstanceIdHash,
    pub mesh_id: u64,
    pub world_from_mesh: glam::Affine3A,
    // pub cpu_mesh: Arc<CpuMesh>,
    pub tint: Option<[u8; 4]>,
}

pub struct Label {
    pub(crate) text: String,
    /// Origin of the label
    pub(crate) origin: Vec3,
}

#[derive(Default)]
pub struct Scene3d {
    pub points: Vec<Point>,
    pub line_segments: Vec<LineSegments>,
    pub meshes: Vec<MeshSource>,
    pub labels: Vec<Label>,
}

impl Scene3d {
    // TODO: this is temporary while we transition out of Objects
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        objects: &re_data_store::Objects<'_>,
    ) {
    }
}

impl Scene3d {
    pub fn is_empty(&self) -> bool {
        true
    }
}

// --- Text logs ---

pub struct TextEntry {
    // props
    pub msg_id: MsgId,
    pub obj_path: ObjPath,
    pub time: i64,
    pub color: Option<[u8; 4]>,

    // text entry
    pub level: Option<String>,
    pub body: String,
}

#[derive(Default)]
pub struct SceneText {
    pub text_entries: Vec<TextEntry>,
}

impl SceneText {
    // TODO: this is temporary while we transition out of Objects
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        objects: &re_data_store::Objects<'_>,
    ) {
        let mut text_entries = {
            crate::profile_scope!("SceneText - collect text entries");
            objects.text_entry.iter().collect::<Vec<_>>()
        };

        {
            crate::profile_scope!("SceneText - sort text entries");
            text_entries.sort_by(|a, b| {
                a.0.time
                    .cmp(&b.0.time)
                    .then_with(|| a.0.obj_path.cmp(b.0.obj_path))
            });
        }

        // TODO: obviously cloning all these strings is not ideal... there are two
        // situations to account for here.
        // We could avoid these by modifying how we store all of this in the existing
        // datastore, but then again we are about to rewrite the datastore so...?
        // We will need to make sure that we don't need these copies once we switch to
        // Arrow though!
        self.text_entries
            .extend(text_entries.into_iter().map(|(props, entry)| TextEntry {
                // props
                msg_id: props.msg_id.clone(),
                obj_path: props.obj_path.clone(), // shallow
                time: props.time,
                color: props.color,
                // text entry
                level: entry.level.map(ToOwned::to_owned),
                body: entry.body.to_owned(),
            }));
    }
}

impl SceneText {
    pub fn is_empty(&self) -> bool {
        self.text_entries.is_empty()
    }
}
