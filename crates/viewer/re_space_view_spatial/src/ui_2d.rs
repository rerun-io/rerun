use egui::{emath::RectTransform, pos2, vec2, Align2, Color32, Pos2, Rect, Shape, Vec2};
use re_math::IsoTransform;

use re_entity_db::EntityPath;
use re_log::ResultExt as _;
use re_renderer::view_builder::{TargetConfiguration, ViewBuilder};
use re_space_view::controls::{DRAG_PAN2D_BUTTON, ZOOM_SCROLL_MODIFIER};
use re_types::{
    archetypes::Pinhole,
    blueprint::{
        archetypes::{Background, VisualBounds2D},
        components as blueprint_components,
    },
    components::ViewCoordinates,
};
use re_ui::{ContextExt as _, ModifiersMarkdown, MouseButtonMarkdown};
use re_viewer_context::{
    gpu_bridge, ItemSpaceContext, SpaceViewId, SpaceViewSystemExecutionError, ViewQuery,
    ViewerContext,
};
use re_viewport_blueprint::ViewProperty;

use super::{
    eye::Eye,
    ui::{create_labels, screenshot_context_menu},
};
use crate::{
    query_pinhole_legacy, ui::SpatialSpaceViewState, view_kind::SpatialSpaceViewKind,
    visualizers::collect_ui_labels, SpatialSpaceView2D,
};

// ---

/// Pan and zoom, and return the current transform.
fn ui_from_scene(
    ctx: &ViewerContext<'_>,
    view_id: SpaceViewId,
    response: &egui::Response,
    view_class: &SpatialSpaceView2D,
    view_state: &mut SpatialSpaceViewState,
) -> RectTransform {
    let bounds_property = ViewProperty::from_archetype::<VisualBounds2D>(
        ctx.blueprint_db(),
        ctx.blueprint_query,
        view_id,
    );
    let bounds: blueprint_components::VisualBounds2D = bounds_property
        .component_or_fallback(ctx, view_class, view_state)
        .ok_or_log_error()
        .unwrap_or_default();
    view_state.visual_bounds_2d = Some(bounds);
    let mut bounds_rect: egui::Rect = bounds.into();

    // --------------------------------------------------------------------------
    // Expand bounds for uniform scaling (letterboxing):

    let mut letterboxed_bounds = bounds_rect;

    // Temporary before applying letterboxing:
    let ui_from_scene = RectTransform::from_to(bounds_rect, response.rect);

    let scale_aspect = ui_from_scene.scale().x / ui_from_scene.scale().y;
    if scale_aspect < 1.0 {
        // Letterbox top/bottom:
        let add = bounds_rect.height() * (1.0 / scale_aspect - 1.0);
        letterboxed_bounds.min.y -= 0.5 * add;
        letterboxed_bounds.max.y += 0.5 * add;
    } else {
        // Letterbox sides:
        let add = bounds_rect.width() * (scale_aspect - 1.0);
        letterboxed_bounds.min.x -= 0.5 * add;
        letterboxed_bounds.max.x += 0.5 * add;
    }

    // --------------------------------------------------------------------------

    // Temporary before applying panning/zooming delta:
    let ui_from_scene = RectTransform::from_to(letterboxed_bounds, response.rect);

    // --------------------------------------------------------------------------

    let mut pan_delta_in_ui = response.drag_delta();
    if response.hovered() {
        pan_delta_in_ui += response.ctx.input(|i| i.smooth_scroll_delta);
    }
    if pan_delta_in_ui != Vec2::ZERO {
        bounds_rect = bounds_rect.translate(-pan_delta_in_ui / ui_from_scene.scale());
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
            bounds_rect = scale_rect(
                bounds_rect.translate(-zoom_center_in_scene),
                Vec2::splat(1.0) / zoom_delta,
            )
            .translate(zoom_center_in_scene);
        }
    }

    // Update blueprint if changed
    let updated_bounds: blueprint_components::VisualBounds2D = bounds_rect.into();
    if response.double_clicked() {
        bounds_property.reset_blueprint_component::<blueprint_components::VisualBounds2D>(ctx);
    } else if bounds != updated_bounds {
        bounds_property.save_blueprint_component(ctx, &updated_bounds);
    }
    // Update stored bounds on the state, so visualizers see an up-to-date value.
    view_state.visual_bounds_2d = Some(bounds);

    RectTransform::from_to(letterboxed_bounds, response.rect)
}

fn scale_rect(rect: Rect, factor: Vec2) -> Rect {
    Rect::from_min_max(
        (factor * rect.min.to_vec2()).to_pos2(),
        (factor * rect.max.to_vec2()).to_pos2(),
    )
}

