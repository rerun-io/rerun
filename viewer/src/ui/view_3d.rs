use eframe::egui;
use egui::{Color32, Rect};
use glam::Affine3A;
use macaw::{vec3, IsoTransform, Mat4, Quat, Vec3};
use std::rc::Rc;

use log_types::*;

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
}

impl OrbitCamera {
    fn to_camera(self) -> Camera {
        let pos = self.center + self.world_from_view_rot * vec3(0.0, 0.0, self.radius);
        Camera {
            world_from_view: IsoTransform::from_rotation_translation(self.world_from_view_rot, pos),
            fov_y: self.fov_y,
        }
    }
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

#[derive(Default)]
struct Scene {
    points: Vec<Point>,
    line_segments: Vec<LineSegments>,
    meshes: Vec<(LogId, ObjectPath, Mesh3D)>,
}

fn show_settings_ui(ui: &mut egui::Ui, state_3d: &mut State3D) {
    if ui.button("Reset camera").clicked() {
        state_3d.camera = Some(default_camera());
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
    if space_summary.messages_3d.is_empty() {
        return;
    }
    crate::profile_function!();

    // TODO: show settings on top of 3D view.
    // Requires some egui work to handle interaction of overlapping widgets.
    show_settings_ui(ui, state_3d);

    let frame = egui::Frame {
        inner_margin: 2.0.into(),
        ..egui::Frame::dark_canvas(ui.style())
    };
    let (outer_rect, response) =
        ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    // ---------------------------------

    let camera = state_3d.camera.get_or_insert_with(default_camera);

    if response.dragged() {
        let drag_delta = response.drag_delta();
        let sensitivity = 0.004; // radians-per-point
        let rot_delta = Quat::from_rotation_y(-sensitivity * drag_delta.x)
            * Quat::from_rotation_x(-sensitivity * drag_delta.y);
        camera.world_from_view_rot *= rot_delta;
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
            let radius = if is_hovered { 0.2 } else { 0.1 }; // TODO: base on distance

            // TODO: selection color
            let color = if is_hovered {
                Color32::WHITE
            } else {
                context.object_color(log_db, &msg.object_path)
            };

            match &msg.data {
                Data::Pos3(pos) => scene.points.push(Point {
                    pos: *pos,
                    radius,
                    color,
                }),
                Data::LineSegments3D(segments) => scene.line_segments.push(LineSegments {
                    segments: segments.clone(),
                    radius,
                    color,
                }),
                Data::Mesh3D(mesh) => {
                    scene
                        .meshes
                        .push((msg.id, msg.object_path.clone(), mesh.clone()));
                }
                _ => {
                    debug_assert!(!msg.data.is_3d());
                }
            }
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
                Data::LineSegments3D(_) | Data::Mesh3D(_) => {
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

fn default_camera() -> OrbitCamera {
    // TODO: calculate an initial/default camera based on the scene contents
    let radius = 25.0;
    let camera_pos = vec3(1.0, 1.0, 0.5).normalize() * radius;
    let center = Vec3::ZERO;
    let up = Vec3::Z;

    OrbitCamera {
        center,
        radius,
        world_from_view_rot: Quat::from_affine3(
            &Affine3A::look_at_rh(camera_pos, center, up).inverse(),
        ),
        fov_y: 50.0_f32.to_radians(), // TODO: base on viewport size?
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
}

impl RenderingContext {
    pub fn new(gl: &std::rc::Rc<glow::Context>) -> three_d::ThreeDResult<Self> {
        let three_d = three_d::Context::from_gl_context(gl.clone())?;

        Ok(Self {
            three_d,
            mesh_cache: Default::default(),
        })
    }
}

#[derive(Default)]
struct MeshCache(nohash_hasher::IntMap<LogId, Option<Rc<GpuMesh>>>);

impl MeshCache {
    fn load(
        &mut self,
        three_d: &three_d::Context,
        log_id: &LogId,
        object_path: &ObjectPath,
        mesh_data: &Mesh3D,
    ) -> Option<Rc<GpuMesh>> {
        crate::profile_function!();
        self.0
            .entry(*log_id)
            .or_insert_with(|| {
                tracing::debug!("Loading mesh {}…", object_path);
                match crate::mesh_loader::load(three_d, mesh_data) {
                    Ok(gpu_mesh) => Some(Rc::new(gpu_mesh)),
                    Err(err) => {
                        tracing::warn!("{}: Failed to load mesh: {}", object_path, err);
                        None
                    }
                }
            })
            .clone()
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

    let viewport = Viewport {
        x: info.viewport_left_px().round() as _,
        y: info.viewport_from_bottom_px().round() as _,
        width: info.viewport_width_px().round() as _,
        height: info.viewport_height_px().round() as _,
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

    let sphere_mesh = CpuMesh::sphere(32);
    let points: Vec<_> = points
        .iter()
        .map(|point| point_to_three_d(three_d, &sphere_mesh, point))
        .collect();

    let line_segments: Vec<_> = line_segments
        .iter()
        .map(|line_segments| line_segments_to_three_d(three_d, line_segments))
        .collect();

    let meshes: Vec<Rc<GpuMesh>> = meshes
        .iter()
        .filter_map(|(log_id, obj_path, mesh)| {
            rendering.mesh_cache.load(three_d, log_id, obj_path, mesh)
        })
        .collect();

    let mut objects: Vec<&dyn Object> = vec![];
    for obj in &points {
        objects.push(obj);
    }
    for obj in &line_segments {
        objects.push(obj);
    }
    for mesh in &meshes {
        for obj in &mesh.models {
            objects.push(obj);
        }
    }

    crate::profile_scope!("render_pass");
    render_pass(&camera, &objects, lights)?;

    Ok(())
}

fn point_to_three_d(
    three_d: &three_d::Context,
    sphere_mesh: &three_d::CpuMesh,
    point: &Point,
) -> three_d::Model<three_d::PhysicalMaterial> {
    crate::profile_function!();
    use three_d::*;

    let [x, y, z] = point.pos;
    let pos = vec3(x, y, z);

    let color = color_to_three_d(point.color);

    let material = PhysicalMaterial {
        albedo: color,
        roughness: 1.0,
        metallic: 0.0,
        lighting_model: LightingModel::Cook(
            NormalDistributionFunction::TrowbridgeReitzGGX,
            GeometryFunction::SmithSchlickGGX,
        ),
        ..Default::default()
    };

    // let material = ColorMaterial {
    //     color,
    //     ..Default::default()
    // };

    let mut model = Model::new_with_material(three_d, sphere_mesh, material).unwrap();
    model.set_transformation(Mat4::from_translation(pos) * Mat4::from_scale(point.radius));
    model
}

fn line_segments_to_three_d(
    three_d: &three_d::Context,
    line_segments: &LineSegments,
) -> three_d::InstancedModel<three_d::ColorMaterial> {
    crate::profile_function!();
    use three_d::*;

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
            let geometry_transform = translation * rotation * scale;
            Instance {
                geometry_transform,
                ..Default::default()
            }
        })
        .collect();

    // Used to paint lines
    let line_material = ColorMaterial {
        color: color_to_three_d(*color),
        ..Default::default()
    };
    let mut line = CpuMesh::cylinder(10);
    line.transform(&Mat4::from_nonuniform_scale(1.0, *radius, *radius))
        .unwrap();
    let lines =
        InstancedModel::new_with_material(three_d, &line_instances, &line, line_material).unwrap();

    lines
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
