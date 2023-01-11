use egui::NumExt as _;
use glam::Affine3A;
use macaw::{vec3, BoundingBox, Quat, Vec3};

use re_data_store::{InstanceId, InstanceIdHash, ObjectsProperties};
use re_log_types::{ObjPath, ViewCoordinates};
use re_renderer::{
    view_builder::{Projection, TargetConfiguration},
    RenderContext, Size,
};

use crate::{
    misc::{HoveredSpace, Selection},
    ui::{
        data_ui::{self, DataUi},
        view_spatial::{
            scene::AdditionalPickingInfo,
            ui_renderer_bridge::{create_scene_paint_callback, get_viewport, ScreenBackground},
            SceneSpatial, SpaceCamera3D,
        },
        Preview,
    },
    ViewerContext,
};

use super::{
    eye::{Eye, OrbitEye},
    ViewSpatialState,
};

// ---

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct View3DState {
    orbit_eye: Option<OrbitEye>,

    #[serde(skip)]
    eye_interpolation: Option<EyeInterpolation>,

    /// Where in world space the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_point: Option<glam::Vec3>,

    // options:
    spin: bool,
    show_axes: bool,

    #[serde(skip)]
    last_eye_interact_time: f64,

    /// Filled in at the start of each frame
    #[serde(skip)]
    pub(crate) space_specs: SpaceSpecs,
    #[serde(skip)]
    space_camera: Vec<SpaceCamera3D>, // TODO(andreas): remove this once camera meshes are gone
}

impl Default for View3DState {
    fn default() -> Self {
        Self {
            orbit_eye: Default::default(),
            eye_interpolation: Default::default(),
            hovered_point: Default::default(),
            spin: false,
            show_axes: false,
            last_eye_interact_time: f64::NEG_INFINITY,
            space_specs: Default::default(),
            space_camera: Default::default(),
        }
    }
}

impl View3DState {
    fn update_eye(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        tracking_camera: Option<Eye>,
        response: &egui::Response,
        scene_bbox_accum: &BoundingBox,
    ) -> &mut OrbitEye {
        if response.double_clicked() {
            // Reset eye
            if tracking_camera.is_some() {
                ctx.clear_selection();
            }
            self.interpolate_to_orbit_eye(default_eye(scene_bbox_accum, &self.space_specs));
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
            .get_or_insert_with(|| default_eye(scene_bbox_accum, &self.space_specs));

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

    pub fn settings_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene_bbox_accum: &BoundingBox,
    ) {
        {
            let up_response = if let Some(up) = self.space_specs.up {
                if up == Vec3::X {
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
                }
            } else {
                ui.label("Up: —")
            };

            up_response.on_hover_ui(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("Set with ");
                    ui.code("rerun.log_view_coordinates");
                    ui.label(".");
                });
            });
        }

        if ui
            .button("Reset virtual camera")
            .on_hover_text(
                "Resets camera position & orientation.\nYou can also double-click the 3D view",
            )
            .clicked()
        {
            self.orbit_eye = Some(default_eye(scene_bbox_accum, &self.space_specs));
            self.eye_interpolation = None;
            // TODO(emilk): reset tracking camera too
        }

        ui.checkbox(&mut self.spin, "Spin virtual camera")
            .on_hover_text("Spin view");
        ui.checkbox(&mut self.show_axes, "Show origin axes")
            .on_hover_text("Show X-Y-Z axes");

        if !self.space_camera.is_empty() {
            ui.checkbox(
                &mut ctx.options.show_camera_mesh_in_3d,
                "Show camera meshes",
            );
        }
    }
}

#[derive(Clone)]
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

#[derive(Clone, Default)]
pub struct SpaceSpecs {
    up: Option<glam::Vec3>,
    right: Option<glam::Vec3>,
}

impl SpaceSpecs {
    pub fn from_view_coordinates(coordinates: Option<ViewCoordinates>) -> Self {
        let up = (|| Some(coordinates?.up()?.as_vec3().into()))();
        let right = (|| Some(coordinates?.right()?.as_vec3().into()))();

        Self { up, right }
    }
}

/// If the path to a camera is selected, we follow that camera.
fn tracking_camera(ctx: &ViewerContext<'_>, space_cameras: &[SpaceCamera3D]) -> Option<Eye> {
    if let Selection::Instance(selected) = ctx.selection() {
        find_camera(space_cameras, &selected)
    } else {
        None
    }
}

