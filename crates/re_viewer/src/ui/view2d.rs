use eframe::{emath::RectTransform, epaint::text::TextWrapping};
use egui::*;

use re_data_store::{InstanceId, InstanceIdHash};
use re_log_types::*;

use crate::{misc::HoveredSpace, Selection, ViewerContext};

use crate::legend::find_legend;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct State2D {
    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_instance: Option<InstanceId>,

    /// Estimated bounding box of all data. Accumulated.
    ///
    /// TODO(emilk): accumulate this per space once as data arrives instead.
    #[serde(skip)]
    scene_bbox_accum: epaint::Rect,

    /// The zoom and pan state, which is either a zoom/center or `Auto` which will fill the screen
    #[serde(skip)]
    zoom: ZoomState,
}

#[derive(Clone, Copy)]
/// Sub-state specific to the Zoom/Scale/Pan engine
enum ZoomState {
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

impl Default for ZoomState {
    fn default() -> Self {
        ZoomState::Auto
    }
}

impl Default for State2D {
    fn default() -> Self {
        Self {
            hovered_instance: Default::default(),
            scene_bbox_accum: epaint::Rect::NOTHING,
            zoom: Default::default(),
        }
    }
}

impl State2D {
    /// Determine the optimal sub-region and size based on the `ZoomState` and
    /// available size. This will generally be used to construct the painter and
    /// subsequent transforms
    ///
    /// Returns `(desired_size, scroll_offset)` where:
    ///   - `desired_size` is the size of the painter necessary to capture the zoomed view in ui points
    ///   - `scroll_offset` is the position of the `ScrollArea` offset in ui points
    fn desired_size_and_offset(&self, available_size: Vec2) -> (Vec2, Vec2) {
        match self.zoom {
            ZoomState::Scaled { scale, center, .. } => {
                let desired_size = self.scene_bbox_accum.size() * scale;

                // Try to keep the center of the scene in the middle of the available size
                let scroll_offset = (center.to_vec2() - self.scene_bbox_accum.left_top().to_vec2())
                    * scale
                    - available_size / 2.0;

                (desired_size, scroll_offset)
            }
            ZoomState::Auto => {
                // Otherwise, we autoscale the space to fit available area while maintaining aspect ratio
                let scene_bbox = if self.scene_bbox_accum.is_positive() {
                    self.scene_bbox_accum
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
            ZoomState::Auto => {
                if let Some(input_zoom) = hovered_zoom {
                    if input_zoom > 1.0 {
                        let scale = response.rect.height() / self.scene_bbox_accum.height();
                        let center = self.scene_bbox_accum.center();
                        self.zoom = ZoomState::Scaled {
                            scale,
                            center,
                            accepting_scroll: false,
                        };
                        // Recursively update now that we have initialized `ZoomState` to `Scaled`
                        self.update(response, ui_to_space, available_size);
                    }
                }
            }
            ZoomState::Scaled {
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
                self.zoom = ZoomState::Scaled {
                    scale,
                    center,
                    accepting_scroll,
                };
            }
        }

        // Process things that might reset ZoomState to Auto
        if let ZoomState::Scaled { scale, .. } = self.zoom {
            // If the user double-clicks
            if response.double_clicked() {
                self.zoom = ZoomState::Auto;
            }

            // If our zoomed region is smaller than the available size
            if self.scene_bbox_accum.size().x * scale < available_size.x
                && self.scene_bbox_accum.size().y * scale < available_size.y
            {
                self.zoom = ZoomState::Auto;
            }
        }
    }

    /// Take the offset from the `ScrollArea` and apply it back to center so that other
    /// scroll interfaces work as expected.
    fn capture_scroll(&mut self, offset: Vec2, available_size: Vec2) {
        if let ZoomState::Scaled {
            scale,
            accepting_scroll,
            ..
        } = self.zoom
        {
            if accepting_scroll {
                let center =
                    self.scene_bbox_accum.left_top() + (available_size / 2.0 + offset) / scale;
                self.zoom = ZoomState::Scaled {
                    scale,
                    center,
                    accepting_scroll,
                };
            }
        }
    }
}

/// Create the outer 2D view, which consists of a scrollable region
pub(crate) fn view_2d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut State2D,
    space: Option<&ObjPath>,
    objects: &re_data_store::Objects<'_>, // Note: only the objects that belong to this space
) -> egui::Response {
    crate::profile_function!();

    // TODO(emilk): make help button hover over the 2D view (https://github.com/emilk/egui/issues/980).
    crate::misc::help_hover_button(ui).on_hover_text(
        "Ctrl-scroll  to zoom (⌘-scroll or Mac).\n\
        Drag to pan.\n\
        Double-click to reset the view.",
    );

    // Save off the available_size since this is used for some of the layout updates later
    let available_size = ui.available_size();

    let (desired_size, offset) = state.desired_size_and_offset(available_size);

    // Bound the offset based on sizes
    // TODO(jleibs): can we derive this from the ScrollArea shape?
    let offset = offset.at_most(desired_size - available_size);
    let offset = offset.at_least(Vec2::ZERO);

    let scroll_area = ScrollArea::both()
        .scroll_offset(offset)
        .auto_shrink([false, false]);

    let scroll_out = scroll_area.show(ui, |ui| {
        view_2d_scrollable(desired_size, available_size, ctx, ui, state, space, objects)
    });

    // Update the scroll area based on the computed offset
    // This handles cases of dragging/zooming the space
    state.capture_scroll(scroll_out.state.offset, available_size);
    scroll_out.inner
}

/// Create the real 2D view inside the scrollable area
fn view_2d_scrollable(
    desired_size: Vec2,
    available_size: Vec2,
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut State2D,
    space: Option<&ObjPath>,
    objects: &re_data_store::Objects<'_>, // Note: only the objects that belong to this space
) -> egui::Response {
    state.scene_bbox_accum = state
        .scene_bbox_accum
        .union(crate::misc::calc_bbox_2d(objects));

    let scene_bbox = state.scene_bbox_accum;

    let (mut response, painter) = ui.allocate_painter(desired_size, egui::Sense::click_and_drag());

    // Create our transforms
    let ui_from_space = egui::emath::RectTransform::from_to(scene_bbox, response.rect);
    let space_from_ui = ui_from_space.inverse();

    // ------------------------------------------------------------------------
    state.update(&response, space_from_ui, available_size);

    // ------------------------------------------------------------------------
    if let Some(instance_id) = &state.hovered_instance {
        if response.clicked() {
            ctx.rec_cfg.selection = Selection::Instance(instance_id.clone());
        }
        egui::containers::popup::show_tooltip_at_pointer(ui.ctx(), Id::new("2d_tooltip"), |ui| {
            ctx.instance_id_button(ui, instance_id);
            crate::ui::data_ui::view_instance(ctx, ui, instance_id, crate::ui::Preview::Small);
        });
    }

    // ------------------------------------------------------------------------

    // Paint background in case there is no image covering it all:
    let mut shapes = vec![Shape::rect_filled(
        ui_from_space.transform_rect(scene_bbox),
        3.0,
        ui.visuals().extreme_bg_color,
    )];

    // ------------------------------------------------------------------------

    let total_num_images = objects.image.len();

    let hover_radius = 5.0; // TODO(emilk): from egui?

    let mut closest_dist = hover_radius;
    let mut closest_instance_id_hash = InstanceIdHash::NONE;
    let pointer_pos = response.hover_pos();

    let mut check_hovering = |props: &re_data_store::InstanceProps<'_>, dist: f32| {
        if dist <= closest_dist {
            closest_dist = dist;
            closest_instance_id_hash = InstanceIdHash::from_props(props);
        }
    };

