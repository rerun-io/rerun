use eframe::emath::RectTransform;
use egui::NumExt as _;
use glam::Affine3A;
use macaw::{vec3, BoundingBox, Quat, Vec3};

use re_data_store::EntityPropertyMap;
use re_log_types::{EntityPath, ViewCoordinates};
use re_renderer::{
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
    Size,
};

use crate::{
    gpu_bridge,
    misc::{HoveredSpace, Item, SpaceViewHighlights},
    ui::{
        view_spatial::{
            ui::{create_labels, outline_config, picking, screenshot_context_menu},
            ui_renderer_bridge::{fill_view_builder, ScreenBackground},
            SceneSpatial, SpaceCamera3D, SpatialNavigationMode,
        },
        SpaceViewId,
    },
    ViewerContext,
};

use super::{
    eye::{Eye, OrbitEye},
    scene::SceneSpatialPrimitives,
    ViewSpatialState,
};

// ---

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct View3DState {
    pub orbit_eye: Option<OrbitEye>,

    /// Currently tracked camera.
    pub tracked_camera: Option<EntityPath>,

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
    pub show_bbox: bool,

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
            show_bbox: false,
            last_eye_interact_time: f64::NEG_INFINITY,
            space_specs: Default::default(),
            space_camera: Default::default(),
        }
    }
}

impl View3DState {
    pub fn reset_camera(&mut self, scene_bbox_accum: &BoundingBox) {
        self.interpolate_to_orbit_eye(default_eye(scene_bbox_accum, &self.space_specs, &[]));
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
            .and_then(|c| find_camera(space_cameras, c));

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
            .get_or_insert_with(|| default_eye(scene_bbox_accum, &self.space_specs, space_cameras));

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

            if 1.0 <= t {
                // We have arrived at our target
                self.eye_interpolation = None;
            }
        }

