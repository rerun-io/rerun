mod eye;
mod scene;

#[cfg(feature = "glow")]
mod glow_rendering;

#[cfg(feature = "glow")]
mod mesh_cache;

#[cfg(feature = "glow")]
pub use mesh_cache::CpuMeshCache;

use eye::*;
use re_data_store::{InstanceId, InstanceIdHash};
use scene::*;

use egui::NumExt as _;
use glam::Affine3A;
use macaw::{vec3, Quat, Ray3, Vec3};
use re_log_types::ObjPath;

use crate::{
    misc::{HoveredSpace, Selection},
    ViewerContext,
};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct State3D {
    orbit_eye: Option<OrbitEye>,

    #[serde(skip)]
    eye_interpolation: Option<EyeInterpolation>,

    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_instance: Option<InstanceId>,

    /// Where in world space the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_point: Option<glam::Vec3>,

    /// Estimate of the the bounding box of all data. Accumulated.
    #[serde(skip)]
    scene_bbox: macaw::BoundingBox,

    // options:
    spin: bool,
    show_axes: bool,

    last_eye_interact_time: f64,
}

impl Default for State3D {
    fn default() -> Self {
        Self {
            orbit_eye: Default::default(),
            eye_interpolation: Default::default(),
            hovered_instance: Default::default(),
            hovered_point: Default::default(),
            scene_bbox: macaw::BoundingBox::nothing(),
            spin: false,
            show_axes: false,
            last_eye_interact_time: f64::NEG_INFINITY,
        }
    }
}

impl State3D {
    fn update_eye(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        tracking_camera: Option<Eye>,
        response: &egui::Response,
        space_specs: &SpaceSpecs,
    ) -> &mut OrbitEye {
        if response.double_clicked() {
            // Reset camera
            if tracking_camera.is_some() {
                ctx.rec_cfg.selection = Selection::None;
            }
            self.interpolate_to_orbit_eye(default_eye(&self.scene_bbox, space_specs));
        }

        if let Some(tracking_camera) = tracking_camera {
            if let Some(cam_interpolation) = &mut self.eye_interpolation {
                // Update interpolation target:
                cam_interpolation.target_orbit = None;
                if cam_interpolation.target_eye != Some(tracking_camera) {
                    cam_interpolation.target_eye = Some(tracking_camera);
                    response.ctx.request_repaint();
                }
            } else {
                self.interpolate_to_eye(tracking_camera);
            }
        }

        let orbit_camera = self
            .orbit_eye
            .get_or_insert_with(|| default_eye(&self.scene_bbox, space_specs));

        if self.spin {
            orbit_camera.rotate(egui::vec2(
                -response.ctx.input().stable_dt.at_most(0.1) * 150.0,
                0.0,
            ));
            response.ctx.request_repaint();
        }

        if let Some(cam_interpolation) = &mut self.eye_interpolation {
            cam_interpolation.elapsed_time += response.ctx.input().stable_dt.at_most(0.1);

            let t = cam_interpolation.elapsed_time / cam_interpolation.target_time;
            let t = t.clamp(0.0, 1.0);
            let t = crate::math::ease_out(t);

            if t < 1.0 {
                response.ctx.request_repaint();
            }

            if let Some(target_orbit) = &cam_interpolation.target_orbit {
                *orbit_camera = cam_interpolation.start.lerp(target_orbit, t);
            } else if let Some(target_camera) = &cam_interpolation.target_eye {
                let camera = cam_interpolation.start.to_eye().lerp(target_camera, t);
                orbit_camera.copy_from_eye(&camera);
            } else {
                self.eye_interpolation = None;
            }
        }

        orbit_camera
    }

    fn interpolate_to_eye(&mut self, target: Eye) {
        if let Some(start) = self.orbit_eye {
            let target_time = EyeInterpolation::target_time(&start.to_eye(), &target);
            self.eye_interpolation = Some(EyeInterpolation {
                elapsed_time: 0.0,
                target_time,
                start,
                target_orbit: None,
                target_eye: Some(target),
            });
        } else {
            // shouldn't really happen (`self.orbit_eye` is only `None` for the first frame).
        }
    }

    fn interpolate_to_orbit_eye(&mut self, target: OrbitEye) {
        if let Some(start) = self.orbit_eye {
            let target_time = EyeInterpolation::target_time(&start.to_eye(), &target.to_eye());
            self.eye_interpolation = Some(EyeInterpolation {
                elapsed_time: 0.0,
                target_time,
                start,
                target_orbit: Some(target),
                target_eye: None,
            });
        } else {
            self.orbit_eye = Some(target);
        }
    }
}