pub fn help_markdown(egui_ctx: &egui::Context) -> String {
    format!(
        "# 2D View

Display 2D content in the reference frame defined by the space origin.

## Navigation controls
- Pinch gesture or {zoom_scroll_modifier} + scroll to zoom.
- Click and drag with the {drag_pan2d_button} to pan.
- Double-click to reset the view.",
        zoom_scroll_modifier = ModifiersMarkdown(ZOOM_SCROLL_MODIFIER, egui_ctx),
        drag_pan2d_button = MouseButtonMarkdown(DRAG_PAN2D_BUTTON),
    )
    .to_owned()
}

/// Create the outer 2D view, which consists of a scrollable region
impl SpatialSpaceView2D {
    pub fn view_2d(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut SpatialSpaceViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        if ui.available_size().min_elem() <= 0.0 {
            return Ok(());
        }

        // TODO(emilk): some way to visualize the resolution rectangle of the pinhole camera (in case there is no image logged).

        // Note that we can't rely on the camera being part of scene.space_cameras since that requires
        // the camera to be added to the scene!
        //
        // TODO(jleibs): Would be nice to use `query_pinhole` here, but we don't have a data-result or the other pieces
        // necessary to properly handle overrides, defaults, or fallbacks. We don't actually use the image_plane_distance
        // so it doesnt technically matter.
        state.pinhole_at_origin =
            query_pinhole_legacy(ctx, &ctx.current_query(), query.space_origin);

        let (mut response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

        // Convert ui coordinates to/from scene coordinates.
        let ui_from_scene = ui_from_scene(ctx, query.space_view_id, &response, self, state);
        let scene_from_ui = ui_from_scene.inverse();

        // TODO(andreas): Use the same eye & transformations as in `setup_target_config`.
        let eye = Eye {
            world_from_rub_view: IsoTransform::IDENTITY,
            fov_y: None,
        };

        let scene_bounds = *scene_from_ui.to();
        let Ok(target_config) = setup_target_config(
            &painter,
            scene_bounds,
            &query.space_origin.to_string(),
            query.highlights.any_outlines(),
            &state.pinhole_at_origin,
        ) else {
            return Ok(());
        };

        // Create labels now since their shapes participate are added to scene.ui for picking.
        let (label_shapes, ui_rects) = create_labels(
            collect_ui_labels(&system_output.view_systems),
            ui_from_scene,
            &eye,
            ui,
            &query.highlights,
            SpatialSpaceViewKind::TwoD,
        );

        let Some(render_ctx) = ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let mut view_builder = ViewBuilder::new(render_ctx, target_config);

        if let Some(pointer_pos_ui) = response.hover_pos() {
            let picking_context = crate::picking::PickingContext::new(
                pointer_pos_ui,
                scene_from_ui,
                ui.ctx().pixels_per_point(),
                &eye,
            );
            response = crate::picking_ui::picking(
                ctx,
                &picking_context,
                ui,
                response,
                &mut view_builder,
                state,
                &system_output,
                &ui_rects,
                query,
                SpatialSpaceViewKind::TwoD,
            )?;
        } else {
            state.previous_picking_result = None;
        }

        for draw_data in system_output.draw_data {
            view_builder.queue_draw(draw_data);
        }

        let background = ViewProperty::from_archetype::<Background>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.space_view_id,
        );
        let (background_drawable, clear_color) =
            crate::configure_background(ctx, &background, render_ctx, self, state)?;
        if let Some(background_drawable) = background_drawable {
            view_builder.queue_draw(background_drawable);
        }

        // ------------------------------------------------------------------------

        if let Some(mode) = screenshot_context_menu(ctx, &response) {
            view_builder
                .schedule_screenshot(render_ctx, query.space_view_id.gpu_readback_id(), mode)
                .ok();
        }

        // Draw a re_renderer driven view.
        // Camera & projection are configured to ingest space coordinates directly.
        painter.add(gpu_bridge::new_renderer_callback(
            view_builder,
            painter.clip_rect(),
            clear_color,
        ));

        // Make sure to _first_ draw the selected, and *then* the hovered context on top!
        for selected_context in ctx.selection_state().selection_space_contexts() {
            painter.extend(show_projections_from_3d_space(
                ui,
                query.space_origin,
                &ui_from_scene,
                selected_context,
                ui.ctx().selection_stroke().color,
            ));
        }
        if let Some(hovered_context) = ctx.selection_state().hovered_space_context() {
            painter.extend(show_projections_from_3d_space(
                ui,
                query.space_origin,
                &ui_from_scene,
                hovered_context,
                ui.ctx().hover_stroke().color,
            ));
        }

        // Add egui-rendered spinners/loaders on top of re_renderer content:
        crate::ui::paint_loading_spinners(ui, ui_from_scene, &eye, &system_output.view_systems);

        // Add egui-rendered labels on top of everything else:
        painter.extend(label_shapes);

        Ok(())
    }
}