    let mut depths_at_pointer = vec![];

    let hovered_instance_id_hash = state
        .hovered_instance
        .as_ref()
        .map_or(InstanceIdHash::NONE, InstanceId::hash);

    for (image_idx, (props, obj)) in objects.image.iter().enumerate() {
        let re_data_store::Image {
            tensor,
            meter,
            legend,
        } = obj;
        let paint_props = paint_properties(
            ctx,
            &hovered_instance_id_hash,
            props,
            DefaultColor::White,
            &None,
        );

        if tensor.shape.len() < 2 {
            continue; // not an image. don't know how to display this!
        }

        let legend = find_legend(*legend, objects);

        match ctx.cache.image.get_view(props.msg_id, tensor, &legend) {
            Ok(tensor_view) => {
                let texture_id = tensor_view.retained_img.texture_id(ui.ctx());

                let rect_in_ui = ui_from_space.transform_rect(Rect::from_min_size(
                    Pos2::ZERO,
                    vec2(tensor.shape[1].size as _, tensor.shape[0].size as _),
                ));
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

                let opacity = if image_idx == 0 {
                    1.0 // bottom image
                } else {
                    // make top images transparent
                    1.0 / total_num_images.at_most(20) as f32 // avoid precision problems in framebuffer
                };
                let tint = paint_props.fg_stroke.color.linear_multiply(opacity);
                shapes.push(egui::Shape::image(texture_id, rect_in_ui, uv, tint));

                if paint_props.is_hovered {
                    shapes.push(Shape::rect_stroke(rect_in_ui, 0.0, paint_props.fg_stroke));
                }

                if let Some(pointer_pos) = pointer_pos {
                    let dist = rect_in_ui.distance_sq_to_pos(pointer_pos).sqrt();
                    let dist = dist.at_least(hover_radius); // allow stuff on top of us to "win"
                    check_hovering(props, dist);

                    if hovered_instance_id_hash.is_instance(props)
                        && rect_in_ui.contains(pointer_pos)
                    {
                        response = crate::ui::image_ui::show_zoomed_image_region_tooltip(
                            ui,
                            response,
                            &tensor_view,
                            rect_in_ui,
                            pointer_pos,
                            *meter,
                        );
                    }

                    if let Some(meter) = *meter {
                        let pos_in_image = space_from_ui.transform_pos(pointer_pos);
                        if let Some(raw_value) =
                            tensor.get(&[pos_in_image.y.round() as _, pos_in_image.x.round() as _])
                        {
                            let raw_value = raw_value.as_f64();
                            let depth_in_meters = raw_value / meter as f64;
                            depths_at_pointer.push(depth_in_meters);
                        }
                    }
                }
            }
            Err(err) => {
                // TODO: do something better here
                re_log::error!("Unhandled tensor error: {:?}", err);
            }
        }
    }

