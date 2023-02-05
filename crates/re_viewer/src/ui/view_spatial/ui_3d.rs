use egui::NumExt as _;
use glam::Affine3A;
use macaw::{vec3, BoundingBox, Quat, Vec3};

use re_data_store::{InstancePath, InstancePathHash};
use re_log_types::{EntityPath, ViewCoordinates};
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
        SpaceViewId, UiVerbosity,
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
    pub orbit_eye: Option<OrbitEye>,

    /// Currently tracked camera.
    tracked_camera: Option<InstancePath>,
    /// Camera pose just before we took over another camera via [Self::tracked_camera].
    camera_before_tracked_camera: Option<Eye>,

    #[serde(skip)]
    eye_interpolation: Option<EyeInterpolation>,

    /// Where in world space the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_point: Option<glam::Vec3>,

    // options:
    pub spin: bool,
    pub show_axes: bool,

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
            tracked_camera: None,
            camera_before_tracked_camera: None,
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
    pub fn reset_camera(&mut self, scene_bbox_accum: &BoundingBox) {
        self.interpolate_to_eye(default_eye(scene_bbox_accum, &self.space_specs).to_eye());
        self.tracked_camera = None;
        self.camera_before_tracked_camera = None;
    }

    fn update_eye(
        &mut self,
        response: &egui::Response,
        scene_bbox_accum: &BoundingBox,
        space_cameras: &[SpaceCamera3D],
    ) -> &mut OrbitEye {
        let tracking_camera = self
            .tracked_camera
            .as_ref()
            .and_then(|c| find_camera(space_cameras, &c.hash()));

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
                -response.ctx.input(|i| i.stable_dt).at_most(0.1) * 150.0,
                0.0,
            ));
            response.ctx.request_repaint();
        }

        if let Some(cam_interpolation) = &mut self.eye_interpolation {
            cam_interpolation.elapsed_time += response.ctx.input(|i| i.stable_dt).at_most(0.1);

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
            let target_time = EyeInterpolation::target_time(&start.to_eye(), &target) * 4.0;
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
    pub up: Option<glam::Vec3>,
    pub right: Option<glam::Vec3>,
}

impl SpaceSpecs {
    pub fn from_view_coordinates(coordinates: Option<ViewCoordinates>) -> Self {
        let up = (|| Some(coordinates?.up()?.as_vec3().into()))();
        let right = (|| Some(coordinates?.right()?.as_vec3().into()))();

        Self { up, right }
    }
}

fn find_camera(space_cameras: &[SpaceCamera3D], needle: &InstancePathHash) -> Option<Eye> {
    let mut found_camera = None;

    for camera in space_cameras {
        if &camera.instance_path_hash == needle {
            if found_camera.is_some() {
                return None; // More than one camera
            } else {
                found_camera = Some(camera);
            }
        }
    }

    found_camera.and_then(Eye::from_camera)
}

// ----------------------------------------------------------------------------

pub const HELP_TEXT_3D: &str = "Drag to rotate.\n\
    Drag with secondary mouse button to pan.\n\
    Drag with middle mouse button to roll the view.\n\
    Scroll to zoom.\n\
    \n\
    While hovering the 3D view, navigate with WSAD and QE.\n\
    CTRL slows down, SHIFT speeds up.\n\
    \n\
    Double-click an object to focus the view on it.\n\
    \n\
    Double-click on empty space to reset the view.";

