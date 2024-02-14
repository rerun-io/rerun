use egui::{
    emath::RectTransform, pos2, vec2, Align2, Color32, NumExt as _, Pos2, Rect, ScrollArea, Shape,
    Vec2,
};
use macaw::IsoTransform;

use re_entity_db::EntityPath;
use re_renderer::view_builder::{TargetConfiguration, ViewBuilder};
use re_space_view::controls::{DRAG_PAN2D_BUTTON, RESET_VIEW_BUTTON_TEXT, ZOOM_SCROLL_MODIFIER};
use re_types::{archetypes::Pinhole, components::ViewCoordinates};
use re_viewer_context::{
    gpu_bridge, SelectedSpaceContext, SpaceViewSystemExecutionError, SystemExecutionOutput,
    ViewQuery, ViewerContext,
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

#[derive(Clone, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct View2DState {
    /// The zoom and pan state, which is either a zoom/center or `Auto` which will fill the screen
    zoom: ZoomState2D,
}

#[derive(Clone, Copy, Default, PartialEq, serde::Deserialize, serde::Serialize)]
/// Sub-state specific to the Zoom/Scale/Pan engine
pub enum ZoomState2D {
    #[default]
    Auto,

    Scaled {
        /// Number of ui points per scene unit
        scale: f32,

        /// Which scene coordinate will be at the center of the zoomed region.
        center: Pos2,

        /// Whether to allow the state to be updated by the current `ScrollArea` offsets
        accepting_scroll: bool,
    },
}

impl View2DState {
    /// Determine the optimal sub-region and size based on the `ZoomState` and
    /// available size. This will generally be used to construct the painter and
    /// subsequent transforms
    ///
    /// Returns `(desired_size, scroll_offset)` where:
    ///   - `desired_size` is the size of the painter necessary to capture the zoomed view in ui points
    ///   - `scroll_offset` is the position of the `ScrollArea` offset in ui points
    fn desired_size_and_offset(&self, available_size: Vec2, canvas_rect: Rect) -> (Vec2, Vec2) {
        match self.zoom {
            ZoomState2D::Scaled { scale, center, .. } => {
                let desired_size = canvas_rect.size() * scale;

                // Try to keep the center of the scene in the middle of the available size
                let scroll_offset = (center.to_vec2() - canvas_rect.left_top().to_vec2()) * scale
                    - available_size / 2.0;

                (desired_size, scroll_offset)
            }
            ZoomState2D::Auto => {
                // Otherwise, we autoscale the space to fit available area while maintaining aspect ratio
                let scene_bbox = if canvas_rect.is_positive() {
                    canvas_rect
                } else {
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
                };
                let mut desired_size = scene_bbox.size();
                desired_size *= available_size.x / desired_size.x; // fill full width
                desired_size *= (available_size.y / desired_size.y).at_most(1.0); // shrink so we don't fill more than full height

                if desired_size.is_finite() {
                    (desired_size, Vec2::ZERO)
                } else {
                    (available_size, Vec2::ZERO)
                }
            }
        }
    }

