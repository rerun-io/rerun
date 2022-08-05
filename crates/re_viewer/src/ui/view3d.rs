mod camera;
mod mesh_cache;
mod rendering;
mod scene;

pub use mesh_cache::CpuMeshCache;

use camera::*;
use rendering::*;
use scene::*;

use egui::NumExt as _;
use glam::Affine3A;
use macaw::{vec3, Quat, Vec3};
use re_log_types::ObjPath;

use crate::LogDb;
use crate::{misc::Selection, ViewerContext};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct State3D {
    orbit_camera: Option<OrbitCamera>,

    #[serde(skip)]
    cam_interpolation: Option<CameraInterpolation>,

    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_obj_path: Option<ObjPath>,
    /// Where in world space the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_point: Option<glam::Vec3>,

    /// Estimate of the the bounding box of all data. Accumulated.
    #[serde(skip)]
    scene_bbox: macaw::BoundingBox,

    // options:
    spin: bool,
    show_axes: bool,
}

impl Default for State3D {
    fn default() -> Self {
        Self {
            orbit_camera: Default::default(),
            cam_interpolation: Default::default(),
            hovered_obj_path: Default::default(),
            hovered_point: Default::default(),
            scene_bbox: macaw::BoundingBox::nothing(),
            spin: false,
            show_axes: false,
        }
    }
}

impl State3D {
    fn update_camera(
        &mut self,
        context: &mut ViewerContext,
        tracking_camera: Option<Camera>,
        response: &egui::Response,
        space_specs: &SpaceSpecs,
    ) -> Camera {
        if response.double_clicked() {
            // Reset camera
            if tracking_camera.is_some() {
                context.selection = Selection::None;
            }
            self.interpolate_to_orbit_camera(default_camera(&self.scene_bbox, space_specs));
        }

        let orbit_camera = self
            .orbit_camera
            .get_or_insert_with(|| default_camera(&self.scene_bbox, space_specs));

        if let Some(tracking_camera) = tracking_camera {
            orbit_camera.copy_from_camera(&tracking_camera);
            self.cam_interpolation = None;
        }

        if self.spin {
            orbit_camera.rotate(egui::vec2(
                -response.ctx.input().stable_dt.at_most(0.1) * 150.0,
                0.0,
            ));
            response.ctx.request_repaint();
        }

        if let Some(cam_interpolation) = &mut self.cam_interpolation {
            if cam_interpolation.elapsed_time < cam_interpolation.target_time {
                cam_interpolation.elapsed_time += response.ctx.input().stable_dt.at_most(0.1);
                response.ctx.request_repaint();
                let t = cam_interpolation.elapsed_time / cam_interpolation.target_time;
                let t = t.clamp(0.0, 1.0);
                let t = crate::math::ease_out(t);
                if let Some(target_orbit) = &cam_interpolation.target_orbit {
                    *orbit_camera = cam_interpolation.start.lerp(target_orbit, t);
                } else if let Some(target_camera) = &cam_interpolation.target_camera {
                    let camera = cam_interpolation.start.to_camera().lerp(target_camera, t);
                    orbit_camera.copy_from_camera(&camera);
                } else {
                    self.cam_interpolation = None;
                }
            }
        }

        // interact with orbit camera:
        {
            let mut did_interact = false;

            if response.dragged_by(egui::PointerButton::Primary) {
                orbit_camera.rotate(response.drag_delta());
                did_interact = true;
            } else if response.dragged_by(egui::PointerButton::Secondary) {
                orbit_camera.translate(response.drag_delta());
                did_interact = true;
            } else if response.dragged_by(egui::PointerButton::Middle) {
                if let Some(pointer_pos) = response.ctx.pointer_latest_pos() {
                    orbit_camera.roll(&response.rect, pointer_pos, response.drag_delta());
                    did_interact = true;
                }
            }

            if response.hovered() {
                orbit_camera.keyboard_navigation(&response.ctx);
                let input = response.ctx.input();

                let factor = input.zoom_delta() * (input.scroll_delta.y / 200.0).exp();
                if factor != 1.0 {
                    orbit_camera.radius /= factor;
                    did_interact = true;
                }
            }

            if did_interact {
                self.cam_interpolation = None;
                if tracking_camera.is_some() {
                    context.selection = Selection::None;
                }
            }
        }

        orbit_camera.to_camera()
    }

    fn interpolate_to_camera(&mut self, target: Camera) {
        if let Some(start) = self.orbit_camera {
            let target_time = CameraInterpolation::target_time(&start.to_camera(), &target);
            self.cam_interpolation = Some(CameraInterpolation {
                elapsed_time: 0.0,
                target_time,
                start,
                target_orbit: None,
                target_camera: Some(target),
            });
        } else {
            // self.orbit_camera = todo!()
        }
    }

