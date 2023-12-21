use egui::{emath::RectTransform, NumExt as _};
use glam::Affine3A;
use macaw::{vec3, BoundingBox, Quat, Vec3};

use re_log_types::EntityPath;
use re_renderer::{
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
    LineStripSeriesBuilder, Size,
};
use re_space_view::controls::{
    RuntimeModifiers, DRAG_PAN3D_BUTTON, RESET_VIEW_BUTTON_TEXT, ROLL_MOUSE, ROLL_MOUSE_ALT,
    ROLL_MOUSE_MODIFIER, ROTATE3D_BUTTON, SPEED_UP_3D_MODIFIER, TRACKED_CAMERA_RESTORE_KEY,
};
use re_types::components::ViewCoordinates;
use re_viewer_context::{
    gpu_bridge, Item, SelectedSpaceContext, SpaceViewSystemExecutionError, SystemExecutionOutput,
    ViewQuery, ViewerContext,
};

use crate::{
    contexts::SharedRenderBuilders,
    parts::{
        collect_ui_labels, image_view_coordinates, CamerasPart,
        SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
    },
    space_camera_3d::SpaceCamera3D,
    ui::{create_labels, outline_config, picking, screenshot_context_menu, SpatialSpaceViewState},
    view_kind::SpatialSpaceViewKind,
};

use super::eye::{Eye, OrbitEye};

// ---

#[derive(Clone)]
pub struct View3DState {
    pub orbit_eye: Option<OrbitEye>,
    pub did_interact_with_eye: bool,

    /// Currently tracked camera.
    pub tracked_camera: Option<EntityPath>,

    /// Camera pose just before we took over another camera via [Self::tracked_camera].
    camera_before_tracked_camera: Option<Eye>,

    eye_interpolation: Option<EyeInterpolation>,

    // options:
    spin: bool,
    pub show_axes: bool,
    pub show_bbox: bool,
    pub show_accumulated_bbox: bool,

    eye_interact_fade_in: bool,
    eye_interact_fade_change_time: f64,
}

impl Default for View3DState {
    fn default() -> Self {
        Self {
            orbit_eye: Default::default(),
            did_interact_with_eye: false,
            tracked_camera: None,
            camera_before_tracked_camera: None,
            eye_interpolation: Default::default(),
            spin: false,
            show_axes: false,
            show_bbox: false,
            show_accumulated_bbox: false,
            eye_interact_fade_in: false,
            eye_interact_fade_change_time: f64::NEG_INFINITY,
        }
    }
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t) * (1. - t)
}

impl View3DState {
    pub fn reset_camera(
        &mut self,
        scene_bbox_accum: &BoundingBox,
        view_coordinates: &Option<ViewCoordinates>,
    ) {
        self.interpolate_to_orbit_eye(default_eye(scene_bbox_accum, view_coordinates));
        self.tracked_camera = None;
        self.camera_before_tracked_camera = None;
    }

