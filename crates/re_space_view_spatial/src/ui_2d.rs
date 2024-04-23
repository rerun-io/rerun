use egui::{emath::RectTransform, pos2, vec2, Align2, Color32, Pos2, Rect, Shape, Vec2};
use macaw::IsoTransform;

use re_entity_db::EntityPath;
use re_renderer::view_builder::{TargetConfiguration, ViewBuilder};
use re_space_view::controls::{DRAG_PAN2D_BUTTON, RESET_VIEW_BUTTON_TEXT, ZOOM_SCROLL_MODIFIER};
use re_types::{archetypes::Pinhole, components::ViewCoordinates};
use re_viewer_context::{
    gpu_bridge, ItemSpaceContext, SpaceViewSystemExecutionError, SystemExecutionOutput, ViewQuery,
    ViewerContext,
};

use super::{
    eye::Eye,
    ui::{create_labels, picking, screenshot_context_menu},
};
use crate::{
    query_pinhole,
    ui::{outline_config, SpatialSpaceViewState},
    view_kind::SpatialSpaceViewKind,
    visualizers::collect_ui_labels,
};

// ---

#[derive(Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct View2DState {
    /// The visible parts of the scene, in the coordinate space of the scene.
    pub bounds: Option<Rect>,
}

impl View2DState {
    /// Pan and and zoom, and return the current transform.
    fn ui_from_scene(
        &mut self,
        response: &egui::Response,
        default_scene_rect: Rect,
    ) -> RectTransform {
        if response.double_clicked() {
            self.bounds = None; // double-click to reset
        }

        let bounds = self.bounds.get_or_insert(default_scene_rect);
        if !bounds.is_positive() {
            *bounds = default_scene_rect;
        }
        if !bounds.is_positive() {
            // Nothing in scene, probably.
            // Just return something that isn't NaN.
            let fake_bounds = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0));
            return RectTransform::from_to(fake_bounds, response.rect);
        }

        // --------------------------------------------------------------------------
        // Expand bounds for uniform scaling (letterboxing):

        let mut letterboxed_bounds = *bounds;

        // Temporary before applying letterboxing:
        let ui_from_scene = RectTransform::from_to(*bounds, response.rect);

        let scale_aspect = ui_from_scene.scale().x / ui_from_scene.scale().y;
        if scale_aspect < 1.0 {
            // Letterbox top/bottom:
            let add = bounds.height() * (1.0 / scale_aspect - 1.0);
            letterboxed_bounds.min.y -= 0.5 * add;
            letterboxed_bounds.max.y += 0.5 * add;
        } else {
            // Letterbox sides:
            let add = bounds.width() * (scale_aspect - 1.0);
            letterboxed_bounds.min.x -= 0.5 * add;
            letterboxed_bounds.max.x += 0.5 * add;
        }

        // --------------------------------------------------------------------------

        // Temporary before applying panning/zooming delta:
        let ui_from_scene = RectTransform::from_to(letterboxed_bounds, response.rect);

        // --------------------------------------------------------------------------

        let mut pan_delta_in_ui = response.drag_delta();
        if response.hovered() {
            pan_delta_in_ui += response.ctx.input(|i| i.raw_scroll_delta);
        }
        if pan_delta_in_ui != Vec2::ZERO {
            *bounds = bounds.translate(-pan_delta_in_ui / ui_from_scene.scale());
        }

        if response.hovered() {
            let zoom_delta = response.ctx.input(|i| i.zoom_delta_2d());

            if zoom_delta != Vec2::splat(1.0) {
                let zoom_center_in_ui = response
                    .hover_pos()
                    .unwrap_or_else(|| response.rect.center());
                let zoom_center_in_scene = ui_from_scene
                    .inverse()
                    .transform_pos(zoom_center_in_ui)
                    .to_vec2();
                *bounds = scale_rect(
                    bounds.translate(-zoom_center_in_scene),
                    Vec2::splat(1.0) / zoom_delta,
                )
                .translate(zoom_center_in_scene);
            }
        }

        // --------------------------------------------------------------------------

        RectTransform::from_to(letterboxed_bounds, response.rect)
    }
}

fn scale_rect(rect: Rect, factor: Vec2) -> Rect {
    Rect::from_min_max(
        (factor * rect.min.to_vec2()).to_pos2(),
        (factor * rect.max.to_vec2()).to_pos2(),
    )
}

pub fn help_text(re_ui: &re_ui::ReUi) -> egui::WidgetText {
    let mut layout = re_ui::LayoutJobBuilder::new(re_ui);

    layout.add(ZOOM_SCROLL_MODIFIER);
    layout.add(" + scroll to zoom.\n");

    layout.add("Click and drag with ");
    layout.add(DRAG_PAN2D_BUTTON);
    layout.add(" to pan.\n");

    layout.add_button_text(RESET_VIEW_BUTTON_TEXT);
    layout.add(" to reset the view.");

    layout.layout_job.into()
}