fn find_camera(space_cameras: &[SpaceCamera3D], needle: &InstanceId) -> Option<Eye> {
    let mut found_camera = None;

    for camera in space_cameras {
        if needle.obj_path == camera.camera_obj_path
            && camera.instance_index_hash == needle.instance_index_hash()
        {
            if found_camera.is_some() {
                return None; // More than one camera
            } else {
                found_camera = Some(camera);
            }
        }
    }

    found_camera.and_then(Eye::from_camera)
}

fn click_object(
    ctx: &mut ViewerContext<'_>,
    space_cameras: &[SpaceCamera3D],
    state: &mut View3DState,
    instance_id: &InstanceId,
) {
    ctx.set_selection(crate::Selection::Instance(instance_id.clone()));

    if let Some(camera) = find_camera(space_cameras, instance_id) {
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

// ----------------------------------------------------------------------------

pub const HELP_TEXT: &str = "Drag to rotate.\n\
    Drag with secondary mouse button to pan.\n\
    Drag with middle mouse button to roll the view.\n\
    Scroll to zoom.\n\
    \n\
    While hovering the 3D view, navigate with WSAD and QE.\n\
    CTRL slows down, SHIFT speeds up.\n\
    \n\
    Click on a object to focus the view on it.\n\
    \n\
    Double-click anywhere to reset the view.";

/// TODO(andreas): Split into smaller parts, more re-use with `ui_2d`
pub fn view_3d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewSpatialState,
    space: &ObjPath,
    mut scene: SceneSpatial,
    objects_properties: &ObjectsProperties,
) -> egui::Response {
    crate::profile_function!();

    state.state_3d.space_camera = scene.space_cameras.clone();

    let (rect, mut response) =
        ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    let tracking_camera = tracking_camera(ctx, &scene.space_cameras);
    let orbit_eye =
        state
            .state_3d
            .update_eye(ctx, tracking_camera, &response, &state.scene_bbox_accum);

    let did_interact_wth_eye = orbit_eye.interact(&response);
    let orbit_eye = *orbit_eye;
    let eye = orbit_eye.to_eye();

    // TODO: this should happen in the scene!
    {
        let hovered_instance_hash = state
            .hovered_instance
            .as_ref()
            .map_or(InstanceIdHash::NONE, |i| i.hash());
        scene.add_cameras(
            ctx,
            &state.scene_bbox_accum,
            rect.size(),
            &eye,
            hovered_instance_hash,
            objects_properties,
        );
    }

    if did_interact_wth_eye {
        state.state_3d.last_eye_interact_time = ui.input().time;
        state.state_3d.eye_interpolation = None;
        if tracking_camera.is_some() {
            ctx.clear_selection();
        }
    }

    // TODO(andreas): We're very close making the hover reaction of ui2d and ui3d the same. Finish the job!
    state.hovered_instance = None;
    if let Some(pointer_pos) = response.hover_pos() {
        let picking_result =
            scene.picking(glam::vec2(pointer_pos.x, pointer_pos.y), &rect, &eye, 5.0);

        for hit in picking_result.iter_hits() {
            let Some(instance_id) = hit.instance_hash.resolve(&ctx.log_db.obj_db)
            else { continue; };

            // Special hover ui for images.
            let picked_image_with_uv = if let AdditionalPickingInfo::TexturedRect(uv) = hit.info {
                scene
                    .ui
                    .images
                    .iter()
                    .find(|image| image.instance_hash == hit.instance_hash)
                    .map(|image| (image, uv))
            } else {
                None
            };
            response = if let Some((image, uv)) = picked_image_with_uv {
                response
                    .on_hover_cursor(egui::CursorIcon::ZoomIn)
                    .on_hover_ui_at_pointer(|ui| {
                        ui.set_max_width(400.0);

                        ui.vertical(|ui| {
                            ui.label(instance_id.to_string());
                            instance_id.data_ui(ctx, ui, Preview::Small);

                            let tensor_view = ctx.cache.image.get_view_with_annotations(
                                &image.tensor,
                                &image.annotations,
                                ctx.render_ctx,
                            );

                            if let [h, w, ..] = image.tensor.shape.as_slice() {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    let (w, h) = (w.size as f32, h.size as f32);
                                    let center = [(uv.x * w) as isize, (uv.y * h) as isize];
                                    data_ui::image::show_zoomed_image_region(
                                        ui,
                                        &tensor_view,
                                        center,
                                        image.meter,
                                    );
                                });
                            }
                        });
                    })
            } else {
                // Hover ui for everything else
                response.on_hover_ui_at_pointer(|ui| {
                    ctx.instance_id_button(ui, &instance_id);
                    instance_id.data_ui(ctx, ui, crate::ui::Preview::Medium);
                })
            };
        }

        if let Some(closest_pick) = picking_result.iter_hits().last() {
            // Save last known hovered object.
            if let Some(instance_id) = closest_pick.instance_hash.resolve(&ctx.log_db.obj_db) {
                state.state_3d.hovered_point = Some(picking_result.space_position(closest_pick));
                state.hovered_instance = Some(instance_id);
            }
        }

        // Clicking the last hovered object.
        if let Some(instance_id) = &state.hovered_instance {
            if ui.input().pointer.any_click() {
                click_object(ctx, &scene.space_cameras, &mut state.state_3d, instance_id);
            }
        }

        project_onto_other_spaces(ctx, &scene.space_cameras, &mut state.state_3d, space);
    }
    show_projections_from_2d_space(ctx, &mut scene, &state.scene_bbox_accum);
    if state.state_3d.show_axes {
        scene.primitives.add_axis_lines(
            macaw::IsoTransform::IDENTITY,
            InstanceIdHash::NONE,
            &eye,
            rect.size(),
        );
    }

    {
        let orbit_center_alpha = egui::remap_clamp(
            ui.input().time - state.state_3d.last_eye_interact_time,
            0.0..=0.4,
            0.7..=0.0,
        ) as f32;

        if orbit_center_alpha > 0.0 {
            // Show center of orbit camera when interacting with camera (it's quite helpful).
            let half_line_length = orbit_eye.orbit_radius * 0.03;

            scene
                .primitives
                .line_strips
                .batch("center orbit orientation help")
                .add_segments(glam::Vec3::AXES.iter().map(|axis| {
                    (
                        orbit_eye.orbit_center - *axis * half_line_length,
                        orbit_eye.orbit_center + *axis * half_line_length,
                    )
                }))
                .radius(Size::new_points(0.75))
                .flags(re_renderer::renderer::LineStripFlags::NO_COLOR_GRADIENT)
                // TODO(andreas): Fade this out.
                .color(re_renderer::Color32::WHITE);

            // TODO(andreas): Idea for nice depth perception:
            // Render the lines once with additive blending and depth test enabled
            // and another time without depth test. In both cases it needs to be rendered last,
            // something re_renderer doesn't support yet for primitives within renderers.

            ui.ctx().request_repaint(); // show it for a bit longer.
        }
    }

    paint_view(
        ui,
        eye,
        rect,
        &scene,
        ctx.render_ctx,
        &space.to_string(),
        state.auto_size_config(),
    );

    response
}