struct EyeInterpolation {
    elapsed_time: f32,
    target_time: f32,
    start: OrbitEye,
    target_orbit: Option<OrbitEye>,
    target_eye: Option<Eye>,
}

impl EyeInterpolation {
    pub fn target_time(start: &Eye, stop: &Eye) -> f32 {
        // Take more time if the rotation is big:
        let angle_difference = start
            .world_from_view
            .rotation()
            .angle_between(stop.world_from_view.rotation());

        egui::remap_clamp(angle_difference, 0.0..=std::f32::consts::PI, 0.2..=0.7)
    }
}

fn show_settings_ui(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut State3D,
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
                ui.label("Up: —")
            };

            up_response.on_hover_ui(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Set with ");
                    ui.code("rerun.set_space_up");
                    ui.label(".");
                });
            });
        }

        if ui
            .button("Reset view")
            .on_hover_text("You can also double-click the 3D view")
            .clicked()
        {
            state.orbit_eye = Some(default_eye(&state.scene_bbox, space_specs));
            state.eye_interpolation = None;
            // TODO(emilk): reset tracking camera too
        }

        // TODO(emilk): only show if there is a camera om scene.
        ui.toggle_value(&mut ctx.options.show_camera_mesh_in_3d, "📷")
            .on_hover_text("Show camera mesh");

        ui.toggle_value(&mut state.spin, "Spin")
            .on_hover_text("Spin view");
        ui.toggle_value(&mut state.show_axes, "Axes")
            .on_hover_text("Show X-Y-Z axes");

        crate::misc::help_hover_button(ui).on_hover_text(
            "Drag to rotate.\n\
            Drag with secondary mouse button to pan.\n\
            Drag with middle mouse button to roll the view.\n\
            Scroll to zoom.\n\
            \n\
            While hovering the 3D view, navigate with WSAD and QE.\n\
            CTRL slows down, SHIFT speeds up.\n\
            \n\
            Click on a object to focus the view on it.\n\
            \n\
            Double-click anywhere to reset the view.",
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
fn tracking_camera(ctx: &ViewerContext<'_>, objects: &re_data_store::Objects<'_>) -> Option<Eye> {
    if let Selection::Instance(selected) = &ctx.rec_cfg.selection {
        find_camera(objects, selected)
    } else {
        None
    }
}

fn find_camera(objects: &re_data_store::Objects<'_>, needle: &InstanceId) -> Option<Eye> {
    let mut found_camera = None;

    for (props, camera) in objects.camera.iter() {
        if needle.is_instance(props) {
            if found_camera.is_some() {
                return None; // More than one camera
            } else {
                found_camera = Some(camera.camera);
            }
        }
    }

    found_camera.map(Eye::from_camera)
}

fn click_object(
    ctx: &mut ViewerContext<'_>,
    objects: &re_data_store::Objects<'_>,
    state: &mut State3D,
    instance_id: &InstanceId,
) {
    ctx.rec_cfg.selection = crate::Selection::Instance(instance_id.clone());

    if let Some(camera) = find_camera(objects, instance_id) {
        state.interpolate_to_eye(camera);
    } else if let Some(clicked_point) = state.hovered_point {
        // center camera on what we click on
        if let Some(mut new_orbit_eye) = state.orbit_eye {
            new_orbit_eye.orbit_radius = new_orbit_eye.position().distance(clicked_point);
            new_orbit_eye.orbit_center = clicked_point;
            state.interpolate_to_orbit_eye(new_orbit_eye);
        }
    }
}

pub(crate) fn view_3d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut State3D,
    space: Option<&ObjPath>,
    objects: &re_data_store::Objects<'_>,
) -> egui::Response {
    crate::profile_function!();

    state.scene_bbox = state.scene_bbox.union(crate::misc::calc_bbox_3d(objects));

    let space_specs = SpaceSpecs::from_objects(space, objects);

    // TODO(emilk): show settings on top of 3D view.
    // Requires some egui work to handle interaction of overlapping widgets.
    show_settings_ui(ctx, ui, state, &space_specs);

    let (rect, response) = ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    let tracking_camera = tracking_camera(ctx, objects);
    let orbit_eye = state.update_eye(ctx, tracking_camera, &response, &space_specs);

    let did_interact_wth_eye = orbit_eye.interact(&response);
    let orbit_eye = *orbit_eye;
    let eye = orbit_eye.to_eye();
    if did_interact_wth_eye {
        state.last_eye_interact_time = ui.input().time;
        state.eye_interpolation = None;
        if tracking_camera.is_some() {
            ctx.rec_cfg.selection = Selection::None;
        }
    }

    let mut hovered_instance = state.hovered_instance.clone();
    if ui.input().pointer.any_click() {
        if let Some(hovered_instance) = &hovered_instance {
            click_object(ctx, objects, state, hovered_instance);
        }
    } else if ui.input().pointer.any_down() {
        hovered_instance = None;
    }

    if let Some(instance_id) = &hovered_instance {
        egui::containers::popup::show_tooltip_at_pointer(
            ui.ctx(),
            egui::Id::new("3d_tooltip"),
            |ui| {
                ctx.instance_id_button(ui, instance_id);
                crate::ui::data_ui::view_instance(ctx, ui, instance_id, crate::ui::Preview::Medium);
            },
        );
    }

    let mut scene = Scene::from_objects(
        ctx,
        &state.scene_bbox,
        rect.size(),
        &eye,
        hovered_instance.as_ref(),
        objects,
    );

    let hovered = response
        .hover_pos()
        .and_then(|pointer_pos| scene.picking(pointer_pos, &rect, &eye));

    state.hovered_instance = None;
    state.hovered_point = None;
    if let Some((instance_id, point)) = hovered {
        if let Some(instance_id) = instance_id.resolve(&ctx.log_db.obj_db.store) {
            state.hovered_instance = Some(instance_id);
            state.hovered_point = Some(point);
        }
    }

    project_onto_other_spaces(ctx, state, space, &response, orbit_eye, objects);
    show_projections_from_2d_space(ctx, objects, state, orbit_eye, &mut scene);

    // Draw labels
    let ui_from_world = eye.ui_from_world(&rect);
    ui.with_layer_id(
        egui::LayerId::new(egui::Order::Foreground, egui::Id::new("LabelsLayer")),
        |ui| {
            for label in &scene.labels {
                let pt = ui_from_world.project_point3(label.origin);
                let font_id = egui::TextStyle::Monospace.resolve(ui.style());

                let galley = ui.fonts().layout(
                    (*label.text).to_owned(),
                    font_id,
                    ui.style().visuals.text_color(),
                    100.0,
                );

                let text_rect = egui::Align2::CENTER_TOP.anchor_rect(egui::Rect::from_min_size(
                    egui::pos2(pt.x, pt.y),
                    galley.size(),
                ));

                let bg_rect = text_rect.expand2(egui::vec2(6.0, 2.0));
                ui.painter().add(egui::Shape::rect_filled(
                    bg_rect,
                    3.0,
                    ui.style().visuals.code_bg_color,
                ));
                ui.painter().add(egui::Shape::galley(text_rect.min, galley));
            }
        },
    );

    {
        let orbit_center_alpha = egui::remap_clamp(
            ui.input().time - state.last_eye_interact_time,
            0.0..=0.4,
            0.7..=0.0,
        ) as f32;

        if orbit_center_alpha > 0.0 {
            // Show center of orbit camera when interacting with camera (it's quite helpful).
            scene.points.push(Point {
                instance_id: InstanceIdHash::NONE,
                pos: orbit_eye.orbit_center.to_array(),
                radius: orbit_eye.orbit_radius * 0.01,
                color: [255, 0, 255, (orbit_center_alpha * 255.0) as u8],
            });
            ui.ctx().request_repaint(); // let it fade out
        }
    }

    let dark_mode = ui.visuals().dark_mode;
    let show_axes = state.show_axes;

    let callback = egui::PaintCallback {
        rect,
        callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |info, painter| {
            glow_rendering::with_three_d_context(painter.gl(), |rendering| {
                glow_rendering::paint_with_three_d(
                    rendering, &eye, &info, &scene, dark_mode, show_axes, painter,
                );
            });
        })),
    };
    ui.painter().add(callback);

    response
}