/// Create the outer 2D view, which consists of a scrollable region
pub fn view_2d(
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

    if ui.available_size().min_elem() <= 0.0 {
        return Ok(());
    }

    let pinhole = query_pinhole(ctx.recording(), &ctx.current_query(), query.space_origin);

    let default_scene_rect = if let Some(res) = pinhole.as_ref().and_then(|p| p.resolution()) {
        // Note that we can't rely on the camera being part of scene.space_cameras since that requires
        // the camera to be added to the scene!
        Rect::from_min_max(Pos2::ZERO, pos2(res.x, res.y))
    } else {
        let scene_rect_accum = state.bounding_boxes.accumulated;
        egui::Rect::from_min_max(
            scene_rect_accum.min.truncate().to_array().into(),
            scene_rect_accum.max.truncate().to_array().into(),
        )
    };

    let (mut response, painter) =
        ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

    // Convert ui coordinates to/from scene coordinates.
    let ui_from_scene = state.state_2d.ui_from_scene(&response, default_scene_rect);
    let scene_from_ui = ui_from_scene.inverse();

    // TODO(andreas): Use the same eye & transformations as in `setup_target_config`.
    let eye = Eye {
        world_from_rub_view: IsoTransform::IDENTITY,
        fov_y: None,
    };

    let Ok(target_config) = setup_target_config(
        &painter,
        scene_from_ui,
        &query.space_origin.to_string(),
        state.auto_size_config(),
        query.highlights.any_outlines(),
        pinhole,
    ) else {
        return Ok(());
    };

    let mut view_builder = ViewBuilder::new(ctx.render_ctx, target_config);

    // Create labels now since their shapes participate are added to scene.ui for picking.
    let (label_shapes, ui_rects) = create_labels(
        collect_ui_labels(&parts),
        ui_from_scene,
        &eye,
        ui,
        &query.highlights,
        SpatialSpaceViewKind::TwoD,
    );

    if ui.ctx().dragged_id().is_none() {
        response = picking(
            ctx,
            response,
            scene_from_ui,
            painter.clip_rect(),
            ui,
            eye,
            &mut view_builder,
            state,
            &view_ctx,
            &parts,
            &ui_rects,
            query,
            SpatialSpaceViewKind::TwoD,
        )?;
    }

    for draw_data in draw_data {
        view_builder.queue_draw(draw_data);
    }

    // ------------------------------------------------------------------------

    if let Some(mode) = screenshot_context_menu(ctx, &response) {
        view_builder
            .schedule_screenshot(ctx.render_ctx, query.space_view_id.gpu_readback_id(), mode)
            .ok();
    }

    // Draw a re_renderer driven view.
    // Camera & projection are configured to ingest space coordinates directly.
    painter.add(gpu_bridge::new_renderer_callback(
        view_builder,
        painter.clip_rect(),
        ui.visuals().extreme_bg_color.into(),
    ));

    // Make sure to _first_ draw the selected, and *then* the hovered context on top!
    for selected_context in ctx.selection_state().selection_space_contexts() {
        painter.extend(show_projections_from_3d_space(
            ui,
            query.space_origin,
            &ui_from_scene,
            selected_context,
            ui.style().visuals.selection.bg_fill,
        ));
    }
    if let Some(hovered_context) = ctx.selection_state().hovered_space_context() {
        painter.extend(show_projections_from_3d_space(
            ui,
            query.space_origin,
            &ui_from_scene,
            hovered_context,
            egui::Color32::WHITE,
        ));
    }

    // Add egui driven labels on top of re_renderer content.
    painter.extend(label_shapes);

    Ok(())
}

