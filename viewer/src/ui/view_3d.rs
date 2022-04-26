use eframe::egui;
use egui::util::hash;
use egui::{Color32, Rect};
use glam::Affine3A;
use itertools::Itertools;
use macaw::{vec3, IsoTransform, Mat4, Quat, Vec3};

use log_types::*;
use three_d::RenderStates;

use crate::mesh_loader::GpuMesh;
use crate::ViewerContext;
use crate::{log_db::SpaceSummary, LogDb};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct State3D {
    camera: Option<OrbitCamera>,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct OrbitCamera {
    center: Vec3,
    radius: f32,
    world_from_view_rot: Quat,
    fov_y: f32,
    /// Zero = no up (3dof rotation)
    up: Vec3,
}

impl OrbitCamera {
    const MAX_PITCH: f32 = 0.999 * 0.25 * std::f32::consts::TAU;

    fn to_camera(self) -> Camera {
        let pos = self.center + self.world_from_view_rot * vec3(0.0, 0.0, self.radius);
        Camera {
            world_from_view: IsoTransform::from_rotation_translation(self.world_from_view_rot, pos),
            fov_y: self.fov_y,
        }
    }

    /// Direction we are looking at
    pub fn dir(&self) -> Vec3 {
        self.world_from_view_rot * -Vec3::Z
    }

    /// Only valid if we have an up vector.
    ///
    /// `[-tau/4, +tau/4]`
    pub fn pitch(&self) -> Option<f32> {
        if self.up == Vec3::ZERO {
            None
        } else {
            Some(self.dir().dot(self.up).clamp(-1.0, 1.0).asin())
        }
    }

    pub fn set_dir(&mut self, dir: Vec3) {
        if self.up == Vec3::ZERO {
            self.world_from_view_rot = Quat::from_rotation_arc(-Vec3::Z, dir);
        } else {
            let pitch = self
                .pitch()
                .unwrap()
                .clamp(-Self::MAX_PITCH, Self::MAX_PITCH);

            let dir = project_onto(dir, self.up).normalize(); // Remove pitch
            let right = dir.cross(self.up).normalize();
            let dir = Quat::from_axis_angle(right, pitch) * dir; // Tilt up/down
            let dir = dir.normalize(); // Prevent drift

            self.world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, dir, self.up).inverse());
        }
    }

    pub fn set_up(&mut self, up: Vec3) {
        self.up = up.normalize_or_zero();

        if self.up != Vec3::ZERO {
            self.set_dir(self.dir()); // this will clamp the rotation
        }
    }

    pub fn rotate(&mut self, delta: egui::Vec2) {
        let sensitivity = 0.004; // radians-per-point
        let delta = sensitivity * delta;

        if self.up == Vec3::ZERO {
            // 3-dof rotation
            let rot_delta = Quat::from_rotation_y(-delta.x) * Quat::from_rotation_x(-delta.y);
            self.world_from_view_rot *= rot_delta;
        } else {
            // 2-dof rotation
            let dir = Quat::from_axis_angle(self.up, -delta.x) * self.dir();
            let dir = dir.normalize(); // Prevent drift

            let pitch = self.pitch().unwrap() - delta.y;
            let pitch = pitch.clamp(-Self::MAX_PITCH, Self::MAX_PITCH);

            let dir = project_onto(dir, self.up).normalize(); // Remove pitch
            let right = dir.cross(self.up).normalize();
            let dir = Quat::from_axis_angle(right, pitch) * dir; // Tilt up/down
            let dir = dir.normalize(); // Prevent drift

            self.world_from_view_rot =
                Quat::from_affine3(&Affine3A::look_at_rh(Vec3::ZERO, dir, self.up).inverse());
        }
    }
}