/// TODO(andreas): Split into smaller parts, more re-use with `ui_2d`
pub fn view_3d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewSpatialState,
    space: &EntityPath,
    space_view_id: SpaceViewId,
    mut scene: SceneSpatial,
) {
    crate::profile_function!();

    state.state_3d.space_camera = scene.space_cameras.clone();

    let (rect, mut response) =
        ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    // If we're tracking a camera right now, we want to make it slightly sticky,
    // so that a click on some entity doesn't immediately break the tracked state.
    // (Threshold is in amount of ui points the mouse was moved.)
    let orbit_eye_drag_threshold = match &state.state_3d.tracked_camera {
        Some(_) => 4.0,
        None => 0.0,
    };
    let orbit_eye =
        state
            .state_3d
            .update_eye(&response, &state.scene_bbox_accum, &scene.space_cameras);
    let did_interact_with_eye = orbit_eye.interact(&response, orbit_eye_drag_threshold);

    let orbit_eye = *orbit_eye;
    let eye = orbit_eye.to_eye();

    if did_interact_with_eye {
        state.state_3d.last_eye_interact_time = ui.input(|i| i.time);
        state.state_3d.eye_interpolation = None;
        state.state_3d.tracked_camera = None;
        state.state_3d.camera_before_tracked_camera = None;
    }

    // TODO(andreas): This isn't part of the camera, but of the transform https://github.com/rerun-io/rerun/issues/753
    for camera in &scene.space_cameras {
        if ctx.app_options.show_camera_axes_in_3d {
            let transform = camera.world_from_cam();
            let axis_length =
                eye.approx_pixel_world_size_at(transform.translation(), rect.size()) * 32.0;
            scene
                .primitives
                .add_axis_lines(transform, camera.instance_path_hash, axis_length);
        }
    }

    // TODO(andreas): We're very close making the hover reaction of ui2d and ui3d the same. Finish the job!
    if let Some(pointer_pos) = response.hover_pos() {
        let picking_result =
            scene.picking(glam::vec2(pointer_pos.x, pointer_pos.y), &rect, &eye, 5.0);

        for hit in picking_result.iter_hits() {
            let Some(instance_path) = hit.instance_path_hash.resolve(&ctx.log_db.entity_db)
            else { continue; };

            // Special hover ui for images.
            let picked_image_with_uv = if let AdditionalPickingInfo::TexturedRect(uv) = hit.info {
                scene
                    .ui
                    .images
                    .iter()
                    .find(|image| image.instance_path_hash == hit.instance_path_hash)
                    .map(|image| (image, uv))
            } else {
                None
            };
            response = if let Some((image, uv)) = picked_image_with_uv {
                response
                    .on_hover_cursor(egui::CursorIcon::Crosshair)
                    .on_hover_ui_at_pointer(|ui| {
                        ui.set_max_width(320.0);

                        ui.vertical(|ui| {
                            ui.label(instance_path.to_string());
                            instance_path.data_ui(
                                ctx,
                                ui,
                                UiVerbosity::Small,
                                &ctx.current_query(),
                            );

                            let tensor_view = ctx.cache.image.get_view_with_annotations(
                                &image.tensor,
                                &image.annotations,
                                ctx.render_ctx,
                            );

                            if let [h, w, ..] = &image.tensor.shape[..] {
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
                    ctx.instance_path_button(ui, Some(space_view_id), &instance_path);
                    instance_path.data_ui(
                        ctx,
                        ui,
                        crate::ui::UiVerbosity::Reduced,
                        &ctx.current_query(),
                    );
                })
            };
        }

        ctx.set_hovered(picking_result.iter_hits().filter_map(|pick| {
            pick.instance_path_hash
                .resolve(&ctx.log_db.entity_db)
                .map(|instance_path| Selection::InstancePath(Some(space_view_id), instance_path))
        }));
        state.state_3d.hovered_point = picking_result
            .opaque_hit
            .as_ref()
            .or_else(|| picking_result.transparent_hits.last())
            .map(|hit| picking_result.space_position(hit));

        project_onto_other_spaces(ctx, &scene.space_cameras, &mut state.state_3d, space);
    }

    ctx.select_hovered_on_click(&response);

    // Double click changes camera
    if response.double_clicked() {
        state.state_3d.tracked_camera = None;
        state.state_3d.camera_before_tracked_camera = None;

        // While hovering an entity, focuses the camera on it.
        if let Some(Selection::InstancePath(_, instance_path)) = ctx.hovered().first() {
            if let Some(camera) = find_camera(&scene.space_cameras, &instance_path.hash()) {
                state.state_3d.camera_before_tracked_camera =
                    state.state_3d.orbit_eye.map(|eye| eye.to_eye());
                state.state_3d.interpolate_to_eye(camera);
                state.state_3d.tracked_camera = Some(instance_path.clone());
            } else if let Some(clicked_point) = state.state_3d.hovered_point {
                if let Some(mut new_orbit_eye) = state.state_3d.orbit_eye {
                    // TODO(andreas): It would be nice if we could focus on the center of the entity rather than the clicked point.
                    //                  We can figure out the transform/translation at the hovered path but that's usually not what we'd expect either
                    //                  (especially for entities with many instances, like a point cloud)
                    new_orbit_eye.orbit_radius = new_orbit_eye.position().distance(clicked_point);
                    new_orbit_eye.orbit_center = clicked_point;
                    state.state_3d.interpolate_to_orbit_eye(new_orbit_eye);
                }
            }
        }
        // Without hovering, resets the camera.
        else {
            state.state_3d.reset_camera(&state.scene_bbox_accum);
        }
    }

    // Allow to restore the camera state with escape if a camera was tracked before.
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if let Some(camera_before_changing_tracked_state) =
            state.state_3d.camera_before_tracked_camera
        {
            state
                .state_3d
                .interpolate_to_eye(camera_before_changing_tracked_state);
            state.state_3d.camera_before_tracked_camera = None;
            state.state_3d.tracked_camera = None;
        }
    }

    show_projections_from_2d_space(ctx, &mut scene, &state.scene_bbox_accum);

    if state.state_3d.show_axes {
        let axis_length = 1.0; // The axes are also a measuring stick
        scene.primitives.add_axis_lines(
            macaw::IsoTransform::IDENTITY,
            InstancePathHash::NONE,
            axis_length,
        );
    }

    {
        let orbit_center_alpha = egui::remap_clamp(
            ui.input(|i| i.time) - state.state_3d.last_eye_interact_time,
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
        state.auto_size_config(rect.size()),
    );
}

fn paint_view(
    ui: &mut egui::Ui,
    eye: Eye,
    rect: egui::Rect,
    scene: &SceneSpatial,
    render_ctx: &mut RenderContext,
    name: &str,
    auto_size_config: re_renderer::AutoSizeConfig,
) {
    crate::profile_function!();

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

    // Draw labels:
    {
        let painter = ui.painter().with_clip_rect(ui.max_rect());

        crate::profile_function!("labels");
        let ui_from_world = eye.ui_from_world(&rect);
        for label in &scene.ui.labels_3d {
            let pos_in_ui = ui_from_world * label.origin.extend(1.0);
            if pos_in_ui.w <= 0.0 {
                continue; // behind camera
            }
            let pos_in_ui = pos_in_ui / pos_in_ui.w;

            let font_id = egui::TextStyle::Monospace.resolve(ui.style());

            let galley = ui.fonts(|fonts| {
                fonts.layout(
                    (*label.text).to_owned(),
                    font_id,
                    ui.style().visuals.text_color(),
                    100.0,
                )
            });

            let text_rect = egui::Align2::CENTER_TOP.anchor_rect(egui::Rect::from_min_size(
                egui::pos2(pos_in_ui.x, pos_in_ui.y),
                galley.size(),
            ));

            let bg_rect = text_rect.expand2(egui::vec2(6.0, 2.0));
            painter.add(egui::Shape::rect_filled(
                bg_rect,
                3.0,
                ui.style().visuals.code_bg_color,
            ));
            painter.add(egui::Shape::galley(text_rect.min, galley));
        }
    }
}

fn show_projections_from_2d_space(
    ctx: &mut ViewerContext<'_>,
    scene: &mut SceneSpatial,
    scene_bbox_accum: &BoundingBox,
) {
    if let HoveredSpace::TwoD { space_2d, pos } = ctx.selection_state().hovered_space() {
        let mut line_batch = scene.primitives.line_strips.batch("picking ray");

        for cam in &scene.space_cameras {
            if &cam.entity_path == space_2d {
                if let Some(ray) = cam.unproject_as_ray(glam::vec2(pos.x, pos.y)) {
                    // Render a thick line to the actual z value if any and a weaker one as an extension
                    // If we don't have a z value, we only render the thick one.
                    let thick_ray_length = if pos.z.is_finite() && pos.z > 0.0 {
                        Some(pos.z)
                    } else {
                        cam.picture_plane_distance
                    };

                    let origin = ray.point_along(0.0);
                    // No harm in making this ray _very_ long. (Infinite messes with things though!)
                    let fallback_ray_end = ray.point_along(scene_bbox_accum.size().length() * 10.0);

                    if let Some(line_length) = thick_ray_length {
                        let main_ray_end = ray.point_along(line_length);
                        line_batch
                            .add_segment(origin, main_ray_end)
                            .color(egui::Color32::WHITE)
                            .flags(re_renderer::renderer::LineStripFlags::NO_COLOR_GRADIENT)
                            .radius(Size::new_points(1.0));
                        line_batch
                            .add_segment(main_ray_end, fallback_ray_end)
                            .color(egui::Color32::DARK_GRAY)
                            // TODO(andreas): Make this dashed.
                            .flags(re_renderer::renderer::LineStripFlags::NO_COLOR_GRADIENT)
                            .radius(Size::new_points(0.5));
                    } else {
                        line_batch
                            .add_segment(origin, fallback_ray_end)
                            .color(egui::Color32::WHITE)
                            .flags(re_renderer::renderer::LineStripFlags::NO_COLOR_GRADIENT)
                            .radius(Size::new_points(1.0));
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
    space: &EntityPath,
) {
    let mut target_spaces = vec![];
    for cam in space_cameras {
        let point_in_2d = state
            .hovered_point
            .and_then(|hovered_point| cam.project_onto_2d(hovered_point));
        target_spaces.push((cam.entity_path.clone(), point_in_2d));
    }
    ctx.selection_state_mut()
        .set_hovered_space(HoveredSpace::ThreeD {
            space_3d: space.clone(),
            target_spaces,
        });
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