    fn interpolate_to_orbit_camera(&mut self, target: OrbitCamera) {
        if let Some(start) = self.orbit_camera {
            let target_time =
                CameraInterpolation::target_time(&start.to_camera(), &target.to_camera());
            self.cam_interpolation = Some(CameraInterpolation {
                elapsed_time: 0.0,
                target_time,
                start,
                target_orbit: Some(target),
                target_camera: None,
            });
        } else {
            // self.orbit_camera = todo!()
        }
    }
}

struct CameraInterpolation {
    elapsed_time: f32,
    target_time: f32,
    start: OrbitCamera,
    target_orbit: Option<OrbitCamera>,
    target_camera: Option<Camera>,
}

impl CameraInterpolation {
    pub fn target_time(start: &Camera, stop: &Camera) -> f32 {
        // Take more time if the rotation is big:
        let angle_difference = start
            .world_from_view
            .rotation()
            .angle_between(stop.world_from_view.rotation());

        egui::remap_clamp(angle_difference, 0.0..=std::f32::consts::PI, 0.2..=0.7)
    }
}

fn show_settings_ui(
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    state: &mut State3D,
    space: Option<&ObjPath>,
    space_specs: &SpaceSpecs,
) {
    ui.horizontal(|ui| {
        {
            let up = space_specs.up.normalize_or_zero();

            let up_response = if up == Vec3::X {
                ui.label("Up: +X")
            } else if up == -Vec3::X {
                ui.label("Up: -X")
            } else if up == Vec3::Y {
                ui.label("Up: +Y")
            } else if up == -Vec3::Y {
                ui.label("Up: -Y")
            } else if up == Vec3::Z {
                ui.label("Up: +Z")
            } else if up == -Vec3::Z {
                ui.label("Up: -Z")
            } else if up != Vec3::ZERO {
                ui.label(format!("Up: [{:.3} {:.3} {:.3}]", up.x, up.y, up.z))
            } else {
                ui.label("Up: unspecified")
            };
            if let Some(space) = space {
                up_response.on_hover_ui(|ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.label("Set by logging to ");
                        ui.code(format!("{space}/up"));
                        ui.label(".");
                    });
                });
            }
        }

        if ui
            .button("Reset camera")
            .on_hover_text("You can also double-click the 3D view")
            .clicked()
        {
            state.orbit_camera = Some(default_camera(&state.scene_bbox, space_specs));
            state.cam_interpolation = None;
            // TODO(emilk): reset tracking camera too
        }

        // TODO(emilk): only show if there is a camera om scene.
        ui.toggle_value(&mut context.options.show_camera_mesh_in_3d, "📷")
            .on_hover_text("Show camera mesh");

        ui.toggle_value(&mut state.spin, "Spin")
            .on_hover_text("Spin camera");
        ui.toggle_value(&mut state.show_axes, "Axes")
            .on_hover_text("Show X-Y-Z axes");

        crate::misc::help_hover_button(ui).on_hover_text(
            "Drag to rotate.\n\
            Drag with secondary mouse button to pan.\n\
            Drag with middle mouse button to roll camera.\n\
            {}\
            Scroll to zoom.\n\
            \n\
            While hovering the 3D view, navigate camera with WSAD and QE.\n\
            CTRL slows down, SHIFT speeds up.\n\
            \n\
            Click on a object to focus the camera on it.\n\
            \n\
            Double-click anywhere to reset camera.",
        );
    });
}

#[derive(Default)]
struct SpaceSpecs {
    /// ZERO = unset
    up: glam::Vec3,
}

impl SpaceSpecs {
    fn from_objects(space: Option<&ObjPath>, objects: &re_data_store::Objects<'_>) -> Self {
        if let Some(space) = space {
            if let Some(space) = objects.space.get(&space) {
                return SpaceSpecs {
                    up: Vec3::from(*space.up).normalize_or_zero(),
                };
            }
        }
        Default::default()
    }
}

/// If the path to a camera is selected, we follow that camera.
fn tracking_camera(
    log_db: &LogDb,
    context: &ViewerContext,
    objects: &re_data_store::Objects<'_>,
) -> Option<Camera> {
    if let Selection::DataPath(data_path) = &context.selection {
        let mut selected_camera = None;

        for (props, camera) in objects.camera.iter() {
            if let Some(msg) = log_db.get_data_msg(props.log_id) {
                if &msg.data_path == data_path {
                    if selected_camera.is_some() {
                        return None; // More than one camera
                    } else {
                        selected_camera = Some(camera.camera);
                    }
                }
            }
        }

        selected_camera.map(Camera::from_camera_data)
    } else {
        None
    }
}