        orbit_camera
    }

    fn interpolate_to_eye(&mut self, target: Eye) {
        if let Some(start) = self.orbit_eye {
            let target_time = EyeInterpolation::target_time(&start.to_eye(), &target);
            self.spin = false; // the user wants to move the camera somewhere, so stop spinning
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
            self.spin = false; // the user wants to move the camera somewhere, so stop spinning
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

#[derive(Clone, Default, Debug)]
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

fn find_camera(space_cameras: &[SpaceCamera3D], needle: &EntityPath) -> Option<Eye> {
    let mut found_camera = None;

    for camera in space_cameras {
        if &camera.ent_path == needle {
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
    Drag with middle mouse button (or primary mouse button + holding SHIFT) to roll the view.\n\
    Scroll to zoom.\n\
    \n\
    While hovering the 3D view, navigate with WSAD and QE.\n\
    CTRL slows down, SHIFT speeds up.\n\
    \n\
    Double-click an object to focus the view on it.\n\
    For cameras, you can restore the view again with Escape.\n\
    \n\
    Double-click on empty space to reset the view.";

/// TODO(andreas): Split into smaller parts, more re-use with `ui_2d`
#[allow(clippy::too_many_arguments)]
pub fn view_3d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut ViewSpatialState,
    space: &EntityPath,
    space_view_id: SpaceViewId,
    mut scene: SceneSpatial,
    highlights: &SpaceViewHighlights,
    entity_properties: &EntityPropertyMap,
) {
    crate::profile_function!();

    state.state_3d.space_camera = scene.space_cameras.clone();

    let (rect, mut response) =
        ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    if !rect.is_positive() {
        return; // protect against problems with zero-sized views
    }

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

    let did_interact_with_eye = orbit_eye.update(&response, orbit_eye_drag_threshold);

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
                .add_axis_lines(transform, Some(&camera.ent_path), axis_length);
        }
    }

    // Determine view port resolution and position.
    let resolution_in_pixel =
        gpu_bridge::viewport_resolution_in_pixels(rect, ui.ctx().pixels_per_point());
    if resolution_in_pixel[0] == 0 || resolution_in_pixel[1] == 0 {
        return;
    }

    let target_config = TargetConfiguration {
        name: space.to_string().into(),

        resolution_in_pixel,

        view_from_world: eye.world_from_view.inverse(),
        projection_from_view: Projection::Perspective {
            vertical_fov: eye.fov_y.unwrap(),
            near_plane_distance: eye.near(),
        },

        pixels_from_point: ui.ctx().pixels_per_point(),
        auto_size_config: state.auto_size_config(),

        outline_config: scene
            .primitives
            .any_outlines
            .then(|| outline_config(ui.ctx())),
    };

    let mut view_builder = ViewBuilder::new(ctx.render_ctx, target_config);

    // Create labels now since their shapes participate are added to scene.ui for picking.
    let label_shapes = create_labels(
        &mut scene.ui,
        RectTransform::from_to(rect, rect),
        RectTransform::from_to(rect, rect),
        &eye,
        ui,
        highlights,
        SpatialNavigationMode::ThreeD,
    );

    if !re_ui::egui_helpers::is_anything_being_dragged(ui.ctx()) {
        response = picking(
            ctx,
            response,
            RectTransform::from_to(rect, rect),
            rect,
            ui,
            eye,
            &mut view_builder,
            space_view_id,
            state,
            &scene,
            space,
            entity_properties,
        );
    }

    // Double click changes camera
    if response.double_clicked() {
        state.state_3d.tracked_camera = None;
        state.state_3d.camera_before_tracked_camera = None;

        // While hovering an entity, focuses the camera on it.
        if let Some(Item::InstancePath(_, instance_path)) = ctx.hovered().first() {
            if let Some(camera) = find_camera(&scene.space_cameras, &instance_path.entity_path) {
                state.state_3d.camera_before_tracked_camera =
                    state.state_3d.orbit_eye.map(|eye| eye.to_eye());
                state.state_3d.interpolate_to_eye(camera);
                state.state_3d.tracked_camera = Some(instance_path.entity_path.clone());
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
    if response.hovered() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
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

    // Screenshot context menu.
    let (_, screenshot_mode) = screenshot_context_menu(ctx, response);
    if let Some(mode) = screenshot_mode {
        let _ =
            view_builder.schedule_screenshot(ctx.render_ctx, space_view_id.gpu_readback_id(), mode);
    }

    show_projections_from_2d_space(
        ctx,
        &mut scene,
        &state.state_3d.tracked_camera,
        &state.scene_bbox_accum,
    );

    if state.state_3d.show_axes {
        let axis_length = 1.0; // The axes are also a measuring stick
        scene
            .primitives
            .add_axis_lines(macaw::IsoTransform::IDENTITY, None, axis_length);
    }

    if state.state_3d.show_bbox {
        let bbox = scene.primitives.bounding_box();
        if bbox.is_something() && bbox.is_finite() {
            let scale = bbox.size();
            let translation = bbox.center();
            let bbox_from_unit_cube = glam::Affine3A::from_scale_rotation_translation(
                scale,
                Default::default(),
                translation,
            );
            scene
                .primitives
                .line_strips
                .batch("scene_bbox")
                .add_box_outline(bbox_from_unit_cube)
                .radius(Size::AUTO)
                .color(egui::Color32::WHITE);
        }
    }

    if state.state_3d.show_bbox {
        let bbox = scene.primitives.bounding_box();
        if bbox.is_something() && bbox.is_finite() {
            let scale = bbox.size();
            let translation = bbox.center();
            let bbox_from_unit_cube = glam::Affine3A::from_scale_rotation_translation(
                scale,
                Default::default(),
                translation,
            );
            scene
                .primitives
                .line_strips
                .batch("scene_bbox")
                .add_box_outline(bbox_from_unit_cube)
                .radius(Size::AUTO)
                .color(egui::Color32::WHITE);
        }
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

    // Composite viewbuilder into egui.
    let command_buffer = match fill_view_builder(
        ctx.render_ctx,
        &mut view_builder,
        scene.primitives,
        &ScreenBackground::GenericSkybox,
    ) {
        Ok(command_buffer) => command_buffer,
        Err(err) => {
            re_log::error!("Failed to fill view builder: {}", err);
            return;
        }
    };
    ui.painter().add(gpu_bridge::renderer_paint_callback(
        ctx.render_ctx,
        command_buffer,
        view_builder,
        rect,
        ui.ctx().pixels_per_point(),
    ));

    // Add egui driven labels on top of re_renderer content.
    let painter = ui.painter().with_clip_rect(ui.max_rect());
    painter.extend(label_shapes);
}

fn show_projections_from_2d_space(
    ctx: &mut ViewerContext<'_>,
    scene: &mut SceneSpatial,
    tracked_space_camera: &Option<EntityPath>,
    scene_bbox_accum: &BoundingBox,
) {
    match ctx.selection_state().hovered_space() {
        HoveredSpace::TwoD { space_2d, pos } => {
            if let Some(cam) = scene
                .space_cameras
                .iter()
                .find(|cam| &cam.ent_path == space_2d)
            {
                if let Some(ray) = cam.unproject_as_ray(glam::vec2(pos.x, pos.y)) {
                    // Render a thick line to the actual z value if any and a weaker one as an extension
                    // If we don't have a z value, we only render the thick one.
                    let thick_ray_length = if pos.z.is_finite() && pos.z > 0.0 {
                        Some(pos.z)
                    } else {
                        cam.picture_plane_distance
                    };

                    add_picking_ray(
                        &mut scene.primitives,
                        ray,
                        scene_bbox_accum,
                        thick_ray_length,
                    );
                }
            }
        }
        HoveredSpace::ThreeD {
            pos: Some(pos),
            tracked_space_camera: Some(camera_path),
            ..
        } => {
            if tracked_space_camera
                .as_ref()
                .map_or(true, |tracked| tracked != camera_path)
            {
                if let Some(cam) = scene
                    .space_cameras
                    .iter()
                    .find(|cam| &cam.ent_path == camera_path)
                {
                    let cam_to_pos = *pos - cam.position();
                    let distance = cam_to_pos.length();
                    let ray = macaw::Ray3::from_origin_dir(cam.position(), cam_to_pos / distance);
                    add_picking_ray(&mut scene.primitives, ray, scene_bbox_accum, Some(distance));
                }
            }
        }
        _ => {}
    }
}

fn add_picking_ray(
    primitives: &mut SceneSpatialPrimitives,
    ray: macaw::Ray3,
    scene_bbox_accum: &BoundingBox,
    thick_ray_length: Option<f32>,
) {
    let mut line_batch = primitives.line_strips.batch("picking ray");

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

/// Look down and forward on the space camera.
fn focus_on_space_camera(
    scene_bbox: &macaw::BoundingBox,
    space_specs: &SpaceSpecs,
    camera: &SpaceCamera3D,
) -> OrbitEye {
    let Some(up) = camera.view_coordinates.up() else {
        return default_eye(scene_bbox, space_specs, &[]);
    };
    let Some(fwd) = camera.view_coordinates.forward() else {
        return default_eye(scene_bbox, space_specs, &[]);
    };
    let [x, y, z] = up.as_vec3();
    let up = vec3(x, y, z);
    let [x, y, z] = fwd.as_vec3();
    let fwd = vec3(x, y, z);
    let down = -up;
    let look_dir = camera
        .world_from_cam()
        .transform_vector3(fwd + 0.5 * down)
        .normalize();
    let look_up = camera.world_from_cam().transform_vector3(up);
    let mut radius = camera.picture_plane_distance.unwrap_or(1.0);
    if !radius.is_finite() || radius == 0.0 {
        radius = 1.0;
    }
    let center = camera.position();
    let eye_pos = center - radius * look_dir;
    OrbitEye::new(
        center,
        radius,
        Quat::from_affine3(&Affine3A::look_at_rh(eye_pos, center, look_up).inverse()),
        Vec3::ZERO,
    )
}

fn default_eye(
    scene_bbox: &macaw::BoundingBox,
    space_specs: &SpaceSpecs,
    space_cameras: &[SpaceCamera3D],
) -> OrbitEye {
    if let Some(space_camera) = space_cameras.get(0) {
        return focus_on_space_camera(scene_bbox, space_specs, space_camera);
    }
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

    OrbitEye::new(
        center,
        radius,
        Quat::from_affine3(&Affine3A::look_at_rh(eye_pos, center, look_up).inverse()),
        space_specs.up.unwrap_or(Vec3::ZERO),
    )
}

// fn default_eye(
//     scene_bbox: &macaw::BoundingBox,
//     space_specs: &SpaceSpecs,
//     space_cameras: &[SpaceCamera3D],
// ) -> OrbitEye {
//     println!("Space cameras: {:?}", space_cameras[0].ent_path);

//     if let Some(space_camera) = space_cameras.get(0) {
//         return focus_on_space_camera(scene_bbox, space_specs, space_camera);
//     }
//     let mut center = scene_bbox.center();
//     if !center.is_finite() {
//         center = Vec3::ZERO;
//     }

//     let mut radius = 2.0 * scene_bbox.half_size().length();
//     if !radius.is_finite() || radius == 0.0 {
//         radius = 1.0;
//     }

//     let space_camera = space_cameras.get(0);
//     let mut look_up = space_specs.up.unwrap_or(Vec3::Z);

//     let look_dir = if let Some(right) = space_specs.right {
//         // Make sure right is to the right, and up is up:
//         let fwd = look_up.cross(right);
//         0.75 * fwd + 0.25 * right - 0.25 * look_up
//     } else if let Some(space_camera) = space_camera {
//         // Look down and forward on the first cam in this space
//         let down = -match space_camera.view_coordinates.up() {
//             Some(up) => {
//                 let [x, y, z] = up.as_vec3();
//                 let as_vec = vec3(x, y, z);
//                 look_up = space_camera.world_from_cam().transform_vector3(as_vec);
//                 as_vec
//             }
//             None => Vec3::X,
//         };
//         let forward = match space_camera.view_coordinates.forward() {
//             Some(forward) => {
//                 let [x, y, z] = forward.as_vec3();
//                 vec3(x, y, z)
//             }
//             None => Vec3::Y,
//         };
//         space_camera
//             .world_from_cam()
//             .transform_vector3(forward + 0.5 * down)
//     } else {
//         // Look along the cardinal directions:
//         vec3(1.0, 1.0, 1.0)
//     };
//     let look_dir = look_dir.normalize();
//     let eye_pos = center - 1.0 * look_dir;
//     let space_camera = space_camera.unwrap();
//     OrbitEye::new(
//         center,
//         radius,
//         Quat::from_affine3(&Affine3A::look_at_rh(eye_pos, center, look_up).inverse()),
//         space_specs.up.unwrap_or(Vec3::ZERO),
//     )
// }