    /// Update our zoom state based on response
    /// If nothing else happens this will reset `accepting_scroll` to true when appropriate
    fn update(
        &mut self,
        response: &egui::Response,
        ui_to_space: egui::emath::RectTransform,
        canvas_rect: Rect,
        available_size: Vec2,
    ) {
        // Determine if we are zooming
        let zoom_delta = response.ctx.input(|i| i.zoom_delta());
        let hovered_zoom = if response.hovered() && zoom_delta != 1.0 {
            Some(zoom_delta)
        } else {
            None
        };

        match self.zoom {
            ZoomState2D::Auto => {
                if let Some(input_zoom) = hovered_zoom {
                    if input_zoom > 1.0 {
                        let scale = response.rect.height() / ui_to_space.to().height();
                        let center = canvas_rect.center();
                        self.zoom = ZoomState2D::Scaled {
                            scale,
                            center,
                            accepting_scroll: false,
                        };
                        // Recursively update now that we have initialized `ZoomState` to `Scaled`
                        self.update(response, ui_to_space, canvas_rect, available_size);
                    }
                }
            }
            ZoomState2D::Scaled {
                mut scale,
                mut center,
                ..
            } => {
                let mut accepting_scroll = true;

                // If we are zooming, adjust the scale and center
                if let Some(input_zoom) = hovered_zoom {
                    let new_scale = scale * input_zoom;

                    // Adjust for mouse location while executing zoom
                    if let Some(hover_pos) = response.ctx.input(|i| i.pointer.hover_pos()) {
                        let zoom_loc = ui_to_space.transform_pos(hover_pos);

                        // Space-units under the cursor will shift based on distance from center
                        let dist_from_center = zoom_loc - center;
                        // In UI points this happens based on the difference in scale;
                        let shift_in_ui = dist_from_center * (new_scale - scale);
                        // But we will compensate for it by a shift in space units
                        let shift_in_space = shift_in_ui / new_scale;

                        // Moving the center in the direction of the desired shift
                        center += shift_in_space;
                    }
                    // Don't show less than one horizontal scene unit in the entire screen.
                    scale = new_scale.at_most(available_size.x);
                    accepting_scroll = false;
                }

                // If we are dragging, adjust the center accordingly
                if response.dragged_by(DRAG_PAN2D_BUTTON) {
                    // Adjust center based on drag
                    center -= response.drag_delta() / scale;
                    accepting_scroll = false;
                }

                // Save the zoom state
                self.zoom = ZoomState2D::Scaled {
                    scale,
                    center,
                    accepting_scroll,
                };
            }
        }

        // Process things that might reset ZoomState to Auto
        if let ZoomState2D::Scaled { scale, .. } = self.zoom {
            // If the user double-clicks
            if response.double_clicked() {
                self.zoom = ZoomState2D::Auto;
            }

            // If our zoomed region is smaller than the available size
            if canvas_rect.size().x * scale < available_size.x
                && canvas_rect.size().y * scale < available_size.y
            {
                self.zoom = ZoomState2D::Auto;
            }
        }
    }

    /// Take the offset from the `ScrollArea` and apply it back to center so that other
    /// scroll interfaces work as expected.
    fn capture_scroll(&mut self, offset: Vec2, available_size: Vec2, canvas_rect: Rect) {
        if let ZoomState2D::Scaled {
            scale,
            accepting_scroll,
            ..
        } = self.zoom
        {
            if accepting_scroll {
                let center = canvas_rect.left_top() + (available_size / 2.0 + offset) / scale;
                self.zoom = ZoomState2D::Scaled {
                    scale,
                    center,
                    accepting_scroll,
                };
            }
        }
    }
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

    // Save off the available_size since this is used for some of the layout updates later
    let available_size = ui.available_size();
    let store = ctx.entity_db.store();

    let scene_rect_accum = state.bounding_boxes.accumulated;
    let scene_rect_accum = egui::Rect::from_min_max(
        scene_rect_accum.min.truncate().to_array().into(),
        scene_rect_accum.max.truncate().to_array().into(),
    );