fn click_object(
    log_db: &LogDb,
    context: &mut ViewerContext,
    state: &mut State3D,
    obj_path: &ObjPath,
) {
    context.selection = crate::Selection::ObjPath(obj_path.clone());

    if log_db.object_types.get(obj_path.obj_type_path()) == Some(&re_log_types::ObjectType::Camera)
    {
        if let Some((_, data_store)) = log_db.data_store.get(context.time_control.source()) {
            if let Some(obj_store) = data_store.get(obj_path.obj_type_path()) {
                // TODO(emilk): use the time of what we clicked instead!
                if let Some(time_query) = context.time_control.time_query() {
                    let mut objects = re_data_store::Objects::default();
                    re_data_store::objects::Camera::query_obj_path(
                        obj_store,
                        obj_path,
                        &time_query,
                        &mut objects,
                    );
                    if let Some((_, camera)) = objects.camera.last() {
                        state.interpolate_to_camera(Camera::from_camera_data(camera.camera));
                        return;
                    }
                }
            }
        }
    }

    if let Some(clicked_point) = state.hovered_point {
        // center camera on what we click on
        if let Some(mut new_orbit_cam) = state.orbit_camera {
            new_orbit_cam.radius = new_orbit_cam.position().distance(clicked_point);
            new_orbit_cam.center = clicked_point;
            state.interpolate_to_orbit_camera(new_orbit_cam);
        }
    }
}

pub(crate) fn combined_view_3d(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    state: &mut State3D,
    space: Option<&ObjPath>,
    objects: &re_data_store::Objects<'_>,
) {
    crate::profile_function!();

    state.scene_bbox = state.scene_bbox.union(crate::misc::calc_bbox_3d(objects));

    let space_specs = SpaceSpecs::from_objects(space, objects);

    // TODO(emilk): show settings on top of 3D view.
    // Requires some egui work to handle interaction of overlapping widgets.
    show_settings_ui(context, ui, state, space, &space_specs);

    let (rect, response) = ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    let tracking_camera = tracking_camera(log_db, context, objects);
    let camera = state.update_camera(context, tracking_camera, &response, &space_specs);

    let mut hovered_obj_path = state.hovered_obj_path.clone();
    if ui.input().pointer.any_click() {
        if let Some(hovered_obj_path) = &hovered_obj_path {
            click_object(log_db, context, state, hovered_obj_path);
        }
    } else if ui.input().pointer.any_down() {
        hovered_obj_path = None;
    }

    if let Some(obj_path) = &hovered_obj_path {
        egui::containers::popup::show_tooltip_at_pointer(
            ui.ctx(),
            egui::Id::new("3d_tooltip"),
            |ui| {
                context.obj_path_button(ui, obj_path);
                crate::ui::context_panel::view_object(log_db, context, ui, obj_path);
            },
        );
    }

    let scene = Scene::from_objects(
        context,
        &state.scene_bbox,
        rect.size(),
        &camera,
        hovered_obj_path.as_ref(),
        objects,
    );

    let hovered = response
        .hover_pos()
        .and_then(|pointer_pos| scene.picking(pointer_pos, &rect, &camera));
    if let Some((obj_path_hash, point)) = hovered {
        state.hovered_obj_path = log_db
            .data_store
            .obj_path_from_hash(&obj_path_hash)
            .cloned();
        state.hovered_point = Some(point);
    } else {
        state.hovered_obj_path = None;
        state.hovered_point = None;
    }

    let dark_mode = ui.visuals().dark_mode;
    let show_axes = state.show_axes;

    let callback = egui::PaintCallback {
        rect,
        callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
            with_three_d_context(painter.gl(), |rendering| {
                paint_with_three_d(rendering, &camera, &info, &scene, dark_mode, show_axes)
                    .unwrap();
            });
        })),
    };
    ui.painter().add(callback);
}

fn default_camera(scene_bbox: &macaw::BoundingBox, space_spects: &SpaceSpecs) -> OrbitCamera {
    let mut center = scene_bbox.center();
    if !center.is_finite() {
        center = Vec3::ZERO;
    }

    let mut radius = 2.0 * scene_bbox.half_size().length();
    if !radius.is_finite() || radius == 0.0 {
        radius = 1.0;
    }

    let cam_dir = vec3(1.0, 1.0, 0.5).normalize();
    let camera_pos = center + radius * cam_dir;

    let look_up = if space_spects.up == Vec3::ZERO {
        Vec3::Z // TODO(emilk): make sure this isn't colinear with `center - camera_pos`.
    } else {
        space_spects.up
    };

    OrbitCamera {
        center,
        radius,
        world_from_view_rot: Quat::from_affine3(
            &Affine3A::look_at_rh(camera_pos, center, look_up).inverse(),
        ),
        fov_y: camera::DEFAULT_FOV_Y,
        up: space_spects.up,
        velocity: Vec3::ZERO,
    }
}

/// We get a [`glow::Context`] from `eframe`, but we want a [`three_d::Context`].
///
/// Sadly we can't just create and store a [`three_d::Context`] in the app and pass it
/// to the [`egui::PaintCallback`] because [`three_d::Context`] isn't `Send+Sync`, which
/// [`egui::PaintCallback`] is.
fn with_three_d_context<R>(
    gl: &std::sync::Arc<glow::Context>,
    f: impl FnOnce(&mut RenderingContext) -> R,
) -> R {
    use std::cell::RefCell;
    thread_local! {
        static THREE_D: RefCell<Option<RenderingContext>> = RefCell::new(None);
    }

    #[allow(unsafe_code)]
    // SAFETY: we should have a valid glow context here, and we _should_ be in the correct thread.
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