fn paint_view(
    ui: &mut egui::Ui,
    eye: Eye,
    rect: egui::Rect,
    scene: &SceneSpatial,
    render_ctx: &mut RenderContext,
    name: &str,
    auto_size_config: re_renderer::Size,
) {
    crate::profile_function!();

    // Draw labels:
    ui.with_layer_id(
        egui::LayerId::new(egui::Order::Foreground, egui::Id::new("LabelsLayer")),
        |ui| {
            crate::profile_function!("labels");
            let ui_from_world = eye.ui_from_world(&rect);
            for label in &scene.ui.labels_3d {
                let pos_in_ui = ui_from_world * label.origin.extend(1.0);
                if pos_in_ui.w <= 0.0 {
                    continue; // behind camera
                }
                let pos_in_ui = pos_in_ui / pos_in_ui.w;

                let font_id = egui::TextStyle::Monospace.resolve(ui.style());

                let galley = ui.fonts().layout(
                    (*label.text).to_owned(),
                    font_id,
                    ui.style().visuals.text_color(),
                    100.0,
                );

                let text_rect = egui::Align2::CENTER_TOP.anchor_rect(egui::Rect::from_min_size(
                    egui::pos2(pos_in_ui.x, pos_in_ui.y),
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

    // Determine view port resolution and position.
    let pixels_from_point = ui.ctx().pixels_per_point();
    let resolution_in_pixel = get_viewport(rect, pixels_from_point);
    if resolution_in_pixel[0] == 0 || resolution_in_pixel[1] == 0 {
        return;
    }
    let target_config = TargetConfiguration {
        name: name.into(),

        resolution_in_pixel,

        view_from_world: eye.world_from_view.inverse(),
        projection_from_view: Projection::Perspective {
            vertical_fov: eye.fov_y.unwrap(),
            near_plane_distance: eye.near(),
        },

        pixels_from_point,
        auto_size_config,
        auto_size_large_factor: 1.5,
    };

    let Ok(callback) = create_scene_paint_callback(
        render_ctx,
        target_config,
        rect,
        &scene.primitives, &ScreenBackground::GenericSkybox)
    else {
        return;
    };
    ui.painter().add(callback);
}

fn show_projections_from_2d_space(
    ctx: &mut ViewerContext<'_>,
    scene: &mut SceneSpatial,
    scene_bbox_accum: &BoundingBox,
) {
    if let HoveredSpace::TwoD { space_2d, pos } = &ctx.rec_cfg.hovered_space_previous_frame {
        let mut point_batch = scene
            .primitives
            .points
            .batch("projection from 2d hit points");

        for cam in &scene.space_cameras {
            if cam.target_space.as_ref() == Some(space_2d) {
                if let Some(ray) = cam.unproject_as_ray(glam::vec2(pos.x, pos.y)) {
                    // TODO(emilk): better visualization of a ray
                    let mut hit_pos = None;
                    if pos.z.is_finite() && pos.z > 0.0 {
                        if let Some(world_from_image) = cam.world_from_image() {
                            let pos = world_from_image
                                .transform_point3(glam::vec3(pos.x, pos.y, 1.0) * pos.z);
                            hit_pos = Some(pos);
                        }
                    }
                    let length = if let Some(hit_pos) = hit_pos {
                        hit_pos.distance(cam.position())
                    } else {
                        4.0 * scene_bbox_accum.half_size().length() // should be long enough
                    };
                    let origin = ray.point_along(0.0);
                    let end = ray.point_along(length);
                    let radius = Size::new_points(1.5);

                    scene
                        .primitives
                        .line_strips
                        .batch("ray")
                        .add_segment(origin, end)
                        .radius(radius);

                    if let Some(pos) = hit_pos {
                        // Show where the ray hits the depth map:
                        point_batch
                            .add_point(pos)
                            .radius(radius * 3.0)
                            .color(egui::Color32::WHITE);
                    }
                }
            }
        }
    }
}

fn project_onto_other_spaces(
    ctx: &mut ViewerContext<'_>,
    space_cameras: &[SpaceCamera3D],
    state: &mut View3DState,
    space: &ObjPath,
) {
    let mut target_spaces = vec![];
    for cam in space_cameras {
        if let Some(target_space) = cam.target_space.clone() {
            let point_in_2d = state
                .hovered_point
                .and_then(|hovered_point| cam.project_onto_2d(hovered_point));
            target_spaces.push((target_space, point_in_2d));
        }
    }
    ctx.rec_cfg.hovered_space_this_frame = HoveredSpace::ThreeD {
        space_3d: space.clone(),
        target_spaces,
    }
}

fn default_eye(scene_bbox: &macaw::BoundingBox, space_specs: &SpaceSpecs) -> OrbitEye {
    let mut center = scene_bbox.center();
    if !center.is_finite() {
        center = Vec3::ZERO;
    }

    let mut radius = 2.0 * scene_bbox.half_size().length();
    if !radius.is_finite() || radius == 0.0 {
        radius = 1.0;
    }

    let look_up = space_specs.up.unwrap_or(Vec3::Z);

    let look_dir = if let Some(right) = space_specs.right {
        // Make sure right is to the right, and up is up:
        let fwd = look_up.cross(right);
        0.75 * fwd + 0.25 * right - 0.25 * look_up
    } else {
        // Look along the cardinal directions:
        let look_dir = vec3(1.0, 1.0, 1.0);

        // Make sure the eye is looking down, but just slightly:
        look_dir + look_up * (-0.5 - look_dir.dot(look_up))
    };

    let look_dir = look_dir.normalize();

    let eye_pos = center - radius * look_dir;

    OrbitEye {
        orbit_center: center,
        orbit_radius: radius,
        world_from_view_rot: Quat::from_affine3(
            &Affine3A::look_at_rh(eye_pos, center, look_up).inverse(),
        ),
        fov_y: Eye::DEFAULT_FOV_Y,
        up: space_specs.up.unwrap_or(Vec3::ZERO),
        velocity: Vec3::ZERO,
    }
}