    for (props, obj) in objects.bbox2d.iter() {
        let re_data_store::BBox2D {
            bbox,
            stroke_width,
            label,
        } = obj;
        let paint_props = paint_properties(
            ctx,
            &hovered_instance_id_hash,
            props,
            DefaultColor::Random,
            stroke_width,
        );

        let rect_in_ui =
            ui_from_space.transform_rect(Rect::from_min_max(bbox.min.into(), bbox.max.into()));
        let rounding = 2.0;
        shapes.push(Shape::rect_stroke(
            rect_in_ui,
            rounding,
            paint_props.bg_stroke,
        ));
        shapes.push(Shape::rect_stroke(
            rect_in_ui,
            rounding,
            paint_props.fg_stroke,
        ));

        let mut hover_dist = f32::INFINITY;

        if let Some(pointer_pos) = pointer_pos {
            hover_dist = rect_in_ui.signed_distance_to_pos(pointer_pos).abs();
        }

        if let Some(label) = label {
            let wrap_width = (rect_in_ui.width() - 4.0).at_least(60.0);
            let font_id = TextStyle::Body.resolve(ui.style());
            let galley = ui.fonts().layout_job({
                text::LayoutJob {
                    sections: vec![text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: 0..label.len(),
                        format: TextFormat::simple(font_id, paint_props.fg_stroke.color),
                    }],
                    text: (*label).to_owned(),
                    wrap: TextWrapping {
                        max_width: wrap_width,
                        ..Default::default()
                    },
                    break_on_newline: true,
                    halign: Align::Center,
                    ..Default::default()
                }
            });

            // Place the text centered below the rect:
            let text_anchor_pos = rect_in_ui.center_bottom() + vec2(0.0, 3.0);
            let text_rect =
                Align2::CENTER_TOP.anchor_rect(Rect::from_min_size(text_anchor_pos, galley.size()));
            let bg_rect = text_rect.expand2(vec2(4.0, 2.0));
            shapes.push(Shape::rect_filled(
                bg_rect,
                3.0,
                paint_props.bg_stroke.color,
            ));
            shapes.push(Shape::galley(text_rect.center_top(), galley));
            if let Some(pointer_pos) = pointer_pos {
                hover_dist = hover_dist.min(bg_rect.signed_distance_to_pos(pointer_pos));
            }
        }

        check_hovering(props, hover_dist);
    }

    for (props, obj) in objects.line_segments2d.iter() {
        let re_data_store::LineSegments2D {
            points,
            stroke_width,
        } = obj;
        let paint_props = paint_properties(
            ctx,
            &hovered_instance_id_hash,
            props,
            DefaultColor::Random,
            stroke_width,
        );

        let mut min_dist_sq = f32::INFINITY;

        for &[a, b] in bytemuck::cast_slice::<_, [egui::Pos2; 2]>(points) {
            let a = ui_from_space.transform_pos(a);
            let b = ui_from_space.transform_pos(b);
            shapes.push(Shape::line_segment([a, b], paint_props.bg_stroke));
            shapes.push(Shape::line_segment([a, b], paint_props.fg_stroke));

            if let Some(pointer_pos) = pointer_pos {
                let line_segment_distance_sq =
                    crate::math::line_segment_distance_sq_to_point_2d([a, b], pointer_pos);
                min_dist_sq = min_dist_sq.min(line_segment_distance_sq);
            }
        }

        check_hovering(props, min_dist_sq.sqrt());
    }