fn show_projections_from_2d_space(
    ctx: &mut ViewerContext<'_>,
    objects: &re_data_store::Objects<'_>,
    state: &mut State3D,
    orbit_eye: OrbitEye,
    scene: &mut Scene,
) {
    let eye_pos = orbit_eye.position();

    if let HoveredSpace::TwoD { space_2d, pos } = &ctx.rec_cfg.hovered_space {
        for (_, cam) in objects.camera.iter() {
            let cam = cam.camera;
            if &cam.target_space == space_2d {
                if let Some(ray) = crate::misc::cam::unproject_as_ray(cam, glam::vec2(pos.x, pos.y))
                {
                    // TODO(emilk): better visualization of a ray
                    let mut hit_pos = None;
                    if pos.z.is_finite() && pos.z > 0.0 {
                        if let Some(world_from_image) = crate::misc::cam::world_from_image(cam) {
                            let pos = world_from_image
                                .transform_point3(glam::vec3(pos.x, pos.y, 1.0) * pos.z);
                            hit_pos = Some(pos);
                        }
                    }
                    let length = if let Some(hit_pos) = hit_pos {
                        hit_pos.distance(Vec3::from_slice(&cam.extrinsics.position))
                    } else {
                        4.0 * state.scene_bbox.half_size().length() // should be long enough
                    };
                    let origin = ray.point_along(0.0);
                    let end = ray.point_along(length);
                    let distance =
                        crate::math::line_segment_distance_to_point_3d([origin, end], eye_pos);
                    let radius = 2.0 * scene.line_radius_from_distance * distance;
                    scene.line_segments.push(LineSegments {
                        instance_id: InstanceIdHash::NONE,
                        segments: vec![[origin.into(), end.into()]],
                        radius,
                        color: [255; 4],
                    });

                    if let Some(pos) = hit_pos {
                        // Show where the ray hits the depth map:
                        scene.points.push(Point {
                            instance_id: InstanceIdHash::NONE,
                            pos: pos.into(),
                            radius: radius * 3.0,
                            color: [255; 4],
                        });
                    }
                }
            }
        }
    }
}

