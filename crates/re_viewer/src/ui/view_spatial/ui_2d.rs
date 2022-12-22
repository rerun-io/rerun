use eframe::{emath::RectTransform, epaint::text::TextWrapping};
use egui::{
    pos2, vec2, Align, Align2, Color32, NumExt as _, Pos2, Rect, Response, ScrollArea, Shape,
    TextFormat, TextStyle, Vec2,
};
use macaw::IsoTransform;
use re_data_store::{InstanceId, InstanceIdHash, ObjPath};
use re_renderer::view_builder::TargetConfiguration;

use crate::{
    misc::HoveredSpace,
    ui::{
        image_ui,
        view_spatial::{
            ui_renderer_bridge::{create_scene_paint_callback, get_viewport, ScreenBackground},
            Image, Label2DTarget, SceneSpatial,
        },
    },
    Selection, ViewerContext,
};

use super::eye::Eye;

// ---

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct View2DState {
    /// The zoom and pan state, which is either a zoom/center or `Auto` which will fill the screen
    #[serde(skip)]
    zoom: ZoomState2D,
}

#[derive(Clone, Copy)]
/// Sub-state specific to the Zoom/Scale/Pan engine
pub enum ZoomState2D {
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

impl Default for ZoomState2D {
    fn default() -> Self {
        ZoomState2D::Auto
    }
}
impl View2DState {
    /// Determine the optimal sub-region and size based on the `ZoomState` and
    /// available size. This will generally be used to construct the painter and
    /// subsequent transforms
    ///
    /// Returns `(desired_size, scroll_offset)` where:
    ///   - `desired_size` is the size of the painter necessary to capture the zoomed view in ui points
    ///   - `scroll_offset` is the position of the `ScrollArea` offset in ui points
    fn desired_size_and_offset(
        &self,
        available_size: Vec2,
        scene_rect_accum: Rect,
    ) -> (Vec2, Vec2) {
        match self.zoom {
            ZoomState2D::Scaled { scale, center, .. } => {
                let desired_size = scene_rect_accum.size() * scale;

                // Try to keep the center of the scene in the middle of the available size
                let scroll_offset = (center.to_vec2() - scene_rect_accum.left_top().to_vec2())
                    * scale
                    - available_size / 2.0;

                (desired_size, scroll_offset)
            }
            ZoomState2D::Auto => {
                // Otherwise, we autoscale the space to fit available area while maintaining aspect ratio
                let scene_bbox = if scene_rect_accum.is_positive() {
                    scene_rect_accum
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
        scene_rect_accum: Rect,
        available_size: Vec2,
    ) {
        // Determine if we are zooming
        let zoom_delta = response.ctx.input().zoom_delta();
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
                        let center = scene_rect_accum.center();
                        self.zoom = ZoomState2D::Scaled {
                            scale,
                            center,
                            accepting_scroll: false,
                        };
                        // Recursively update now that we have initialized `ZoomState` to `Scaled`
                        self.update(response, ui_to_space, scene_rect_accum, available_size);
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
                    if let Some(hover_pos) = response.ctx.input().pointer.hover_pos() {
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
                    scale = new_scale;
                    accepting_scroll = false;
                }

                // If we are dragging, adjust the center accordingly
                if response.dragged_by(egui::PointerButton::Primary) {
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
            if scene_rect_accum.size().x * scale < available_size.x
                && scene_rect_accum.size().y * scale < available_size.y
            {
                self.zoom = ZoomState2D::Auto;
            }
        }
    }

    /// Take the offset from the `ScrollArea` and apply it back to center so that other
    /// scroll interfaces work as expected.
    fn capture_scroll(&mut self, offset: Vec2, available_size: Vec2, scene_rect_accum: Rect) {
        if let ZoomState2D::Scaled {
            scale,
            accepting_scroll,
            ..
        } = self.zoom
        {
            if accepting_scroll {
                let center = scene_rect_accum.left_top() + (available_size / 2.0 + offset) / scale;
                self.zoom = ZoomState2D::Scaled {
                    scale,
                    center,
                    accepting_scroll,
                };
            }
        }
    }
}

pub const HELP_TEXT: &str = "Ctrl-scroll  to zoom (⌘-scroll or Mac).\n\
    Drag to pan.\n\
    Double-click to reset the view.";

/// Create the outer 2D view, which consists of a scrollable region
/// TODO(andreas): Split into smaller parts, more re-use with `ui_3d`
pub fn view_2d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut View2DState,
    space: &ObjPath,
    scene: SceneSpatial,
    scene_rect_accum: Rect,
    hovered_instance: &mut Option<InstanceId>,
) -> egui::Response {
    crate::profile_function!();

    // Save off the available_size since this is used for some of the layout updates later
    let available_size = ui.available_size();

    let (desired_size, offset) = state.desired_size_and_offset(available_size, scene_rect_accum);

    // Bound the offset based on sizes
    // TODO(jleibs): can we derive this from the ScrollArea shape?
    let offset = offset.at_most(desired_size - available_size);
    let offset = offset.at_least(Vec2::ZERO);

    let scroll_area = ScrollArea::both()
        .scroll_offset(offset)
        .auto_shrink([false, false]);

    let scroll_out = scroll_area.show(ui, |ui| {
        view_2d_scrollable(
            desired_size,
            available_size,
            ctx,
            ui,
            state,
            space,
            scene,
            scene_rect_accum,
            hovered_instance,
        )
    });

    // Update the scroll area based on the computed offset
    // This handles cases of dragging/zooming the space
    state.capture_scroll(scroll_out.state.offset, available_size, scene_rect_accum);
    scroll_out.inner
}

/// Create the real 2D view inside the scrollable area
#[allow(clippy::too_many_arguments)]
fn view_2d_scrollable(
    desired_size: Vec2,
    available_size: Vec2,
    ctx: &mut ViewerContext<'_>,
    parent_ui: &mut egui::Ui,
    state: &mut View2DState,
    space: &ObjPath,
    mut scene: SceneSpatial,
    scene_rect_accum: Rect,
    hovered_instance: &mut Option<InstanceId>,
) -> egui::Response {
    let (mut response, painter) =
        parent_ui.allocate_painter(desired_size, egui::Sense::click_and_drag());

    // Create our transforms.
    let ui_from_space = egui::emath::RectTransform::from_to(scene_rect_accum, response.rect);
    let space_from_ui = ui_from_space.inverse();
    let space_from_points = space_from_ui.scale().y;
    let points_from_pixels = 1.0 / painter.ctx().pixels_per_point();
    let space_from_pixel = space_from_points * points_from_pixels;

    state.update(&response, space_from_ui, scene_rect_accum, available_size);

    // ------------------------------------------------------------------------

    // Add egui driven labels on top of re_renderer content.
    // Needs to come before hovering checks because it adds more objects for hovering.
    {
        let hovered_instance_hash = hovered_instance
            .as_ref()
            .map_or(InstanceIdHash::NONE, |i| i.hash());
        painter.extend(create_labels(
            &mut scene,
            ui_from_space,
            space_from_ui,
            parent_ui,
            hovered_instance_hash,
        ));
    }

    // ------------------------------------------------------------------------

    // What tooltips we've shown so far
    let mut shown_tooltips = ahash::HashSet::default();
    let mut depths_at_pointer = vec![];
    let mut closest_instance_id_hash = InstanceIdHash::NONE;

    // Check if we're hovering any hover primitive.
    if let Some(pointer_pos_ui) = response.hover_pos() {
        // All hover primitives have their coordinates in space units.
        // Transform the pointer pos so we don't have to transform anything else!
        let pointer_pos_space = space_from_ui.transform_pos(pointer_pos_ui);
        let pointer_pos_space_glam = glam::vec2(pointer_pos_space.x, pointer_pos_space.y);

        let hover_radius = space_from_ui.scale().y * 5.0; // TODO(emilk): from egui?
        let mut closest_dist = hover_radius;

        if let Some((instance, position)) = scene
            .primitives
            .picking(
                pointer_pos_space_glam,
                &scene_rect_accum,
                &Eye {
                    world_from_view: IsoTransform::IDENTITY,
                    fov_y: None,
                },
            )
            .first()
        {
            closest_instance_id_hash = *instance;
            closest_dist = pointer_pos_space_glam
                .extend(position.z)
                .distance(*position);
        }

        let mut check_hovering = |instance_hash, dist: f32| {
            if dist <= closest_dist {
                closest_dist = dist;
                closest_instance_id_hash = instance_hash;
            }
        };

        for (bbox, instance_hash) in &scene.ui.rects {
            check_hovering(*instance_hash, bbox.distance_to_pos(pointer_pos_space));
        }

        for img in &scene.ui.images {
            let Image {
                instance_hash,
                tensor,
                meter,
                annotations,
            } = img;

            if instance_hash.is_none() {
                continue;
            }

            let (w, h) = (tensor.shape[1].size as f32, tensor.shape[0].size as f32);
            let rect = Rect::from_min_size(Pos2::ZERO, vec2(w, h));
            let dist = rect.distance_sq_to_pos(pointer_pos_space).sqrt();
            let dist = dist.at_least(hover_radius); // allow stuff on top of us to "win"
            check_hovering(*instance_hash, dist);

            // Show tooltips for all images, not just the "most hovered" one.
            if rect.contains(pointer_pos_space) {
                response = response
                    .on_hover_cursor(egui::CursorIcon::ZoomIn)
                    .on_hover_ui_at_pointer(|ui| {
                        ui.set_max_width(400.0);

                        ui.vertical(|ui| {
                            if let Some(instance_id) = instance_hash.resolve(&ctx.log_db.obj_db) {
                                ui.label(instance_id.to_string());
                                crate::ui::data_ui::instance_ui(
                                    ctx,
                                    ui,
                                    &instance_id,
                                    crate::ui::Preview::Small,
                                );
                                ui.separator();
                            }

                            let legend = Some(annotations.clone());
                            let tensor_view = ctx.cache.image.get_view_with_annotations(
                                tensor,
                                &legend,
                                ctx.render_ctx,
                            );

                            ui.horizontal(|ui| {
                                image_ui::show_zoomed_image_region(
                                    parent_ui,
                                    ui,
                                    &tensor_view,
                                    ui_from_space.transform_rect(rect),
                                    pointer_pos_ui,
                                    *meter,
                                );
                            });
                        });
                    });

                shown_tooltips.insert(*instance_hash);
            }

            if let Some(meter) = *meter {
                if let Some(raw_value) = tensor.get(&[
                    pointer_pos_space.y.round() as _,
                    pointer_pos_space.x.round() as _,
                ]) {
                    let raw_value = raw_value.as_f64();
                    let depth_in_meters = raw_value / meter as f64;
                    depths_at_pointer.push(depth_in_meters);
                }
            }
        }
    }

    // ------------------------------------------------------------------------

    // Draw a re_renderer driven view.
    // Camera & projection are configured to ingest space coordinates directly.
    {
        crate::profile_scope!("build command buffer for 2D view {}", space.to_string());

        let Ok(target_config) = setup_target_config(
            &painter,
            space_from_ui,
            space_from_pixel,
            &space.to_string(),
        ) else {
            return response;
        };

        let Ok(callback) = create_scene_paint_callback(
            ctx.render_ctx,
            target_config, painter.clip_rect(),
            &scene.primitives,
            &ScreenBackground::ClearColor(parent_ui.visuals().extreme_bg_color.into()),
        ) else {
            return response;
        };

        painter.add(callback);
    }

    // ------------------------------------------------------------------------

    if let Some(instance_id) = hovered_instance {
        if response.clicked() {
            ctx.set_selection(Selection::Instance(instance_id.clone()));
        }
        if !shown_tooltips.contains(&instance_id.hash()) {
            response = response.on_hover_ui_at_pointer(|ui| {
                ctx.instance_id_button(ui, instance_id);
                crate::ui::data_ui::instance_ui(ctx, ui, instance_id, crate::ui::Preview::Small);
            });
        }
    }

    // ------------------------------------------------------------------------

    let depth_at_pointer = if depths_at_pointer.len() == 1 {
        depths_at_pointer[0] as f32
    } else {
        f32::INFINITY
    };
    project_onto_other_spaces(ctx, space, &response, &space_from_ui, depth_at_pointer);
    painter.extend(show_projections_from_3d_space(
        ctx,
        parent_ui,
        space,
        &ui_from_space,
    ));

    // ------------------------------------------------------------------------

    *hovered_instance = closest_instance_id_hash.resolve(&ctx.log_db.obj_db);

    response
}

fn create_labels(
    scene: &mut SceneSpatial,
    ui_from_space: RectTransform,
    space_from_ui: RectTransform,
    parent_ui: &mut egui::Ui,
    hovered_instance: InstanceIdHash,
) -> Vec<Shape> {
    let mut label_shapes = Vec::with_capacity(scene.ui.labels_2d.len() * 2);

    for label in &scene.ui.labels_2d {
        let (wrap_width, text_anchor_pos) = match label.target {
            Label2DTarget::Rect(rect) => {
                let rect_in_ui = ui_from_space.transform_rect(rect);
                (
                    // Place the text centered below the rect
                    (rect_in_ui.width() - 4.0).at_least(60.0),
                    rect_in_ui.center_bottom() + vec2(0.0, 3.0),
                )
            }
            Label2DTarget::Point(pos) => {
                let pos_in_ui = ui_from_space.transform_pos(pos);
                (f32::INFINITY, pos_in_ui + vec2(0.0, 3.0))
            }
        };

        let font_id = TextStyle::Body.resolve(parent_ui.style());
        let galley = parent_ui.fonts().layout_job({
            egui::text::LayoutJob {
                sections: vec![egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: 0..label.text.len(),
                    format: TextFormat::simple(font_id, label.color),
                }],
                text: label.text.clone(),
                wrap: TextWrapping {
                    max_width: wrap_width,
                    ..Default::default()
                },
                break_on_newline: true,
                halign: Align::Center,
                ..Default::default()
            }
        });

        let text_rect =
            Align2::CENTER_TOP.anchor_rect(Rect::from_min_size(text_anchor_pos, galley.size()));
        let bg_rect = text_rect.expand2(vec2(4.0, 2.0));

        let fill_color = if label.labled_instance == hovered_instance {
            parent_ui.style().visuals.widgets.active.bg_fill
        } else {
            parent_ui.style().visuals.widgets.inactive.bg_fill
        };

        label_shapes.push(Shape::rect_filled(bg_rect, 3.0, fill_color));
        label_shapes.push(Shape::galley(text_rect.center_top(), galley));

        scene
            .ui
            .rects
            .push((space_from_ui.transform_rect(bg_rect), label.labled_instance));
    }

    label_shapes
}

fn setup_target_config(
    painter: &egui::Painter,
    space_from_ui: RectTransform,
    space_from_pixel: f32,
    space_name: &str,
) -> anyhow::Result<TargetConfiguration> {
    let pixels_from_points = painter.ctx().pixels_per_point();
    let resolution_in_pixel = get_viewport(painter.clip_rect(), pixels_from_points);
    anyhow::ensure!(resolution_in_pixel[0] > 0 && resolution_in_pixel[1] > 0);

    let camera_position_space = space_from_ui.transform_pos(painter.clip_rect().min);

    Ok(TargetConfiguration::new_2d_target(
        space_name.into(),
        resolution_in_pixel,
        space_from_pixel,
        pixels_from_points,
        glam::vec2(camera_position_space.x, camera_position_space.y),
    ))
}

// ------------------------------------------------------------------------

fn project_onto_other_spaces(
    ctx: &mut ViewerContext<'_>,
    space: &ObjPath,
    response: &Response,
    space_from_ui: &RectTransform,
    z: f32,
) {
    if let Some(pointer_in_screen) = response.hover_pos() {
        let pointer_in_space = space_from_ui.transform_pos(pointer_in_screen);
        ctx.rec_cfg.hovered_space_this_frame = HoveredSpace::TwoD {
            space_2d: space.clone(),
            pos: glam::vec3(pointer_in_space.x, pointer_in_space.y, z),
        };
    }
}

fn show_projections_from_3d_space(
    ctx: &ViewerContext<'_>,
    ui: &egui::Ui,
    space: &ObjPath,
    ui_from_space: &RectTransform,
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    if let HoveredSpace::ThreeD { target_spaces, .. } = &ctx.rec_cfg.hovered_space_previous_frame {
        for (space_2d, ray_2d, pos_2d) in target_spaces {
            if space_2d == space {
                if let Some(pos_2d) = pos_2d {
                    // User is hovering a 2D point inside a 3D view.
                    let pos_in_ui = ui_from_space.transform_pos(pos2(pos_2d.x, pos_2d.y));
                    let radius = 4.0;
                    shapes.push(Shape::circle_filled(
                        pos_in_ui,
                        radius + 2.0,
                        Color32::BLACK,
                    ));
                    shapes.push(Shape::circle_filled(pos_in_ui, radius, Color32::WHITE));

                    let text = format!("Depth: {:.3} m", pos_2d.z);
                    let font_id = egui::TextStyle::Body.resolve(ui.style());
                    let galley = ui.fonts().layout_no_wrap(text, font_id, Color32::WHITE);
                    let rect = Align2::CENTER_TOP.anchor_rect(Rect::from_min_size(
                        pos_in_ui + vec2(0.0, 5.0),
                        galley.size(),
                    ));
                    shapes.push(Shape::rect_filled(
                        rect,
                        2.0,
                        Color32::from_black_alpha(196),
                    ));
                    shapes.push(Shape::galley(rect.min, galley));
                }

                let show_ray = false; // This visualization is mostly confusing
                if show_ray {
                    if let Some(ray_2d) = ray_2d {
                        // User is hovering a 3D view with a camera in it.
                        // TODO(emilk): figure out a nice visualization here, or delete the code.
                        let origin = ray_2d.origin;
                        let end = ray_2d.point_along(10_000.0);

                        let origin = pos2(origin.x / origin.z, origin.y / origin.z);
                        let end = pos2(end.x / end.z, end.y / end.z);

                        let origin = ui_from_space.transform_pos(origin);
                        let end = ui_from_space.transform_pos(end);

                        shapes.push(Shape::circle_filled(origin, 5.0, Color32::WHITE));
                        shapes.push(Shape::line_segment([origin, end], (3.0, Color32::BLACK)));
                        shapes.push(Shape::line_segment([origin, end], (2.0, Color32::WHITE)));
                    }
                }
            }
        }
    }
    shapes
}
