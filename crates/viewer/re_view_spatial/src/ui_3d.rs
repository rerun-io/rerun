use egui::emath::RectTransform;
use egui::{Modifiers, NumExt as _};
use glam::Vec3;
use macaw::BoundingBox;
use re_chunk_store::MissingChunkReporter;
use re_log_types::Instance;
use re_renderer::view_builder::{Projection, TargetConfiguration, ViewBuilder};
use re_renderer::{LineDrawableBuilder, Size};
use re_sdk_types::blueprint::archetypes::{
    Background, EyeControls3D, LineGrid3D, SpatialInformation,
};
use re_sdk_types::blueprint::components::{Enabled, GridSpacing};
use re_sdk_types::components::{ViewCoordinates, Visible};
use re_tf::{image_view_coordinates, query_view_coordinates_at_closest_ancestor};
use re_ui::{ContextExt as _, Help, IconText, MouseButtonText, UiExt as _, icons};
use re_view::controls::{
    DRAG_PAN3D_BUTTON, ROLL_MOUSE_ALT, ROLL_MOUSE_MODIFIER, ROTATE3D_BUTTON, RuntimeModifiers,
    SPEED_UP_3D_MODIFIER, TRACKED_OBJECT_RESTORE_KEY,
};
use re_viewer_context::{
    Item, ItemContext, ViewClassExt as _, ViewContext, ViewQuery, ViewSystemExecutionError,
    ViewerContext, gpu_bridge,
};
use re_viewport_blueprint::ViewProperty;

use super::eye::{Eye, EyeState};
use crate::SpatialView3D;
use crate::eye::find_camera;
use crate::pinhole_wrapper::PinholeWrapper;
use crate::ui::{SpatialViewState, create_labels};
use crate::view_kind::SpatialViewKind;
use crate::visualizers::{CamerasVisualizer, collect_ui_labels};

// ---

#[derive(Clone)]
pub struct View3DState {
    pub eye_state: EyeState,

    /// Last known view coordinates.
    /// Used to detect changes in view coordinates, in which case we reset the camera eye.
    pub scene_view_coordinates: Option<ViewCoordinates>,

    eye_interact_fade_in: bool,
    eye_interact_fade_change_time: f64,

    pub show_smoothed_bbox: bool,
}

impl Default for View3DState {
    fn default() -> Self {
        Self {
            eye_state: Default::default(),
            scene_view_coordinates: None,
            eye_interact_fade_in: false,
            eye_interact_fade_change_time: f64::NEG_INFINITY,
            show_smoothed_bbox: false,
        }
    }
}

impl View3DState {
    pub fn reset_eye(&mut self, ctx: &ViewerContext<'_>, eye_property: &ViewProperty) {
        eye_property.reset_all_components(ctx);

        let last_eye = self.eye_state.last_eye;
        self.eye_state = Default::default();
        self.eye_state.last_eye = last_eye;

        self.eye_state.start_interpolation();
    }

    fn update(&mut self, scene_view_coordinates: Option<ViewCoordinates>) {
        // Detect live changes to view coordinates, and interpolate to the new up axis as needed.
        if scene_view_coordinates != self.scene_view_coordinates {
            self.eye_state.start_interpolation();
        }
        self.scene_view_coordinates = scene_view_coordinates;
    }
}

// ----------------------------------------------------------------------------

pub fn help(os: egui::os::OperatingSystem) -> Help {
    Help::new("3D view")
        .docs_link("https://rerun.io/docs/reference/types/views/spatial3d_view")
        .control("Pan", (MouseButtonText(DRAG_PAN3D_BUTTON), "+", "drag"))
        .control("Zoom", icons::SCROLL)
        .control("Rotate", (MouseButtonText(ROTATE3D_BUTTON), "+", "drag"))
        .control(
            "Roll",
            IconText::from_modifiers_and(os, ROLL_MOUSE_MODIFIER, MouseButtonText(ROLL_MOUSE_ALT)),
        )
        .control("Navigate", ("WASD", "/", "QE"))
        .control(
            "Slow down / speed up",
            (
                IconText::from_modifiers(os, RuntimeModifiers::slow_down(&os)),
                "/",
                IconText::from_modifiers(os, SPEED_UP_3D_MODIFIER),
            ),
        )
        .control("Focus", ("double", icons::LEFT_MOUSE_CLICK, "object"))
        .control(
            "Track",
            (
                IconText::from_modifiers(os, Modifiers::ALT),
                "+",
                "double",
                icons::LEFT_MOUSE_CLICK,
                "object",
            ),
        )
        .control(
            "Reset view",
            ("double", icons::LEFT_MOUSE_CLICK, "background"),
        )
}