fn setup_target_config(
    egui_painter: &egui::Painter,
    scene_from_ui: RectTransform,
    space_name: &str,
    auto_size_config: re_renderer::AutoSizeConfig,
    any_outlines: bool,
    pinhole: Option<Pinhole>,
) -> anyhow::Result<TargetConfiguration> {
    let pixels_from_points = egui_painter.ctx().pixels_per_point();
    let resolution_in_pixel =
        gpu_bridge::viewport_resolution_in_pixels(egui_painter.clip_rect(), pixels_from_points);
    anyhow::ensure!(resolution_in_pixel[0] > 0 && resolution_in_pixel[1] > 0);

    // TODO(#1025):
    // The camera setup is done in a way that works well with the way we inverse pinhole camera transformations right now.
    // This has a lot of issues though, mainly because we pretend that the 2D plane has a defined depth.
    // * very bad depth precision as we limit the depth range from 0 to focal_length_in_pixels
    // * depth values in depth buffer are almost non-sensical and can't be used easily for picking
    // * 2D rendering can use depth buffer for layering only in a very limited way
    //
    // Instead we should treat 2D objects as pre-projected with their depth information already lost.
    //
    // We would define two cameras then:
    // * an orthographic camera for handling 2D rendering
    // * a perspective camera *at the origin* for 3D rendering
    // Both share the same view-builder and the same viewport transformation but are independent otherwise.

    // For simplicity (and to reduce surprises!) we always render with a pinhole camera.
    // Make up a default pinhole camera if we don't have one placing it in a way to look at the entire space.
    let scene_bounds_size = glam::vec2(scene_from_ui.to().width(), scene_from_ui.to().height());
    let default_principal_point = scene_from_ui.to().center();
    let default_principal_point = glam::vec2(default_principal_point.x, default_principal_point.y);
    let pinhole = pinhole.unwrap_or_else(|| {
        let focal_length_in_pixels = scene_bounds_size.x;

        Pinhole {
            image_from_camera: glam::Mat3::from_cols(
                glam::vec3(focal_length_in_pixels, 0.0, 0.0),
                glam::vec3(0.0, focal_length_in_pixels, 0.0),
                default_principal_point.extend(1.0),
            )
            .into(),
            resolution: Some(scene_bounds_size.into()),
            camera_xyz: Some(ViewCoordinates::RDF),
        }
    });

    let projection_from_view = re_renderer::view_builder::Projection::Perspective {
        vertical_fov: pinhole.fov_y().unwrap_or(Eye::DEFAULT_FOV_Y),
        near_plane_distance: 0.1,
        aspect_ratio: pinhole
            .aspect_ratio()
            .unwrap_or(scene_bounds_size.x / scene_bounds_size.y),
    };

    // Put the camera at the position where it sees the entire image plane as defined
    // by the pinhole camera.
    // TODO(andreas): Support anamorphic pinhole cameras properly.
    let focal_length = pinhole.focal_length_in_pixels();
    let focal_length = 2.0 / (1.0 / focal_length.x() + 1.0 / focal_length.y()); // harmonic mean
    let Some(view_from_world) = macaw::IsoTransform::look_at_rh(
        pinhole.principal_point().extend(-focal_length),
        pinhole.principal_point().extend(0.0),
        -glam::Vec3::Y,
    ) else {
        anyhow::bail!("Failed to compute camera transform for 2D view.");
    };

    // Cut to the portion of the currently visible ui area.
    let mut viewport_transformation = re_renderer::RectTransform {
        region_of_interest: re_render_rect_from_egui_rect(egui_painter.clip_rect()),
        region: re_render_rect_from_egui_rect(*scene_from_ui.from()),
    };

    // The principal point might not be quite centered.
    // We need to account for this translation in the viewport transformation.
    let principal_point_offset = default_principal_point - pinhole.principal_point();
    let ui_from_scene_scale = scene_from_ui.inverse().scale();
    viewport_transformation.region_of_interest.min +=
        principal_point_offset * glam::vec2(ui_from_scene_scale.x, ui_from_scene_scale.y);

    Ok({
        let name = space_name.into();
        TargetConfiguration {
            name,
            resolution_in_pixel,
            view_from_world,
            projection_from_view,
            viewport_transformation,
            pixels_from_point: pixels_from_points,
            auto_size_config,
            outline_config: any_outlines.then(|| outline_config(egui_painter.ctx())),
        }
    })
}

fn re_render_rect_from_egui_rect(rect: egui::Rect) -> re_renderer::RectF32 {
    re_renderer::RectF32 {
        min: glam::vec2(rect.left(), rect.top()),
        extent: glam::vec2(rect.width(), rect.height()),
    }
}

// ------------------------------------------------------------------------

fn show_projections_from_3d_space(
    ui: &egui::Ui,
    space: &EntityPath,
    ui_from_scene: &RectTransform,
    space_context: &ItemSpaceContext,
    color: egui::Color32,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    if let ItemSpaceContext::ThreeD {
        point_in_space_cameras: target_spaces,
        ..
    } = space_context
    {
        for (space_2d, pos_2d) in target_spaces {
            if space_2d == space {
                if let Some(pos_2d) = pos_2d {
                    // User is hovering a 2D point inside a 3D view.
                    let pos_in_ui = ui_from_scene.transform_pos(pos2(pos_2d.x, pos_2d.y));
                    let radius = 4.0;
                    shapes.push(Shape::circle_filled(
                        pos_in_ui,
                        radius + 2.0,
                        Color32::BLACK,
                    ));
                    shapes.push(Shape::circle_filled(pos_in_ui, radius, color));

                    let text_color = Color32::WHITE;
                    let text = format!("Depth: {:.3} m", pos_2d.z);
                    let font_id = egui::TextStyle::Body.resolve(ui.style());
                    let galley = ui.fonts(|fonts| fonts.layout_no_wrap(text, font_id, text_color));
                    let rect = Align2::CENTER_TOP.anchor_rect(Rect::from_min_size(
                        pos_in_ui + vec2(0.0, 5.0),
                        galley.size(),
                    ));
                    shapes.push(Shape::rect_filled(
                        rect,
                        2.0,
                        Color32::from_black_alpha(196),
                    ));
                    shapes.push(Shape::galley(rect.min, galley, text_color));
                }
            }
        }
    }
    shapes
}