fn setup_target_config(
    egui_painter: &egui::Painter,
    scene_bounds: Rect,
    space_name: &str,
    any_outlines: bool,
    scene_pinhole: &Option<Pinhole>,
) -> anyhow::Result<TargetConfiguration> {
    // ⚠️ When changing this code, make sure to run `tests/rust/test_pinhole_projection`.

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

    // TODO(andreas): Support anamorphic pinhole cameras properly.

    // For simplicity (and to reduce surprises!) we always render with a pinhole camera.
    // Make up a default pinhole camera if we don't have one placing it in a way to look at the entire space.
    let scene_bounds_size = glam::vec2(scene_bounds.width(), scene_bounds.height());

    let pinhole;
    let resolution;

    if let Some(scene_pinhole) = scene_pinhole {
        // The user has a pinhole, and we may want to project 3D stuff into this 2D space,
        // and we want to use that pinhole projection to do so.
        pinhole = scene_pinhole.clone();

        resolution = pinhole.resolution().unwrap_or_else(|| {
            // This is weird - we have a projection with an unknown resolution.
            // Let's just pick something plausible and hope for the best 😬.
            re_log::warn_once!("Pinhole projection lacks resolution.");
            glam::Vec2::splat(1000.0)
        });
    } else {
        // The user didn't pick a pinhole, but we still set up a 3D projection.
        // So we just pick _any_ pinhole camera, but we pick a "plausible" one so that
        // it is similar to real-life pinhole cameras, so that we get similar scales and precision.
        let focal_length = 1000.0; // Whatever, but small values can cause precision issues, noticeable on rectangle corners.
        let principal_point = glam::Vec2::splat(500.0); // Whatever
        resolution = glam::Vec2::splat(1000.0); // Whatever
        pinhole = Pinhole {
            image_from_camera: glam::Mat3::from_cols(
                glam::vec3(focal_length, 0.0, 0.0),
                glam::vec3(0.0, focal_length, 0.0),
                principal_point.extend(1.0),
            )
            .into(),
            resolution: Some([resolution.x, resolution.y].into()),
            camera_xyz: Some(ViewCoordinates::RDF),
            image_plane_distance: None,
        };
    }
    let pinhole_rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(resolution.x, resolution.y));

    let projection_from_view = re_renderer::view_builder::Projection::Perspective {
        vertical_fov: pinhole.fov_y().unwrap_or(Eye::DEFAULT_FOV_Y),
        near_plane_distance: 0.1,
        aspect_ratio: pinhole
            .aspect_ratio()
            .unwrap_or(scene_bounds_size.x / scene_bounds_size.y), // only happens if the pinhole lacks resolution
    };

    let focal_length = pinhole.focal_length_in_pixels();
    let focal_length = 2.0 / (1.0 / focal_length.x() + 1.0 / focal_length.y()); // harmonic mean (lack of anamorphic support)

    // Position the camera looking straight at the principal point:
    let view_from_world = re_math::IsoTransform::look_at_rh(
        pinhole.principal_point().extend(-focal_length),
        pinhole.principal_point().extend(0.0),
        -glam::Vec3::Y,
    )
    .ok_or_else(|| anyhow::format_err!("Failed to compute camera transform for 2D view."))?;

    // "pan-and-scan" to look at a particular part (scene_bounds) of the scene (pinhole_rect).
    let mut viewport_transformation = re_renderer::RectTransform {
        region: re_render_rect_from_egui_rect(pinhole_rect),
        region_of_interest: re_render_rect_from_egui_rect(scene_bounds),
    };

    // We want to look at the center of the scene bounds,
    // but we set up the camera to look at the principal point,
    // so we need to translate the view camera to compensate for that:
    let image_center = 0.5 * resolution;
    viewport_transformation.region_of_interest.min += image_center - pinhole.principal_point();

    // ----------------------

    let pixels_per_point = egui_painter.ctx().pixels_per_point();
    let resolution_in_pixel =
        gpu_bridge::viewport_resolution_in_pixels(egui_painter.clip_rect(), pixels_per_point);
    anyhow::ensure!(0 < resolution_in_pixel[0] && 0 < resolution_in_pixel[1]);

    Ok({
        let name = space_name.into();
        TargetConfiguration {
            name,
            resolution_in_pixel,
            view_from_world,
            projection_from_view,
            viewport_transformation,
            pixels_per_point,
            outline_config: any_outlines.then(|| re_space_view::outline_config(egui_painter.ctx())),
            blend_with_background: false,
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
    circle_fill_color: egui::Color32,
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
                    shapes.push(Shape::circle_filled(pos_in_ui, radius, circle_fill_color));

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