impl SpatialView3D {
    pub fn view_3d(
        &self,
        ctx: &ViewerContext<'_>,
        missing_chunk_reporter: &MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut SpatialViewState,
        query: &ViewQuery<'_>,
        mut system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let highlights = &query.highlights;
        let space_cameras = &system_output
            .view_systems
            .get::<CamerasVisualizer>()?
            .pinhole_cameras;
        let scene_view_coordinates = query_view_coordinates_at_closest_ancestor(
            query.space_origin,
            ctx.recording(),
            &ctx.current_query(),
        );

        let (ui_rect, response) =
            ui.allocate_at_least(ui.available_size(), egui::Sense::click_and_drag());

        if !ui_rect.is_positive() {
            return Ok(()); // protect against problems with zero-sized views
        }

        let mut state_3d = state.state_3d.clone();

        let view_context = self.view_context(ctx, query.view_id, state, query.space_origin);

        let information_property = ViewProperty::from_archetype::<SpatialInformation>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        let show_axes = **information_property.component_or_fallback::<Enabled>(
            &view_context,
            SpatialInformation::descriptor_show_axes().component,
        )?;
        let show_bounding_box = **information_property.component_or_fallback::<Enabled>(
            &view_context,
            SpatialInformation::descriptor_show_bounding_box().component,
        )?;
        state_3d.update(scene_view_coordinates);

        let eye = state_3d.eye_state.update(
            &view_context,
            &response,
            space_cameras,
            &state.bounding_boxes,
        )?;

        state.state_3d = state_3d;

        // Determine view port resolution and position.
        let resolution_in_pixel =
            gpu_bridge::viewport_resolution_in_pixels(ui_rect, ui.ctx().pixels_per_point());
        if resolution_in_pixel[0] == 0 || resolution_in_pixel[1] == 0 {
            return Ok(());
        }

        // Various ui interactions draw additional lines.
        let mut line_builder = LineDrawableBuilder::new(ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );
        // We don't know ahead of time how many lines we need, but it's not gonna be a huge amount!
        line_builder.reserve_strips(32)?;
        line_builder.reserve_vertices(64)?;

        // Origin gizmo if requested.
        // TODO(andreas): Move this to the transform3d_arrow scene part.
        //              As of #2522 state is now longer accessible there, move the property to a context?
        if show_axes {
            let axis_length = 1.0; // The axes are also a measuring stick
            crate::visualizers::add_axis_arrows(
                ctx.tokens(),
                &mut line_builder,
                glam::Affine3A::IDENTITY,
                None,
                axis_length,
                re_renderer::OutlineMaskPreference::NONE,
                Instance::ALL.get(),
            );

            // If we are showing the axes for the space, then add the space origin to the bounding box.
            state.bounding_boxes.current.extend(glam::Vec3::ZERO);
        }

        // Create labels now since their shapes participate are added to scene.ui for picking.
        let (label_shapes, ui_rects) = create_labels(
            collect_ui_labels(&system_output.view_systems),
            RectTransform::from_to(ui_rect, ui_rect),
            &eye,
            ui,
            highlights,
            SpatialViewKind::ThreeD,
        );

        let (response, picking_config) = if let Some(pointer_pos_ui) = response.hover_pos() {
            // There's no panning & zooming, so this is an identity transform.
            let ui_pan_and_zoom_from_ui = RectTransform::from_to(ui_rect, ui_rect);

            let picking_context = crate::picking::PickingContext::new(
                pointer_pos_ui,
                ui_pan_and_zoom_from_ui,
                ui.ctx().pixels_per_point(),
                &eye,
            );
            crate::picking_ui::picking(
                ctx,
                missing_chunk_reporter,
                &picking_context,
                ui,
                response,
                state,
                &system_output,
                &ui_rects,
                query,
                SpatialViewKind::ThreeD,
            )?
        } else {
            state.previous_picking_result = None;
            (response, None)
        };

        let target_config = TargetConfiguration {
            name: query.space_origin.to_string().into(),
            render_mode: ctx.render_mode(),

            resolution_in_pixel,

            view_from_world: eye.world_from_rub_view.inverse(),
            projection_from_view: Projection::Perspective {
                vertical_fov: eye.fov_y.unwrap_or(Eye::DEFAULT_FOV_Y),
                near_plane_distance: eye.near(),
                aspect_ratio: resolution_in_pixel[0] as f32 / resolution_in_pixel[1] as f32,
            },
            viewport_transformation: re_renderer::RectTransform::IDENTITY,

            pixels_per_point: ui.ctx().pixels_per_point(),

            outline_config: query
                .highlights
                .any_outlines()
                .then(|| re_view::outline_config(ui.ctx())),
            blend_with_background: false,
            picking_config,
        };

        let mut view_builder = ViewBuilder::new(ctx.render_ctx(), target_config)?;

        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        // Track focused entity if any.
        if let Some(focused_item) = ctx.focused_item() {
            let focused_entity = match focused_item {
                Item::AppId(_)
                | Item::DataSource(_)
                | Item::StoreId(_)
                | Item::Container(_)
                | Item::RedapEntry(_)
                | Item::RedapServer(_)
                | Item::TableId(_) => None,

                Item::View(view_id) => {
                    if view_id == &query.view_id {
                        state.state_3d.reset_eye(ctx, &eye_property);
                    }
                    None
                }

                Item::ComponentPath(component_path) => Some(&component_path.entity_path),

                Item::InstancePath(instance_path) => Some(&instance_path.entity_path),

                Item::DataResult(data_result) => {
                    if data_result.view_id == query.view_id {
                        Some(&data_result.instance_path.entity_path)
                    } else {
                        None
                    }
                }
            };

            if let Some(entity_path) = focused_entity {
                if ui.ctx().input(|i| i.modifiers.alt)
                    || find_camera(space_cameras, entity_path).is_some()
                {
                    if state.last_tracked_entity() != Some(entity_path) {
                        eye_property.save_blueprint_component(
                            ctx,
                            &EyeControls3D::descriptor_tracking_entity(),
                            &re_sdk_types::components::EntityPath::from(entity_path),
                        );
                        state.state_3d.eye_state.last_interaction_time = Some(ui.time());
                    }
                } else {
                    state.state_3d.eye_state.start_interpolation();
                    state.state_3d.eye_state.focus_entity(
                        &self.view_context(ctx, query.view_id, state, query.space_origin),
                        space_cameras,
                        &state.bounding_boxes,
                        &eye_property,
                        entity_path,
                    )?;
                }
            }

            // Make sure focus consequences happen in the next frames.
            ui.ctx().request_repaint();
        }

        // Allow to restore the camera state with escape if a camera was tracked before.
        if response.hovered() && ui.input(|i| i.key_pressed(TRACKED_OBJECT_RESTORE_KEY)) {
            eye_property
                .clear_blueprint_component(ctx, EyeControls3D::descriptor_tracking_entity());
        }

        for selected_context in ctx.selection_state().selection_item_contexts() {
            show_projections_from_2d_space(
                &mut line_builder,
                space_cameras,
                state,
                selected_context,
                ui.ctx().selection_stroke().color,
            );
        }
        if let Some(hovered_context) = ctx.selection_state().hovered_item_context() {
            show_projections_from_2d_space(
                &mut line_builder,
                space_cameras,
                state,
                hovered_context,
                ui.ctx().hover_stroke().color,
            );
        }

        // TODO(andreas): Make configurable. Could pick up default radius for this view?
        let box_line_radius = Size(*re_sdk_types::components::Radius::default().0);

        if show_bounding_box {
            line_builder
                .batch("scene_bbox_current")
                .add_box_outline(&state.bounding_boxes.current)
                .map(|lines| {
                    lines
                        .radius(box_line_radius)
                        .color(ui.tokens().frustum_color)
                });
        }
        if state.state_3d.show_smoothed_bbox {
            line_builder
                .batch("scene_bbox_smoothed")
                .add_box_outline(&state.bounding_boxes.smoothed)
                .map(|lines| {
                    lines
                        .radius(box_line_radius)
                        .color(ctx.tokens().frustum_color)
                });
        }

        show_orbit_eye_center(
            ui.ctx(),
            &mut state.state_3d,
            &mut line_builder,
            scene_view_coordinates,
        );

        for draw_data in system_output.drain_draw_data() {
            view_builder.queue_draw(ctx.render_ctx(), draw_data);
        }

        let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);

