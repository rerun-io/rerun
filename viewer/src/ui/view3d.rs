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
use log_types::{Data, LogId, ObjPath};
use macaw::{vec3, Quat, Vec3};

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
    hovered: Option<LogId>,

    /// Estimate of the the bounding box of all data. Accumulated.
    #[serde(skip)]
    scene_bbox: macaw::BoundingBox,
}

impl Default for State3D {
    fn default() -> Self {
        Self {
            orbit_camera: Default::default(),
            cam_interpolation: Default::default(),
            hovered: Default::default(),
            scene_bbox: macaw::BoundingBox::nothing(),
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
            if self.cam_interpolation.is_none() {
                orbit_camera.set_up(space_specs.up);
            }

            let mut did_interact = false;

            if response.dragged_by(egui::PointerButton::Primary) {
                orbit_camera.rotate(response.drag_delta());
                did_interact = true;
            } else if response.dragged_by(egui::PointerButton::Secondary) {
                orbit_camera.translate(response.drag_delta());
                did_interact = true;
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
            // self.orbit_camera = TODO
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
            // self.orbit_camera = TODO
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
    state_3d: &mut State3D,
    space: &ObjPath,
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
            up_response.on_hover_ui(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Set by logging to ");
                    ui.code(format!("{space}/up"));
                    ui.label(".");
                });
            });
        }

        if ui
            .button("Reset camera")
            .on_hover_text("You can also double-click the 3D view")
            .clicked()
        {
            state_3d.orbit_camera = Some(default_camera(&state_3d.scene_bbox, space_specs));
            state_3d.cam_interpolation = None;
            // TODO: reset tracking camera too
        }

        // TODO: only show if there is a camera om scene.
        ui.toggle_value(&mut context.options.show_camera_mesh_in_3d, "📷")
            .on_hover_text("Show camera mesh");

        crate::misc::help_hover_button(ui).on_hover_text(
            "Drag to rotate.\n\
            Drag with secondary mouse button to pan.\n\
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
    fn from_objects(space: &ObjPath, objects: &data_store::Objects<'_>) -> Self {
        if let Some(space) = objects.space.get(space) {
            SpaceSpecs {
                up: Vec3::from(*space.up).normalize_or_zero(),
            }
        } else {
            Default::default()
        }
    }
}

/// If the path to a camera is selected, we follow that camera.
fn tracking_camera(
    log_db: &LogDb,
    context: &ViewerContext,
    objects: &data_store::Objects<'_>,
) -> Option<Camera> {
    if let Selection::DataPath(data_path) = &context.selection {
        let mut selected_camera = None;

        for (_, props, camera) in objects.camera.iter() {
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

pub(crate) fn combined_view_3d(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    state: &mut State3D,
    space: &ObjPath,
    objects: &data_store::Objects<'_>,
) {
    crate::profile_function!();

    state.scene_bbox = state.scene_bbox.union(crate::misc::calc_bbox_3d(objects));

    let space_specs = SpaceSpecs::from_objects(space, objects);

    // TODO: show settings on top of 3D view.
    // Requires some egui work to handle interaction of overlapping widgets.
    show_settings_ui(context, ui, state, space, &space_specs);

    let (rect, response) = ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    let tracking_camera = tracking_camera(log_db, context, objects);
    let camera = state.update_camera(context, tracking_camera, &response, &space_specs);

    let mut hovered_id = state.hovered;
    if ui.input().pointer.any_click() {
        if let Some(clicked_id) = hovered_id {
            if let Some(msg) = log_db.get_data_msg(&clicked_id) {
                context.selection = crate::Selection::LogId(clicked_id);
                if let Data::Camera(cam) = &msg.data {
                    state.interpolate_to_camera(Camera::from_camera_data(cam));
                } else if let Some(center) = msg.data.center3d() {
                    // center camera on what we click on
                    // TODO: center on where you clicked instead of the centroid of the data
                    if let Some(mut new_orbit_cam) = state.orbit_camera {
                        let center = Vec3::from(center);
                        new_orbit_cam.radius = new_orbit_cam.position().distance(center);
                        new_orbit_cam.center = center;
                        state.interpolate_to_orbit_camera(new_orbit_cam);
                    }
                }
            }
        }
    } else if ui.input().pointer.any_down() {
        hovered_id = None;
    }

    if let Some(hovered_id) = hovered_id {
        if let Some(msg) = log_db.get_data_msg(&hovered_id) {
            egui::containers::popup::show_tooltip_at_pointer(
                ui.ctx(),
                egui::Id::new("3d_tooltip"),
                |ui| {
                    crate::view2d::on_hover_ui(context, ui, msg);
                },
            );
        }
    }

    let scene = Scene::from_objects(
        context,
        &state.scene_bbox,
        rect.size(),
        &camera,
        hovered_id.as_ref(),
        objects,
    );

    state.hovered = response
        .hover_pos()
        .and_then(|pointer_pos| scene.picking(pointer_pos, &rect, &camera));

    let dark_mode = ui.visuals().dark_mode;

    let callback = egui::PaintCallback {
        rect,
        callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
            with_three_d_context(painter.gl(), |rendering| {
                paint_with_three_d(rendering, &camera, &info, &scene, dark_mode).unwrap();
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
        Vec3::Z
    } else {
        space_spects.up
    };

    OrbitCamera {
        center,
        radius,
        world_from_view_rot: Quat::from_affine3(
            &Affine3A::look_at_rh(camera_pos, center, look_up).inverse(),
        ),
        fov_y: 65.0_f32.to_radians(), // TODO: base on viewport size?
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