    fn update_eye(
        &mut self,
        response: &egui::Response,
        scene_bbox_accum: &BoundingBox,
        space_cameras: &[SpaceCamera3D],
        view_coordinates: Option<ViewCoordinates>,
    ) -> &mut OrbitEye {
        // If the user has not interacted with the eye-camera yet, continue to
        // interpolate to the new default eye. This gives much better robustness
        // with scenes that grow over time.
        if !self.did_interact_with_eye {
            self.interpolate_to_orbit_eye(default_eye(scene_bbox_accum, &view_coordinates));
        }

        let orbit_camera = self
            .orbit_eye
            .get_or_insert_with(|| default_eye(scene_bbox_accum, &view_coordinates));

        // Follow tracked camera if any.
        if let Some(tracking_camera) = self
            .tracked_camera
            .as_ref()
            .and_then(|c| find_camera(space_cameras, c))
        {
            // While we're still interpolating towards it, we need to continuously update the interpolation target.
            if let Some(cam_interpolation) = &mut self.eye_interpolation {
                cam_interpolation.target_orbit = None;
                if cam_interpolation.target_eye != Some(tracking_camera) {
                    cam_interpolation.target_eye = Some(tracking_camera);
                    response.ctx.request_repaint();
                }
            } else {
                orbit_camera.copy_from_eye(&tracking_camera);
            }
        }

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
            let t = ease_out(t);

            if t < 1.0 {
                response.ctx.request_repaint();
            }

            if let Some(target_orbit) = &cam_interpolation.target_orbit {
                *orbit_camera = cam_interpolation.start.lerp(target_orbit, t);
            } else if let Some(target_eye) = &cam_interpolation.target_eye {
                let camera = cam_interpolation.start.to_eye().lerp(target_eye, t);
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
        if let Some(start) = self.orbit_eye.as_mut() {
            // the user wants to move the camera somewhere, so stop spinning
            self.spin = false;

            if let Some(target_time) = EyeInterpolation::target_time(&start.to_eye(), &target) {
                self.eye_interpolation = Some(EyeInterpolation {
                    elapsed_time: 0.0,
                    target_time,
                    start: *start,
                    target_orbit: None,
                    target_eye: Some(target),
                });
            } else {
                start.copy_from_eye(&target);
            }
        } else {
            // shouldn't really happen (`self.orbit_eye` is only `None` for the first frame).
        }
    }

    fn interpolate_to_orbit_eye(&mut self, target: OrbitEye) {
        if let Some(start) = self.orbit_eye {
            // the user wants to move the camera somewhere, so stop spinning
            self.spin = false;

            if let Some(target_time) =
                EyeInterpolation::target_time(&start.to_eye(), &target.to_eye())
            {
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
        } else {
            self.orbit_eye = Some(target);
        }
    }

    pub fn spin(&self) -> bool {
        self.spin
    }

    pub fn set_spin(&mut self, spin: bool) {
        self.spin = spin;
        self.did_interact_with_eye = true;
    }
}

#[derive(Clone, PartialEq)]
struct EyeInterpolation {
    elapsed_time: f32,
    target_time: f32,
    start: OrbitEye,
    target_orbit: Option<OrbitEye>,
    target_eye: Option<Eye>,
}

impl EyeInterpolation {
    pub fn target_time(start: &Eye, stop: &Eye) -> Option<f32> {
        // Take more time if the rotation is big:
        let angle_difference = start
            .world_from_rub_view
            .rotation()
            .angle_between(stop.world_from_rub_view.rotation());

        // Threshold to avoid doing pointless interpolations that trigger frame requests.
        if angle_difference < 0.01 && start.pos_in_world().distance(stop.pos_in_world()) < 0.0001 {
            None
        } else {
            Some(egui::remap_clamp(
                angle_difference,
                0.0..=std::f32::consts::PI,
                0.2..=0.7,
            ))
        }
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

pub fn help_text(re_ui: &re_ui::ReUi) -> egui::WidgetText {
    let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

    layout.add("Click and drag ");
    layout.add(ROTATE3D_BUTTON);
    layout.add(" to rotate.\n");

    layout.add("Click and drag with ");
    layout.add(DRAG_PAN3D_BUTTON);
    layout.add(" to pan.\n");

    layout.add("Drag with ");
    layout.add(ROLL_MOUSE);
    layout.add(" ( ");
    layout.add(ROLL_MOUSE_ALT);
    layout.add(" + holding ");
    layout.add(ROLL_MOUSE_MODIFIER);
    layout.add(" ) to roll the view.\n");

    layout.add("Scroll or pinch to zoom.\n\n");

    layout.add("While hovering the 3D view, navigate with ");
    layout.add_button_text("WASD");
    layout.add(" and ");
    layout.add_button_text("QE");
    layout.add(".\n");

    layout.add(RuntimeModifiers::slow_down(&re_ui.egui_ctx.os()));
    layout.add(" slows down, ");
    layout.add(SPEED_UP_3D_MODIFIER);
    layout.add(" speeds up\n\n");

    layout.add_button_text("double-click");
    layout.add(" an object to focus the view on it.\n");
    layout.add("For cameras, you can restore the view again with ");
    layout.add(TRACKED_CAMERA_RESTORE_KEY);
    layout.add(" .\n\n");

    layout.add_button_text(RESET_VIEW_BUTTON_TEXT);
    layout.add(" on empty space to reset the view.");

    layout.layout_job.into()
}

pub fn view_3d(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut SpatialSpaceViewState,
    query: &ViewQuery<'_>,
    system_output: re_viewer_context::SystemExecutionOutput,
) -> Result<(), SpaceViewSystemExecutionError> {
    re_tracing::profile_function!();

    let SystemExecutionOutput {
        view_systems: parts,
        context_systems: view_ctx,
        draw_data,
    } = system_output;

    let highlights = &query.highlights;
    let space_cameras = &parts.get::<CamerasPart>()?.space_cameras;
    let view_coordinates = ctx
        .store_db
        .store()
        // Allow logging view-coordinates to `/` and have it apply to `/world` etc.
        // See https://github.com/rerun-io/rerun/issues/3538
        .query_latest_component_at_closest_ancestor(query.space_origin, &ctx.current_query())
        .map(|(_, c)| c.value);

    let (rect, mut response) =
        ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

    if !rect.is_positive() {
        return Ok(()); // protect against problems with zero-sized views
    }

    // If we're tracking a camera right now, we want to make it slightly sticky,
    // so that a click on some entity doesn't immediately break the tracked state.
    // (Threshold is in amount of ui points the mouse was moved.)
    let orbit_eye_drag_threshold = match &state.state_3d.tracked_camera {
        Some(_) => 4.0,
        None => 0.0,
    };
    let orbit_eye = state.state_3d.update_eye(
        &response,
        &state.scene_bbox_accum,
        space_cameras,
        view_coordinates,
    );
    let did_interact_with_eye =
        orbit_eye.update(&response, orbit_eye_drag_threshold, &state.scene_bbox_accum);

    let orbit_eye = *orbit_eye;
    let eye = orbit_eye.to_eye();

    if did_interact_with_eye {
        state.state_3d.did_interact_with_eye = true;
        state.state_3d.eye_interpolation = None;
        state.state_3d.tracked_camera = None;
        state.state_3d.camera_before_tracked_camera = None;
    }

    let mut line_builder = LineStripSeriesBuilder::new(ctx.render_ctx)
        .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

    // Origin gizmo if requested.
    // TODO(andreas): Move this to the transform3d_arrow scene part.
    //              As of #2522 state is now longer accessible there, move the property to a context?
    if state.state_3d.show_axes {
        let axis_length = 1.0; // The axes are also a measuring stick
        crate::parts::add_axis_arrows(
            &mut line_builder,
            macaw::Affine3A::IDENTITY,
            None,
            axis_length,
            re_renderer::OutlineMaskPreference::NONE,
        );
    }

    // Determine view port resolution and position.
    let resolution_in_pixel =
        gpu_bridge::viewport_resolution_in_pixels(rect, ui.ctx().pixels_per_point());
    if resolution_in_pixel[0] == 0 || resolution_in_pixel[1] == 0 {
        return Ok(());
    }

    let target_config = TargetConfiguration {
        name: query.space_origin.to_string().into(),

        resolution_in_pixel,

        view_from_world: eye.world_from_rub_view.inverse(),
        projection_from_view: Projection::Perspective {
            vertical_fov: eye.fov_y.unwrap_or(Eye::DEFAULT_FOV_Y),
            near_plane_distance: eye.near(),
            aspect_ratio: resolution_in_pixel[0] as f32 / resolution_in_pixel[1] as f32,
        },
        viewport_transformation: re_renderer::RectTransform::IDENTITY,

        pixels_from_point: ui.ctx().pixels_per_point(),
        auto_size_config: state.auto_size_config(),

        outline_config: query
            .highlights
            .any_outlines()
            .then(|| outline_config(ui.ctx())),
    };

    let mut view_builder = ViewBuilder::new(ctx.render_ctx, target_config);

    // Create labels now since their shapes participate are added to scene.ui for picking.
    let (label_shapes, ui_rects) = create_labels(
        &collect_ui_labels(&parts),
        RectTransform::from_to(rect, rect),
        &eye,
        ui,
        highlights,
        SpatialSpaceViewKind::ThreeD,
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
            state,
            &view_ctx,
            &parts,
            &ui_rects,
            query,
            SpatialSpaceViewKind::ThreeD,
        )?;
    }

    // Double click changes camera to focus on an entity.
    if response.double_clicked() {
        // Clear out tracked camera if there's any.
        state.state_3d.tracked_camera = None;
        state.state_3d.camera_before_tracked_camera = None;

        // While hovering an entity, focuses the camera on it.
        if let Some((Item::InstancePath(_, instance_path), _)) = ctx.hovered().first() {
            if let Some(camera) = find_camera(space_cameras, &instance_path.entity_path) {
                state.state_3d.camera_before_tracked_camera =
                    state.state_3d.orbit_eye.map(|eye| eye.to_eye());
                state.state_3d.interpolate_to_eye(camera);
                state.state_3d.tracked_camera = Some(instance_path.entity_path.clone());
            } else if let Some(SelectedSpaceContext::ThreeD {
                pos: Some(clicked_point),
                ..
            }) = ctx.selection_state().hovered_space_context()
            {
                if let Some(mut new_orbit_eye) = state.state_3d.orbit_eye {
                    // TODO(andreas): It would be nice if we could focus on the center of the entity rather than the clicked point.
                    //                  We can figure out the transform/translation at the hovered path but that's usually not what we'd expect either
                    //                  (especially for entities with many instances, like a point cloud)
                    new_orbit_eye.orbit_radius = new_orbit_eye.position().distance(*clicked_point);
                    new_orbit_eye.orbit_center = *clicked_point;
                    state.state_3d.interpolate_to_orbit_eye(new_orbit_eye);
                }
            }
        }
        // Without hovering, resets the camera.
        else {
            state.scene_bbox_accum = state.scene_bbox;
            state
                .state_3d
                .reset_camera(&state.scene_bbox_accum, &view_coordinates);
        }
    }

    // Allow to restore the camera state with escape if a camera was tracked before.
    if response.hovered() && ui.input(|i| i.key_pressed(TRACKED_CAMERA_RESTORE_KEY)) {
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
        view_builder
            .schedule_screenshot(ctx.render_ctx, query.space_view_id.gpu_readback_id(), mode)
            .ok();
    }

    for selected_context in ctx.selection_state().selected_space_context() {
        show_projections_from_2d_space(
            &mut line_builder,
            space_cameras,
            state,
            selected_context,
            ui.style().visuals.selection.bg_fill,
        );
    }
    if let Some(hovered_context) = ctx.selection_state().hovered_space_context() {
        show_projections_from_2d_space(
            &mut line_builder,
            space_cameras,
            state,
            hovered_context,
            egui::Color32::WHITE,
        );
    }

    {
        let mut box_batch = line_builder.batch("scene_bbox");
        if state.state_3d.show_bbox {
            box_batch
                .add_box_outline(&state.scene_bbox)
                .map(|lines| lines.radius(Size::AUTO).color(egui::Color32::WHITE));
        }
        if state.state_3d.show_accumulated_bbox {
            box_batch
                .add_box_outline(&state.scene_bbox_accum)
                .map(|lines| {
                    lines
                        .radius(Size::AUTO)
                        .color(egui::Color32::from_gray(170))
                });
        }
    }

    // Show center of orbit camera when interacting with camera (it's quite helpful).
    {
        const FADE_DURATION: f32 = 0.1;

        let ui_time = ui.input(|i| i.time);
        let any_mouse_button_down = ui.input(|i| i.pointer.any_down());

        // Don't show for merely scrolling.
        // Scroll events from a mouse wheel often happen with some pause between meaning we either need a long delay for the center to show
        // or live with the flickering.
        let should_show_center_of_orbit_camera = did_interact_with_eye && any_mouse_button_down;

        if should_show_center_of_orbit_camera && !state.state_3d.eye_interact_fade_in {
            // Any interaction immediately causes fade in to start if it's not already on.
            state.state_3d.eye_interact_fade_change_time = ui_time;
            state.state_3d.eye_interact_fade_in = true;
        } else if state.state_3d.eye_interact_fade_in && !any_mouse_button_down {
            // Fade out on the other hand only happens if no mouse cursor is pressed.
            state.state_3d.eye_interact_fade_change_time = ui_time;
            state.state_3d.eye_interact_fade_in = false;
        }

        pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
            let t = f32::clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
            t * t * (3.0 - t * 2.0)
        }

        // Compute smooth fade.
        let time_since_fade_change =
            (ui_time - state.state_3d.eye_interact_fade_change_time) as f32;
        let orbit_center_fade = if state.state_3d.eye_interact_fade_in {
            // Fade in.
            smoothstep(0.0, FADE_DURATION, time_since_fade_change)
        } else {
            // Fade out.
            smoothstep(FADE_DURATION, 0.0, time_since_fade_change)
        };

        if orbit_center_fade > 0.001 {
            let half_line_length = orbit_eye.orbit_radius * 0.03;

            let half_line_length = half_line_length * orbit_center_fade;

            line_builder
                .batch("center orbit orientation help")
                .add_segments(glam::Vec3::AXES.iter().map(|axis| {
                    (
                        orbit_eye.orbit_center - *axis * half_line_length,
                        orbit_eye.orbit_center + *axis * half_line_length,
                    )
                }))
                .radius(Size::new_points(0.75))
                // TODO(andreas): Fade this out.
                .color(re_renderer::Color32::WHITE);

            // TODO(andreas): Idea for nice depth perception:
            // Render the lines once with additive blending and depth test enabled
            // and another time without depth test. In both cases it needs to be rendered last,
            // something re_renderer doesn't support yet for primitives within renderers.

            ui.ctx().request_repaint(); // show it for a bit longer.
        }
    }

    for draw_data in draw_data {
        view_builder.queue_draw(draw_data);
    }
    if let Ok(shared_render_builders) = view_ctx.get::<SharedRenderBuilders>() {
        for draw_data in shared_render_builders.queuable_draw_data(ctx.render_ctx) {
            view_builder.queue_draw(draw_data);
        }
    }

    // Commit ui induced lines.
    match line_builder.into_draw_data(ctx.render_ctx) {
        Ok(line_draw_data) => {
            view_builder.queue_draw(line_draw_data);
        }
        Err(err) => {
            re_log::error_once!("Failed to create draw data for lines from ui interaction: {err}");
        }
    }

    // Composite viewbuilder into egui.
    view_builder.queue_draw(re_renderer::renderer::GenericSkyboxDrawData::new(
        ctx.render_ctx,
    ));
    ui.painter().add(gpu_bridge::new_renderer_callback(
        view_builder,
        rect,
        re_renderer::Rgba::TRANSPARENT,
    ));

    // Add egui driven labels on top of re_renderer content.
    let painter = ui.painter().with_clip_rect(ui.max_rect());
    painter.extend(label_shapes);

    Ok(())
}

fn show_projections_from_2d_space(
    line_builder: &mut LineStripSeriesBuilder,
    space_cameras: &[SpaceCamera3D],
    state: &SpatialSpaceViewState,
    space_context: &SelectedSpaceContext,
    color: egui::Color32,
) {
    match space_context {
        SelectedSpaceContext::TwoD { space_2d, pos } => {
            if let Some(cam) = space_cameras.iter().find(|cam| &cam.ent_path == space_2d) {
                if let Some(pinhole) = cam.pinhole.as_ref() {
                    // Render a thick line to the actual z value if any and a weaker one as an extension
                    // If we don't have a z value, we only render the thick one.
                    let depth = if 0.0 < pos.z && pos.z.is_finite() {
                        pos.z
                    } else {
                        cam.picture_plane_distance
                    };
                    let stop_in_image_plane = pinhole.unproject(glam::vec3(pos.x, pos.y, depth));

                    let world_from_image = glam::Affine3A::from(cam.world_from_camera)
                        * glam::Affine3A::from_mat3(
                            cam.pinhole_view_coordinates
                                .from_other(&image_view_coordinates()),
                        );
                    let stop_in_world = world_from_image.transform_point3(stop_in_image_plane);

                    let origin = cam.position();
                    let ray =
                        macaw::Ray3::from_origin_dir(origin, (stop_in_world - origin).normalize());

                    let thick_ray_length = (stop_in_world - origin).length();
                    add_picking_ray(
                        line_builder,
                        ray,
                        &state.scene_bbox_accum,
                        thick_ray_length,
                        color,
                    );
                }
            }
        }
        SelectedSpaceContext::ThreeD {
            pos: Some(pos),
            tracked_space_camera: Some(camera_path),
            ..
        } => {
            let current_tracked_camera = state.state_3d.tracked_camera.as_ref();
            if current_tracked_camera.map_or(true, |tracked| tracked != camera_path) {
                if let Some(cam) = space_cameras
                    .iter()
                    .find(|cam| &cam.ent_path == camera_path)
                {
                    let cam_to_pos = *pos - cam.position();
                    let distance = cam_to_pos.length();
                    let ray = macaw::Ray3::from_origin_dir(cam.position(), cam_to_pos / distance);
                    add_picking_ray(line_builder, ray, &state.scene_bbox_accum, distance, color);
                }
            }
        }
        SelectedSpaceContext::ThreeD { .. } => {}
    }
}

fn add_picking_ray(
    line_builder: &mut LineStripSeriesBuilder,
    ray: macaw::Ray3,
    scene_bbox_accum: &BoundingBox,
    thick_ray_length: f32,
    color: egui::Color32,
) {
    let mut line_batch = line_builder.batch("picking ray");

    let origin = ray.point_along(0.0);
    // No harm in making this ray _very_ long. (Infinite messes with things though!)
    let fallback_ray_end = ray.point_along(scene_bbox_accum.size().length() * 10.0);
    let main_ray_end = ray.point_along(thick_ray_length);

    line_batch
        .add_segment(origin, main_ray_end)
        .color(color)
        .radius(Size::new_points(1.0));
    line_batch
        .add_segment(main_ray_end, fallback_ray_end)
        .color(color.gamma_multiply(0.7))
        // TODO(andreas): Make this dashed.
        .radius(Size::new_points(0.5));
}

fn default_eye(
    scene_bbox: &macaw::BoundingBox,
    view_coordinates: &Option<ViewCoordinates>,
) -> OrbitEye {
    let mut center = scene_bbox.center();
    if !center.is_finite() {
        center = Vec3::ZERO;
    }

    let mut radius = 1.5 * scene_bbox.half_size().length();
    if !radius.is_finite() || radius == 0.0 {
        radius = 1.0;
    }

    let look_up: glam::Vec3 = view_coordinates
        .and_then(|vc| vc.up())
        .unwrap_or(re_types::view_coordinates::SignedAxis3::POSITIVE_Z)
        .into();

    let look_dir = if let Some(right) = view_coordinates.and_then(|vc| vc.right()) {
        // Make sure right is to the right, and up is up:
        let right = right.into();
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
        view_coordinates
            .and_then(|vc| vc.up())
            .map_or(glam::Vec3::ZERO, Into::into),
    )
}