        // Optional 3D line grid.
        let grid_config = ViewProperty::from_archetype::<LineGrid3D>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        if let Some(draw_data) = Self::setup_grid_3d(&view_ctx, &grid_config)? {
            view_builder.queue_draw(ctx.render_ctx(), draw_data);
        }

        // Commit ui induced lines.
        view_builder.queue_draw(ctx.render_ctx(), line_builder.into_draw_data()?);

        let background = ViewProperty::from_archetype::<Background>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let (background_drawable, clear_color) =
            crate::configure_background(&view_ctx, &background)?;
        if let Some(background_drawable) = background_drawable {
            view_builder.queue_draw(ctx.render_ctx(), background_drawable);
        }

        ui.painter().add(gpu_bridge::new_renderer_callback(
            view_builder,
            ui_rect,
            clear_color,
        ));

        // Add egui-rendered loading indicators on top of re_renderer content:
        crate::ui::paint_loading_indicators(
            ui,
            RectTransform::from_to(ui_rect, ui_rect),
            &eye,
            &system_output.view_systems,
        );

        // Add egui-rendered labels on top of everything else:
        let painter = ui.painter().with_clip_rect(ui.max_rect());
        painter.extend(label_shapes);

        Ok(())
    }

    fn setup_grid_3d(
        ctx: &ViewContext<'_>,
        grid_config: &ViewProperty,
    ) -> Result<Option<re_renderer::renderer::WorldGridDrawData>, ViewSystemExecutionError> {
        if !**grid_config
            .component_or_fallback::<Visible>(ctx, LineGrid3D::descriptor_visible().component)?
        {
            return Ok(None);
        }

        let spacing = **grid_config.component_or_fallback::<GridSpacing>(
            ctx,
            LineGrid3D::descriptor_spacing().component,
        )?;
        let thickness_ui = **grid_config
            .component_or_fallback::<re_sdk_types::components::StrokeWidth>(
                ctx,
                LineGrid3D::descriptor_stroke_width().component,
            )?;
        let color = grid_config.component_or_fallback::<re_sdk_types::components::Color>(
            ctx,
            LineGrid3D::descriptor_color().component,
        )?;
        let plane = grid_config.component_or_fallback::<re_sdk_types::components::Plane3D>(
            ctx,
            LineGrid3D::descriptor_plane().component,
        )?;

        Ok(Some(re_renderer::renderer::WorldGridDrawData::new(
            ctx.render_ctx(),
            &re_renderer::renderer::WorldGridConfiguration {
                color: color.into(),
                plane: plane.into(),
                spacing,
                thickness_ui,
            },
        )))
    }
}