/// e.g. up is [0,0,1], we return things like [x,y,0]
fn project_onto(v: Vec3, up: Vec3) -> Vec3 {
    v - up * v.dot(up)
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
struct Camera {
    world_from_view: IsoTransform,
    fov_y: f32,
}

impl Camera {
    #[allow(clippy::unused_self)]
    fn near(&self) -> f32 {
        0.01 // TODO
    }

    fn screen_from_world(&self, rect: &Rect) -> Mat4 {
        let aspect_ratio = rect.width() / rect.height();
        Mat4::from_translation(vec3(rect.center().x, rect.center().y, 0.0))
            * Mat4::from_scale(0.5 * vec3(rect.width(), -rect.height(), 1.0))
            * Mat4::perspective_infinite_rh(self.fov_y, aspect_ratio, self.near())
            * self.world_from_view.inverse()
    }

    fn pos(&self) -> glam::Vec3 {
        self.world_from_view.translation()
    }
}

struct Point {
    pos: [f32; 3],
    radius: f32,
    color: Color32,
}

struct LineSegments {
    segments: Vec<[[f32; 3]; 2]>,
    radius: f32,
    color: Color32,
}

enum MeshSourceData {
    Mesh3D(Mesh3D),
    /// e.g. the camera mesh
    StaticGlb(&'static [u8]),
}

struct MeshSource {
    mesh_id: u64,
    object_path: ObjectPath,
    world_from_mesh: glam::Mat4,
    mesh_data: MeshSourceData,
}

#[derive(Default)]
struct Scene {
    points: Vec<Point>,
    line_segments: Vec<LineSegments>,
    meshes: Vec<MeshSource>,
}

impl Scene {
    pub fn add_msg(&mut self, camera: &Camera, is_hovered: bool, color: Color32, msg: &LogMsg) {
        let radius_multiplier = if is_hovered { 1.5 } else { 1.0 };
        // TODO: base sizes on viewport size and maybe fov_y
        let small_radius = 0.02 * radius_multiplier;
        let point_radius_from_distance = 0.002 * radius_multiplier;
        let line_radius_from_distance = 0.001 * radius_multiplier;

        let eye_pos = camera.pos();

        match &msg.data {
            Data::Pos3(pos) => {
                // scale with distance
                let dist_to_camera = eye_pos.distance(Vec3::from(*pos));
                self.points.push(Point {
                    pos: *pos,
                    radius: dist_to_camera * point_radius_from_distance,
                    color,
                });
            }
            Data::Vec3(_) => {
                // Can't visualize vectors (yet)
            }
            Data::Box3(box3) => {
                self.add_box(camera, color, line_radius_from_distance, box3);
            }
            Data::Path3D(points) => {
                let bbox = macaw::BoundingBox::from_points(points.iter().copied().map(Vec3::from));
                let dist_to_camera = eye_pos.distance(bbox.center());
                let segments = points
                    .iter()
                    .tuple_windows()
                    .map(|(a, b)| [*a, *b])
                    .collect();

                self.line_segments.push(LineSegments {
                    segments,
                    radius: dist_to_camera * line_radius_from_distance,
                    color,
                });
            }
            Data::LineSegments3D(segments) => self.line_segments.push(LineSegments {
                segments: segments.clone(),
                radius: small_radius, // TODO: scale based on camera distance
                color,
            }),
            Data::Mesh3D(mesh) => {
                self.meshes.push(MeshSource {
                    mesh_id: hash(msg.id),
                    object_path: msg.object_path.clone(),
                    world_from_mesh: glam::Mat4::IDENTITY,
                    mesh_data: MeshSourceData::Mesh3D(mesh.clone()),
                });
            }
            Data::Camera(camera) => {
                let rotation = Quat::from_slice(&camera.rotation);
                let translation = Vec3::from_slice(&camera.position);

                // let dist_to_camera = eye_pos.distance(translation);
                // let scale = Vec3::splat(0.1 * dist_to_camera);
                let scale = Vec3::splat(0.05); // camera mesh is 1m long

                let world_from_mesh =
                    Mat4::from_scale_rotation_translation(scale, rotation, translation);
                self.meshes.push(MeshSource {
                    mesh_id: hash("camera"),
                    object_path: msg.object_path.clone(),
                    world_from_mesh,
                    mesh_data: MeshSourceData::StaticGlb(include_bytes!("../../data/camera.glb")),
                });
            }
            _ => {
                debug_assert!(!msg.data.is_3d());
            }
        }
    }

    pub fn add_box(
        &mut self,
        camera: &Camera,
        color: Color32,
        line_radius_from_distance: f32,
        box3: &Box3,
    ) {
        let Box3 {
            rotation,
            translation,
            half_size,
        } = box3;
        let rotation = glam::Quat::from_array(*rotation);
        let translation = glam::Vec3::from(*translation);
        let half_size = glam::Vec3::from(*half_size);
        let transform =
            glam::Mat4::from_scale_rotation_translation(half_size, rotation, translation);

        let corners = [
            transform
                .transform_point3(vec3(-0.5, -0.5, -0.5))
                .to_array(),
            transform.transform_point3(vec3(-0.5, -0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(-0.5, 0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(-0.5, 0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(0.5, -0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(0.5, -0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(0.5, 0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(0.5, 0.5, 0.5)).to_array(),
        ];

        let segments = vec![
            // bottom:
            [corners[0b000], corners[0b001]],
            [corners[0b000], corners[0b010]],
            [corners[0b011], corners[0b001]],
            [corners[0b011], corners[0b010]],
            // top:
            [corners[0b100], corners[0b101]],
            [corners[0b100], corners[0b110]],
            [corners[0b111], corners[0b101]],
            [corners[0b111], corners[0b110]],
            // sides:
            [corners[0b000], corners[0b100]],
            [corners[0b001], corners[0b101]],
            [corners[0b010], corners[0b110]],
            [corners[0b011], corners[0b111]],
        ];

        let dist_to_camera = camera.pos().distance(translation);
        self.line_segments.push(LineSegments {
            segments,
            radius: dist_to_camera * line_radius_from_distance,
            color,
        });
    }
}

fn show_settings_ui(
    ui: &mut egui::Ui,
    state_3d: &mut State3D,
    space_summary: &SpaceSummary,
    space_specs: &SpaceSpecs,
) {
    if ui.button("Reset camera").clicked() {
        state_3d.camera = Some(default_camera(space_summary, space_specs));
    }
}

#[derive(Default)]
struct SpaceSpecs {
    up: Option<glam::Vec3>,
}

impl SpaceSpecs {
    fn from_messages(space: &ObjectPath, messages: &[&LogMsg]) -> Self {
        let mut slf = Self::default();

        let up_path = space / "up";

        for msg in messages {
            if msg.object_path == up_path {
                if let Data::Vec3(vec3) = msg.data {
                    slf.up = Some(vec3.into());
                } else {
                    tracing::warn!("Expected {} to be a Vec3; got: {:?}", up_path, msg.data);
                }
            }
        }
        slf
    }
}

pub(crate) fn combined_view_3d(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    state_3d: &mut State3D,
    space: &ObjectPath,
    space_summary: &SpaceSummary,
    messages: &[&LogMsg],
) {
    crate::profile_function!();

    let space_specs = SpaceSpecs::from_messages(space, messages);

    // TODO: show settings on top of 3D view.
    // Requires some egui work to handle interaction of overlapping widgets.
    show_settings_ui(ui, state_3d, space_summary, &space_specs);

    let frame = egui::Frame::canvas(ui.style()).inner_margin(2.0);
    let (outer_rect, response) =
        ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    // ---------------------------------

    let camera = state_3d
        .camera
        .get_or_insert_with(|| default_camera(space_summary, &space_specs));

    camera.set_up(space_specs.up.unwrap_or(Vec3::ZERO));

    if response.dragged() {
        camera.rotate(response.drag_delta());
    }

    // ---------------------------------

    // TODO: focus
    // if response.clicked() || response.dragged() {
    //     ui.ctx().memory().request_focus(response.id);
    // } else if response.clicked_elsewhere() {
    //     ui.ctx().memory().surrender_focus(response.id);
    // }
    // if ui.ctx().memory().has_focus(response.id) {
    //     frame.stroke = ui.visuals().selection.stroke; // TODO: something less subtle
    //     frame.stroke.width *= 2.0; // hack to make it less subtle
    // }
    // if ui.ctx().memory().has_focus(response.id) {
    //     // TODO: WASD movement
    // }

    if response.hovered() {
        camera.radius /= ui.input().zoom_delta();
    }

    ui.painter().add(frame.paint(outer_rect));

    let inner_rect = outer_rect.shrink2(frame.inner_margin.sum() + frame.outer_margin.sum());

    let camera = camera.to_camera();

    let hovered_id = picking(ui, &inner_rect, space, messages, &camera);
    if let Some(hovered_id) = hovered_id {
        if response.clicked() {
            context.selection = crate::Selection::LogId(hovered_id);
        }

        if let Some(msg) = log_db.get_msg(&hovered_id) {
            egui::containers::popup::show_tooltip_at_pointer(
                ui.ctx(),
                egui::Id::new("3d_tooltip"),
                |ui| {
                    crate::view_2d::on_hover_ui(context, ui, msg);
                },
            );
        }
    }

    let mut scene = Scene::default();
    for msg in messages {
        if msg.space.as_ref() == Some(space) {
            let is_hovered = Some(msg.id) == hovered_id;

            // TODO: selection color
            let color = if is_hovered {
                Color32::WHITE
            } else {
                context.object_color(log_db, msg)
            };

            scene.add_msg(&camera, is_hovered, color, msg);
        }
    }

    let callback = egui::PaintCallback {
        rect: inner_rect,
        callback: std::sync::Arc::new(move |info, render_ctx| {
            if let Some(painter) = render_ctx.downcast_ref::<egui_glow::Painter>() {
                with_three_d_context(painter.gl(), |rendering| {
                    paint_with_three_d(rendering, &camera, info, &scene).unwrap();
                });
            } else {
                eprintln!("Can't do custom painting because we are not using a glow context");
            }
        }),
    };
    ui.painter().add(callback);
}

fn picking(
    ui: &egui::Ui,
    rect: &Rect,
    space: &ObjectPath,
    messages: &[&LogMsg],
    camera: &Camera,
) -> Option<LogId> {
    let pointer_pos = ui.ctx().pointer_hover_pos()?;

    let screen_from_world = camera.screen_from_world(rect);

    let mut closest_dist_sq = 5.0 * 5.0; // TODO: interaction radius from egui
    let mut closest_id = None;

    for msg in messages {
        if msg.space.as_ref() == Some(space) {
            match &msg.data {
                Data::Pos3([x, y, z]) => {
                    let screen_pos = screen_from_world.project_point3(vec3(*x, *y, *z));
                    if screen_pos.z < 0.0 {
                        continue;
                    }
                    let screen_pos = egui::pos2(screen_pos.x, screen_pos.y);

                    let dist_sq = screen_pos.distance_sq(pointer_pos);
                    if dist_sq < closest_dist_sq {
                        closest_dist_sq = dist_sq;
                        closest_id = Some(msg.id);
                    }

                    if false {
                        // good for sanity checking the projection matrix
                        ui.ctx()
                            .debug_painter()
                            .circle_filled(screen_pos, 3.0, egui::Color32::RED);
                    }
                }
                Data::Vec3(_)
                | Data::Box3(_)
                | Data::Path3D(_)
                | Data::LineSegments3D(_)
                | Data::Mesh3D(_)
                | Data::Camera(_) => {
                    // TODO: more picking
                }
                _ => {
                    debug_assert!(!msg.data.is_3d());
                }
            }
        }
    }

    closest_id
}

fn default_camera(space_summary: &SpaceSummary, space_spects: &SpaceSpecs) -> OrbitCamera {
    let bbox = space_summary.bbox3d;

    let mut center = bbox.center();
    if !center.is_finite() {
        center = Vec3::ZERO;
    }

    let mut radius = 3.0 * bbox.half_size().length();
    if !radius.is_finite() || radius == 0.0 {
        radius = 1.0;
    }

    let cam_dir = vec3(1.0, 1.0, 0.5).normalize();
    let camera_pos = center + radius * cam_dir;

    let up = space_spects.up.unwrap_or(Vec3::Z);

    OrbitCamera {
        center,
        radius,
        world_from_view_rot: Quat::from_affine3(
            &Affine3A::look_at_rh(camera_pos, center, up).inverse(),
        ),
        fov_y: 50.0_f32.to_radians(), // TODO: base on viewport size?
        up: space_spects.up.unwrap_or(Vec3::ZERO),
    }
}

/// We get a [`glow::Context`] from `eframe`, but we want a [`three_d::Context`].
///
/// Sadly we can't just create and store a [`three_d::Context`] in the app and pass it
/// to the [`egui::PaintCallback`] because [`three_d::Context`] isn't `Send+Sync`, which
/// [`egui::PaintCallback`] is.
fn with_three_d_context<R>(
    gl: &std::rc::Rc<glow::Context>,
    f: impl FnOnce(&mut RenderingContext) -> R,
) -> R {
    use std::cell::RefCell;
    thread_local! {
        static THREE_D: RefCell<Option<RenderingContext>> = RefCell::new(None);
    }

    #[allow(unsafe_code)]
    unsafe {
        use glow::HasContext as _;
        gl.enable(glow::DEPTH_TEST);
        if !cfg!(target_arch = "wasm32") {
            gl.disable(glow::FRAMEBUFFER_SRGB);
        }
        gl.clear(glow::DEPTH_BUFFER_BIT);
    }

    THREE_D.with(|three_d| {
        let mut three_d = three_d.borrow_mut();
        let three_d = three_d.get_or_insert_with(|| RenderingContext::new(gl).unwrap());
        f(three_d)
    })
}

struct RenderingContext {
    three_d: three_d::Context,

    mesh_cache: MeshCache,

    sphere_mesh: three_d::CpuMesh,
    line_mesh: three_d::CpuMesh,

    /// So we don't need to re-allocate them.
    points_cache: Vec<three_d::InstancedModel<three_d::PhysicalMaterial>>,
    lines_cache: Vec<three_d::InstancedModel<three_d::ColorMaterial>>,
}

impl RenderingContext {
    pub fn new(gl: &std::rc::Rc<glow::Context>) -> three_d::ThreeDResult<Self> {
        let three_d = three_d::Context::from_gl_context(gl.clone())?;

        Ok(Self {
            three_d,
            sphere_mesh: three_d::CpuMesh::sphere(24),
            line_mesh: three_d::CpuMesh::cylinder(10),
            mesh_cache: Default::default(),
            points_cache: Default::default(),
            lines_cache: Default::default(),
        })
    }
}

fn allocate_points<'a>(
    three_d: &'a three_d::Context,
    sphere_mesh: &'a three_d::CpuMesh,
    points_cache: &'a mut Vec<three_d::InstancedModel<three_d::PhysicalMaterial>>,
    render_states: three_d::RenderStates,
    points: &'a [Point],
) -> &'a [three_d::InstancedModel<three_d::PhysicalMaterial>] {
    crate::profile_function!();
    use three_d::*;

    let mut per_color_instances: ahash::AHashMap<Color32, Vec<Instance>> = Default::default();
    for point in points {
        let p = point.pos;
        let geometry_transform =
            Mat4::from_translation(vec3(p[0], p[1], p[2])) * Mat4::from_scale(point.radius);
        per_color_instances
            .entry(point.color)
            .or_default()
            .push(Instance {
                geometry_transform,
                ..Default::default()
            });
    }

    if points_cache.len() < per_color_instances.len() {
        points_cache.resize_with(per_color_instances.len(), || {
            let material = PhysicalMaterial {
                roughness: 1.0,
                metallic: 0.0,
                lighting_model: LightingModel::Cook(
                    NormalDistributionFunction::TrowbridgeReitzGGX,
                    GeometryFunction::SmithSchlickGGX,
                ),
                ..Default::default()
            };
            InstancedModel::new_with_material(three_d, &[], sphere_mesh, material).unwrap()
        });
    }

    for ((color, instances), points) in per_color_instances.iter().zip(points_cache.iter_mut()) {
        points.material.albedo = color_to_three_d(*color);
        points.material.render_states = render_states;
        points.set_instances(instances).unwrap();
    }

    &points_cache[..per_color_instances.len()]
}

fn allocate_line_segments<'a>(
    three_d: &'a three_d::Context,
    line_mesh: &'a three_d::CpuMesh,
    lines_cache: &'a mut Vec<three_d::InstancedModel<three_d::ColorMaterial>>,
    render_states: three_d::RenderStates,
    line_segments: &'a [LineSegments],
) -> &'a [three_d::InstancedModel<three_d::ColorMaterial>] {
    crate::profile_function!();
    use three_d::*;

    if lines_cache.len() < line_segments.len() {
        lines_cache.resize_with(line_segments.len(), || {
            let material = ColorMaterial::default();
            InstancedModel::new_with_material(three_d, &[], line_mesh, material).unwrap()
        });
    }

    for (line_segments, model) in line_segments.iter().zip(lines_cache.iter_mut()) {
        let LineSegments {
            segments,
            radius,
            color,
        } = line_segments;

        let line_instances: Vec<Instance> = segments
            .iter()
            .map(|&[p0, p1]| {
                let p0 = vec3(p0[0], p0[1], p0[2]);
                let p1 = vec3(p1[0], p1[1], p1[2]);
                let scale = Mat4::from_nonuniform_scale((p0 - p1).magnitude(), 1.0, 1.0);
                let rotation =
                    rotation_matrix_from_dir_to_dir(vec3(1.0, 0.0, 0.0), (p1 - p0).normalize());
                let translation = Mat4::from_translation(p0);
                let geometry_transform = translation
                    * rotation
                    * scale
                    * Mat4::from_nonuniform_scale(1.0, *radius, *radius);
                Instance {
                    geometry_transform,
                    ..Default::default()
                }
            })
            .collect();

        model.material.render_states = render_states;
        model.material.color = color_to_three_d(*color);

        model.set_instances(&line_instances).unwrap();
    }

    &lines_cache[..line_segments.len()]
}

#[derive(Default)]
struct MeshCache(nohash_hasher::IntMap<u64, Option<GpuMesh>>);

impl MeshCache {
    fn load(
        &mut self,
        three_d: &three_d::Context,
        mesh_id: u64,
        object_path: &ObjectPath,
        mesh_data: &MeshSourceData,
    ) {
        crate::profile_function!();
        self.0.entry(mesh_id).or_insert_with(|| {
            let name = object_path.to_string();
            tracing::debug!("Loading mesh {}…", name);
            let result = match mesh_data {
                MeshSourceData::Mesh3D(mesh3d) => {
                    crate::mesh_loader::load(three_d, name.clone(), mesh3d)
                }
                MeshSourceData::StaticGlb(glb_bytes) => {
                    crate::mesh_loader::load_raw(three_d, name.clone(), MeshFormat::Glb, glb_bytes)
                }
            };

            match result {
                Ok(gpu_mesh) => Some(gpu_mesh),
                Err(err) => {
                    tracing::warn!("{}: Failed to load mesh: {}", name, err);
                    None
                }
            }
        });
    }

    fn set_instances(
        &mut self,
        mesh_id: u64,
        render_states: RenderStates,
        instances: &[three_d::Instance],
    ) -> three_d::ThreeDResult<()> {
        if let Some(Some(gpu_mesh)) = self.0.get_mut(&mesh_id) {
            for model in &mut gpu_mesh.models {
                model.material.render_states = render_states;
                model.set_instances(instances)?;
            }
        }
        Ok(())
    }

    fn get(&self, mesh_id: u64) -> Option<&GpuMesh> {
        self.0.get(&mesh_id)?.as_ref()
    }
}

fn paint_with_three_d(
    rendering: &mut RenderingContext,
    camera: &Camera,
    info: &egui::PaintCallbackInfo,
    scene: &Scene,
) -> three_d::ThreeDResult<()> {
    crate::profile_function!();
    use three_d::*;
    let three_d = &rendering.three_d;

    let viewport = info.viewport_in_pixels();
    let viewport = Viewport {
        x: viewport.left_px.round() as _,
        y: viewport.from_bottom_px.round() as _,
        width: viewport.width_px.round() as _,
        height: viewport.height_px.round() as _,
    };

    // Respect the egui clip region (e.g. if we are inside an `egui::ScrollArea`).
    let clip_rect = info.clip_rect_in_pixels();
    let render_states = RenderStates {
        clip: Clip::Enabled {
            x: clip_rect.left_px.round() as _,
            y: clip_rect.from_bottom_px.round() as _,
            width: clip_rect.width_px.round() as _,
            height: clip_rect.height_px.round() as _,
        },
        ..Default::default()
    };

    let position = camera.world_from_view.translation();
    let target = camera.world_from_view.transform_point3(-glam::Vec3::Z);
    let up = camera.world_from_view.transform_vector3(glam::Vec3::Y);
    let camera = Camera::new_perspective(
        three_d,
        viewport,
        mint::Vector3::from(position).into(),
        mint::Vector3::from(target).into(),
        mint::Vector3::from(up).into(),
        radians(camera.fov_y),
        camera.near(),
        1000.0, // TODO: infinity (https://github.com/rustgd/cgmath/pull/547)
    )?;

    // -------------------

    let ambient = AmbientLight::new(three_d, 0.7, Color::WHITE)?;
    let directional0 = DirectionalLight::new(three_d, 2.0, Color::WHITE, &vec3(-1.0, -1.0, -1.0))?;
    let directional1 = DirectionalLight::new(three_d, 2.0, Color::WHITE, &vec3(1.0, 1.0, 1.0))?;
    let lights: &[&dyn Light] = &[&ambient, &directional0, &directional1];

    // -------------------

    let Scene {
        points,
        line_segments,
        meshes,
    } = scene;

    let mut mesh_instances: std::collections::HashMap<u64, Vec<Instance>> = Default::default();

    for mesh in meshes {
        mesh_instances
            .entry(mesh.mesh_id)
            .or_default()
            .push(Instance {
                geometry_transform: mint::ColumnMatrix4::from(mesh.world_from_mesh).into(),
                ..Default::default()
            });

        rendering
            .mesh_cache
            .load(three_d, mesh.mesh_id, &mesh.object_path, &mesh.mesh_data);
    }

    for (mesh_id, instances) in &mesh_instances {
        rendering
            .mesh_cache
            .set_instances(*mesh_id, render_states, instances)?;
    }

    let mut objects: Vec<&dyn Object> = vec![];
    for &mesh_id in mesh_instances.keys() {
        if let Some(gpu_mesh) = rendering.mesh_cache.get(mesh_id) {
            for obj in &gpu_mesh.models {
                objects.push(obj);
            }
        }
    }

    for obj in allocate_points(
        &rendering.three_d,
        &rendering.sphere_mesh,
        &mut rendering.points_cache,
        render_states,
        points,
    ) {
        objects.push(obj);
    }
    for obj in allocate_line_segments(
        &rendering.three_d,
        &rendering.line_mesh,
        &mut rendering.lines_cache,
        render_states,
        line_segments,
    ) {
        objects.push(obj);
    }

    crate::profile_scope!("render_pass");
    render_pass(&camera, &objects, lights)?;

    Ok(())
}

fn color_to_three_d(color: egui::Color32) -> three_d::Color {
    assert_eq!(color.a(), 255);

    // three_d::Color::new_opaque(color.r(), color.g(), color.b())

    // TODO: figure out why three_d colors are messed up
    let rgba: egui::Rgba = color.into();
    three_d::Color::new_opaque(
        (rgba.r() * 255.0).round() as _,
        (rgba.g() * 255.0).round() as _,
        (rgba.b() * 255.0).round() as _,
    )
}