    // Determine the canvas which determines the extent of the explorable scene coordinates,
    // and thus the size of the scroll area.
    //
    // TODO(andreas): We want to move away from the scroll area and instead work with open ended 2D scene coordinates!
    // The term canvas might then refer to the area in scene coordinates visible at a given moment.
    // Orthogonally, we'll want to visualize the resolution rectangle of the pinhole camera.
    //
    // For that we need to check if this is defined by a pinhole camera.
    // Note that we can't rely on the camera being part of scene.space_cameras since that requires
    // the camera to be added to the scene!
    let pinhole = query_pinhole(store, &ctx.current_query(), query.space_origin);
    let canvas_rect = pinhole
        .as_ref()
        .and_then(|p| p.resolution())
        .map_or(scene_rect_accum, |res| {
            Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(res.x, res.y))
        });

    let (desired_size, offset) = state
        .state_2d
        .desired_size_and_offset(available_size, canvas_rect);

    // Bound the offset based on sizes
    // TODO(jleibs): can we derive this from the ScrollArea shape?
    let offset = offset.at_most(desired_size - available_size);
    let offset = offset.at_least(Vec2::ZERO);

    let scroll_area = ScrollArea::both()
        .scroll_offset(offset)
        .auto_shrink([false, false]);

    let scroll_out = scroll_area.show(ui, |ui| -> Result<(), SpaceViewSystemExecutionError> {
        let desired_size = desired_size.at_least(Vec2::ZERO);
        let (mut response, painter) =
            ui.allocate_painter(desired_size, egui::Sense::click_and_drag());

        if !response.rect.is_positive() {
            return Ok(());
        }

        let ui_from_canvas = egui::emath::RectTransform::from_to(canvas_rect, response.rect);
        let canvas_from_ui = ui_from_canvas.inverse();

        state
            .state_2d
            .update(&response, canvas_from_ui, canvas_rect, available_size);

        // TODO(andreas): Use the same eye & transformations as in `setup_target_config`.
        let eye = Eye {
            world_from_rub_view: IsoTransform::IDENTITY,
            fov_y: None,
        };

        let Ok(target_config) = setup_target_config(
            &painter,
            canvas_from_ui,
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
            ui_from_canvas,
            &eye,
            ui,
            &query.highlights,
            SpatialSpaceViewKind::TwoD,
        );

        if !re_ui::egui_helpers::is_anything_being_dragged(ui.ctx()) {
            response = picking(
                ctx,
                response,
                canvas_from_ui,
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

        // Screenshot context menu.
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
        for selected_context in ctx.selection_state().selected_space_context() {
            painter.extend(show_projections_from_3d_space(
                ui,
                query.space_origin,
                &ui_from_canvas,
                selected_context,
                ui.style().visuals.selection.bg_fill,
            ));
        }
        if let Some(hovered_context) = ctx.selection_state().hovered_space_context() {
            painter.extend(show_projections_from_3d_space(
                ui,
                query.space_origin,
                &ui_from_canvas,
                hovered_context,
                egui::Color32::WHITE,
            ));
        }

        // Add egui driven labels on top of re_renderer content.
        painter.extend(label_shapes);

        Ok(())
    });
    scroll_out.inner?;

    // Update the scroll area based on the computed offset
    // This handles cases of dragging/zooming the space
    state
        .state_2d
        .capture_scroll(scroll_out.state.offset, available_size, scene_rect_accum);
    Ok(())
}

fn setup_target_config(
    egui_painter: &egui::Painter,
    canvas_from_ui: RectTransform,
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
    let canvas_size = glam::vec2(canvas_from_ui.to().width(), canvas_from_ui.to().height());
    let default_principal_point = canvas_from_ui.to().center();
    let default_principal_point = glam::vec2(default_principal_point.x, default_principal_point.y);
    let pinhole = pinhole.unwrap_or_else(|| {
        let focal_length_in_pixels = canvas_size.x;

        Pinhole {
            image_from_camera: glam::Mat3::from_cols(
                glam::vec3(focal_length_in_pixels, 0.0, 0.0),
                glam::vec3(0.0, focal_length_in_pixels, 0.0),
                default_principal_point.extend(1.0),
            )
            .into(),
            resolution: Some(canvas_size.into()),
            camera_xyz: Some(ViewCoordinates::RDF),
        }
    });

    let projection_from_view = re_renderer::view_builder::Projection::Perspective {
        vertical_fov: pinhole.fov_y().unwrap_or(Eye::DEFAULT_FOV_Y),
        near_plane_distance: 0.1,
        aspect_ratio: pinhole
            .aspect_ratio()
            .unwrap_or(canvas_size.x / canvas_size.y),
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
        region: re_render_rect_from_egui_rect(*canvas_from_ui.from()),
    };

    // The principal point might not be quite centered.
    // We need to account for this translation in the viewport transformation.
    let principal_point_offset = default_principal_point - pinhole.principal_point();
    let ui_from_canvas_scale = canvas_from_ui.inverse().scale();
    viewport_transformation.region_of_interest.min +=
        principal_point_offset * glam::vec2(ui_from_canvas_scale.x, ui_from_canvas_scale.y);

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
    ui_from_canvas: &RectTransform,
    space_context: &SelectedSpaceContext,
    color: egui::Color32,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    if let SelectedSpaceContext::ThreeD {
        point_in_space_cameras: target_spaces,
        ..
    } = space_context
    {
        for (space_2d, pos_2d) in target_spaces {
            if space_2d == space {
                if let Some(pos_2d) = pos_2d {
                    // User is hovering a 2D point inside a 3D view.
                    let pos_in_ui = ui_from_canvas.transform_pos(pos2(pos_2d.x, pos_2d.y));
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