/// Show center of orbit camera when interacting with camera (it's quite helpful).
fn show_orbit_eye_center(
    egui_ctx: &egui::Context,
    state_3d: &mut View3DState,
    line_builder: &mut LineDrawableBuilder<'_>,
    scene_view_coordinates: Option<ViewCoordinates>,
) {
    // These are only none at the start or just as the view resets so can
    // skip displaying anything then.
    let Some(look_target) = state_3d.eye_state.last_look_target else {
        return;
    };
    let Some(orbit_radius) = state_3d.eye_state.last_orbit_radius else {
        return;
    };
    let Some(up) = state_3d.eye_state.last_eye_up else {
        return;
    };

    const FADE_DURATION: f32 = 0.1;

    let ui_time = egui_ctx.input(|i| i.time);
    let any_mouse_button_down = egui_ctx.input(|i| i.pointer.any_down());

    let should_show_center_of_orbit_camera = state_3d
        .eye_state
        .last_interaction_time
        .is_some_and(|time| (egui_ctx.time() - time) < 0.35);

    if !state_3d.eye_interact_fade_in && should_show_center_of_orbit_camera {
        // Any interaction immediately causes fade in to start if it's not already on.
        state_3d.eye_interact_fade_change_time = ui_time;
        state_3d.eye_interact_fade_in = true;
    }
    if state_3d.eye_interact_fade_in
            && !should_show_center_of_orbit_camera
            // Don't start fade-out while dragging, even if mouse is still
            && !any_mouse_button_down
    {
        state_3d.eye_interact_fade_change_time = ui_time;
        state_3d.eye_interact_fade_in = false;
    }

    pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = f32::clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
        t * t * (3.0 - t * 2.0)
    }

    // Compute smooth fade.
    let time_since_fade_change = (ui_time - state_3d.eye_interact_fade_change_time) as f32;
    let orbit_center_fade = if state_3d.eye_interact_fade_in {
        // Fade in.
        smoothstep(0.0, FADE_DURATION, time_since_fade_change)
    } else {
        // Fade out.
        smoothstep(FADE_DURATION, 0.0, time_since_fade_change)
    };

    if orbit_center_fade > 0.001 {
        let half_line_length = orbit_radius * 0.03;
        let half_line_length = half_line_length * orbit_center_fade;

        // For the other two axes, try to use the scene view coordinates if available:
        let right = scene_view_coordinates
            .and_then(|vc| vc.right())
            .map_or(glam::Vec3::X, Vec3::from);
        let forward = up
            .cross(right)
            .try_normalize()
            .unwrap_or_else(|| up.any_orthogonal_vector());
        let right = forward.cross(up);

        line_builder
            .batch("center orbit orientation help")
            .add_segments(
                [
                    (look_target, look_target + 0.5 * up * half_line_length),
                    (
                        look_target - right * half_line_length,
                        look_target + right * half_line_length,
                    ),
                    (
                        look_target - forward * half_line_length,
                        look_target + forward * half_line_length,
                    ),
                ]
                .into_iter(),
            )
            .radius(Size::new_ui_points(0.75))
            // TODO(andreas): Fade this out.
            .color(egui_ctx.tokens().frustum_color);

        // TODO(andreas): Idea for nice depth perception:
        // Render the lines once with additive blending and depth test enabled
        // and another time without depth test. In both cases it needs to be rendered last,
        // something re_renderer doesn't support yet for primitives within renderers.

        egui_ctx.request_repaint(); // show it for a bit longer.
    }
}