    for (props, obj) in objects.point2d.iter() {
        let re_data_store::Point2D { pos, radius } = obj;
        let paint_props = paint_properties(
            ctx,
            &hovered_instance_id_hash,
            props,
            DefaultColor::Random,
            &None,
        );

        let radius = radius.unwrap_or(1.5);
        let radius = paint_props.boost_radius_on_hover(radius);

        let pos_in_ui = ui_from_space.transform_pos(pos2(pos[0], pos[1]));
        shapes.push(Shape::circle_filled(
            pos_in_ui,
            radius + 1.0,
            paint_props.bg_stroke.color,
        ));
        shapes.push(Shape::circle_filled(
            pos_in_ui,
            radius,
            paint_props.fg_stroke.color,
        ));

        if let Some(pointer_pos) = pointer_pos {
            check_hovering(props, pos_in_ui.distance(pointer_pos));
        }
    }

    // ------------------------------------------------------------------------

    let depth_at_pointer = if depths_at_pointer.len() == 1 {
        depths_at_pointer[0] as f32
    } else {
        f32::INFINITY
    };
    project_onto_other_spaces(ctx, space, &response, &space_from_ui, depth_at_pointer);
    show_projections_from_3d_space(ctx, ui, space, &ui_from_space, &mut shapes);

    // ------------------------------------------------------------------------

    painter.extend(shapes);

    state.hovered_instance = closest_instance_id_hash.resolve(&ctx.log_db.obj_db.store);

    response
}

// ------------------------------------------------------------------------

fn project_onto_other_spaces(
    ctx: &mut ViewerContext<'_>,
    space: Option<&ObjPath>,
    response: &Response,
    space_from_ui: &RectTransform,
    z: f32,
) {
    if let Some(pointer_in_screen) = response.hover_pos() {
        let pointer_in_space = space_from_ui.transform_pos(pointer_in_screen);
        ctx.rec_cfg.hovered_space_this_frame = HoveredSpace::TwoD {
            space_2d: space.cloned(),
            pos: glam::vec3(pointer_in_space.x, pointer_in_space.y, z),
        };
    }
}

fn show_projections_from_3d_space(
    ctx: &ViewerContext<'_>,
    ui: &egui::Ui,
    space: Option<&ObjPath>,
    ui_from_space: &RectTransform,
    shapes: &mut Vec<Shape>,
) {
    if let HoveredSpace::ThreeD { target_spaces, .. } = &ctx.rec_cfg.hovered_space_previous_frame {
        for (space_2d, ray_2d, pos_2d) in target_spaces {
            if Some(space_2d) == space {
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
}

// ------------------------------------------------------------------------

struct ObjectPaintProperties {
    is_hovered: bool,
    bg_stroke: Stroke,
    fg_stroke: Stroke,
}

impl ObjectPaintProperties {
    pub fn boost_radius_on_hover(&self, r: f32) -> f32 {
        if self.is_hovered {
            2.0 * r
        } else {
            r
        }
    }
}

#[derive(Clone, Copy)]
enum DefaultColor {
    White,
    Random,
}

fn paint_properties(
    ctx: &mut ViewerContext<'_>,
    hovered: &InstanceIdHash,
    props: &re_data_store::InstanceProps<'_>,
    default_color: DefaultColor,
    stroke_width: &Option<f32>,
) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(196);
    let color = props.color.map_or_else(
        || match default_color {
            DefaultColor::White => Color32::WHITE,
            DefaultColor::Random => {
                let [r, g, b] = ctx.random_color(props);
                Color32::from_rgb(r, g, b)
            }
        },
        to_egui_color,
    );
    let is_hovered = &InstanceIdHash::from_props(props) == hovered;
    let fg_color = if is_hovered { Color32::WHITE } else { color };
    let stroke_width = stroke_width.unwrap_or(1.5);
    let stoke_width = if is_hovered {
        2.0 * stroke_width
    } else {
        stroke_width
    };
    let bg_stroke = Stroke::new(stoke_width + 2.0, bg_color);
    let fg_stroke = Stroke::new(stoke_width, fg_color);

    ObjectPaintProperties {
        is_hovered,
        bg_stroke,
        fg_stroke,
    }
}

fn to_egui_color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