fn project_onto_other_spaces(
    ctx: &mut ViewerContext<'_>,
    state: &mut State3D,
    space: Option<&ObjPath>,
    response: &egui::Response,
    orbit_eye: OrbitEye,
    objects: &re_data_store::Objects<'_>,
) {
    if let Some(pos_in_ui) = response.hover_pos() {
        let ray_in_world = {
            let eye = orbit_eye.to_eye();
            let world_from_ui = eye.world_from_ui(&response.rect);
            let ray_origin = eye.pos_in_world();
            let ray_dir = world_from_ui.project_point3(glam::vec3(pos_in_ui.x, pos_in_ui.y, 1.0))
                - ray_origin;
            Ray3::from_origin_dir(ray_origin, ray_dir.normalize())
        };

        let mut target_spaces = vec![];
        for (_, cam) in objects.camera.iter() {
            let cam = cam.camera;
            if let Some(target_space) = cam.target_space.clone() {
                let ray_in_2d = crate::misc::cam::image_from_world(cam)
                    .map(|image_from_world| (image_from_world * ray_in_world).normalize());

                let point_in_2d = state.hovered_point.and_then(|hovered_point| {
                    crate::misc::cam::project_onto_2d(cam, hovered_point)
                });

                target_spaces.push((target_space, ray_in_2d, point_in_2d));
            }
        }
        ctx.rec_cfg.hovered_space = HoveredSpace::ThreeD {
            space_3d: space.cloned(),
            target_spaces,
        }
    }
}

fn default_eye(scene_bbox: &macaw::BoundingBox, space_spects: &SpaceSpecs) -> OrbitEye {
    let mut center = scene_bbox.center();
    if !center.is_finite() {
        center = Vec3::ZERO;
    }

    let mut radius = 2.0 * scene_bbox.half_size().length();
    if !radius.is_finite() || radius == 0.0 {
        radius = 1.0;
    }

    let look_up = if space_spects.up == Vec3::ZERO {
        Vec3::Z
    } else {
        space_spects.up.normalize()
    };

    // Look along the cardinal directions:
    let look_dir = vec3(1.0, 1.0, 1.0);
    // Make sure the eye is looking down, but just slightly:
    let look_dir = look_dir + look_up * (-0.5 - look_dir.dot(look_up));
    let look_dir = look_dir.normalize();

    let eye_pos = center - radius * look_dir;

    OrbitEye {
        orbit_center: center,
        orbit_radius: radius,
        world_from_view_rot: Quat::from_affine3(
            &Affine3A::look_at_rh(eye_pos, center, look_up).inverse(),
        ),
        fov_y: eye::DEFAULT_FOV_Y,
        up: space_spects.up,
        velocity: Vec3::ZERO,
    }
}