fn show_projections_from_2d_space(
    line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
    cameras: &[PinholeWrapper],
    state: &SpatialViewState,
    item_context: &ItemContext,
    ray_color: egui::Color32,
) {
    match item_context {
        ItemContext::TwoD { space_2d, pos } => {
            if let Some(cam) = cameras.iter().find(|cam| &cam.ent_path == space_2d) {
                // Render a thick line to the actual z value if any and a weaker one as an extension
                // If we don't have a z value, we only render the thick one.
                let depth = if 0.0 < pos.z && pos.z.is_finite() {
                    pos.z
                } else {
                    cam.picture_plane_distance
                };
                let stop_in_image_plane = cam.pinhole.unproject(glam::vec3(pos.x, pos.y, depth));

                let world_from_image = glam::Affine3A::from(cam.world_from_camera)
                    * glam::Affine3A::from_mat3(
                        cam.pinhole_view_coordinates
                            .from_other(&image_view_coordinates()),
                    );
                let stop_in_world = world_from_image.transform_point3(stop_in_image_plane);

                let origin = cam.position();

                if let Some(dir) = (stop_in_world - origin).try_normalize() {
                    let ray = macaw::Ray3::from_origin_dir(origin, dir);

                    let thick_ray_length = (stop_in_world - origin).length();
                    add_picking_ray(
                        line_builder,
                        ray,
                        &state.bounding_boxes.smoothed,
                        thick_ray_length,
                        ray_color,
                    );
                }
            }
        }
        ItemContext::ThreeD {
            pos: Some(pos),
            tracked_entity: Some(tracked_entity),
            ..
        } => {
            if state.last_tracked_entity() != Some(tracked_entity)
                && let Some(tracked_camera) =
                    cameras.iter().find(|cam| &cam.ent_path == tracked_entity)
            {
                let cam_to_pos = *pos - tracked_camera.position();
                let distance = cam_to_pos.length();
                let ray =
                    macaw::Ray3::from_origin_dir(tracked_camera.position(), cam_to_pos / distance);
                add_picking_ray(
                    line_builder,
                    ray,
                    &state.bounding_boxes.current,
                    distance,
                    ray_color,
                );
            }
        }
        ItemContext::ThreeD { .. }
        | ItemContext::StreamsTree { .. }
        | ItemContext::BlueprintTree { .. } => {}
    }
}

fn add_picking_ray(
    line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
    ray: macaw::Ray3,
    scene_bbox: &BoundingBox,
    thick_ray_length: f32,
    ray_color: egui::Color32,
) {
    let mut line_batch = line_builder.batch("picking ray");

    let origin = ray.point_along(0.0);

    // No harm in making this ray _very_ long. (Infinite messes with things though!)
    //
    // There are some degenerated cases where just taking the scene bounding box isn't enough:
    // For instance, we don't add pinholes & depth images to the bounding box since
    // the default size of a pinhole visualization itself is determined by the bounding box.
    let fallback_ray_end =
        ray.point_along((scene_bbox.size().length() * 10.0).at_least(thick_ray_length * 10.0));
    let main_ray_end = ray.point_along(thick_ray_length);

    line_batch
        .add_segment(origin, main_ray_end)
        .color(ray_color)
        .radius(Size::new_ui_points(1.0));
    line_batch
        .add_segment(main_ray_end, fallback_ray_end)
        .color(ray_color.gamma_multiply(0.7))
        // TODO(andreas): Make this dashed.
        .radius(Size::new_ui_points(0.5));
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(help);
}
