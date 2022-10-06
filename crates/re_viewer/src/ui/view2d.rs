use eframe::emath::RectTransform;
use egui::*;

use re_data_store::{InstanceId, InstanceIdHash};
use re_log_types::*;

use crate::{misc::HoveredSpace, Selection, ViewerContext};

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

    // If ZoomInfo isn't set, we assume "Auto" and scale the full accum_bbox to the available space
    #[serde(skip)]
    zoom: Option<ZoomState>,
}

#[derive(Clone, Copy)]
struct ZoomState {
    scale: f32,   // How many display-pixels an image-pixel will take up
    center: Pos2, // Which pixel will be at the center of the zoomed region
    // Accepting_scroll is a kind of hacky way of preventing the scroll-based updates
    // from later overriding the zoom/drag-based updates.
    accepting_scroll: bool,
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
    /// available size This will generally be used to construct the painter and
    /// subsequent transforms
    fn desired_size_and_offset(&self, available_size: Vec2) -> (Vec2, Vec2) {
        if let Some(zoom) = self.zoom {
            let desired_size = self.scene_bbox_accum.size() * zoom.scale;

            // Try to keep the center of the image in the middle of the available size
            let offset = (zoom.center.to_vec2() - self.scene_bbox_accum.left_top().to_vec2())
                * zoom.scale
                - available_size / 2.0;

            (desired_size, offset)
        } else {
            // Otherwise, we autoscale to fit available space while maintaining aspect ratio
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

    /// Update our zoom state based on response
    /// If nothing else happens this will reset `accepting_scroll` to true when appropriate
    fn update(
        &mut self,
        response: &egui::Response,
        ui_to_space: egui::emath::RectTransform,
        available_size: Vec2,
    ) {
        // First check if we are initiating zoom-mode
        if self.zoom.is_none() && response.hovered() {
            let input = response.ctx.input();

            // Process zoom changes
            let input_zoom = input.zoom_delta();
            if input_zoom > 1.0 {
                // If increasing zoom, initialize zoom context if necessary
                if self.zoom.is_none() {
                    let scale = response.rect.height() / self.scene_bbox_accum.height();
                    let center = self.scene_bbox_accum.center();
                    self.zoom = Some(ZoomState {
                        scale,
                        center,
                        accepting_scroll: false,
                    });
                }
            }
        }

        // Now, if we are in zoom-mode update our state
        if let Some(mut zoom) = self.zoom {
            // Default to accepting scroll unless we disable it
            zoom.accepting_scroll = true;

            if response.hovered() {
                let input = response.ctx.input();

                // Process zoom changes
                let input_zoom = input.zoom_delta();

                if input_zoom != 1.0 {
                    let new_scale = zoom.scale * input_zoom;
                    let new_scale = (new_scale * 10.0).round() / 10.0;

                    // Adjust for mouse location while executing zoom
                    if let Some(hover_pos) = input.pointer.hover_pos() {
                        let zoom_loc = ui_to_space.transform_pos(hover_pos);

                        // Pixels under the cursor will shift based on distance from center
                        let dist_from_center = zoom_loc - zoom.center;
                        let shift_in_ui =
                            dist_from_center * new_scale - dist_from_center * zoom.scale;
                        let shift_in_space = shift_in_ui / new_scale;

                        // Need to counteract by moving the center in the direction of shift
                        zoom.center += shift_in_space;
                    }
                    zoom.scale = new_scale;
                    zoom.accepting_scroll = false;
                }

                // If we have zoomed past our native size, switch back to auto-zoom
                if self.scene_bbox_accum.size().x * zoom.scale < available_size.x
                    && self.scene_bbox_accum.size().y * zoom.scale < available_size.y
                {
                    self.zoom = None;
                    return;
                }

                if response.dragged_by(egui::PointerButton::Primary) {
                    // Adjust center based on drag
                    let center = zoom.center - response.drag_delta() / zoom.scale;
                    zoom.center = center;
                    zoom.accepting_scroll = false;
                }
            }
            self.zoom = Some(zoom);
        }
    }

    /// Take the offset from the `ScrollArea` and apply it back to center so that other
    /// scroll interfaces work as expected.
    fn capture_scroll(&mut self, offset: Vec2, available_size: Vec2) {
        if let Some(ZoomState {
            scale,
            accepting_scroll,
            ..
        }) = self.zoom
        {
            if accepting_scroll {
                let center =
                    self.scene_bbox_accum.left_top() + (available_size / 2.0 + offset) / scale;
                self.zoom = Some(ZoomState {
                    scale,
                    center,
                    accepting_scroll,
                });
            }
        }
    }
}

pub(crate) fn view_2d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut State2D,
    space: Option<&ObjPath>,
    objects: &re_data_store::Objects<'_>, // Note: only the objects that belong to this space
) -> egui::Response {
    crate::profile_function!();

    // Save off the available_size since this is used for some of the layout updates later
    let available_size = ui.available_size();

    let (desired_size, offset) = state.desired_size_and_offset(available_size);

    let mut scroll_area = ScrollArea::both();

    // Bound the offset based on sizes
    // TODO(jleibs): can we derive this from the ScrollArea shape?
    let offset = offset.at_most(desired_size - available_size);
    let offset = offset.at_least(Vec2::ZERO);

    // Update the scroll area based on the computed offset
    // This handles cases of dragging/zooming the image
    scroll_area = scroll_area.scroll_offset(offset);

    let scroll_out = scroll_area.show(ui, |ui| {
        state.scene_bbox_accum = state
            .scene_bbox_accum
            .union(crate::misc::calc_bbox_2d(objects));

        let scene_bbox = state.scene_bbox_accum;

        let (mut response, painter) =
            ui.allocate_painter(desired_size, egui::Sense::click_and_drag());

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
            egui::containers::popup::show_tooltip_at_pointer(
                ui.ctx(),
                Id::new("2d_tooltip"),
                |ui| {
                    ctx.instance_id_button(ui, instance_id);
                    crate::ui::data_ui::view_instance(
                        ctx,
                        ui,
                        instance_id,
                        crate::ui::Preview::Small,
                    );
                },
            );
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
            let re_data_store::Image { tensor, meter } = obj;
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

            let texture_id = ctx
                .cache
                .image
                .get(props.msg_id, tensor)
                .texture_id(ui.ctx());
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

                if hovered_instance_id_hash.is_instance(props) && rect_in_ui.contains(pointer_pos) {
                    let (dynamic_image, _) = ctx.cache.image.get_pair(props.msg_id, tensor);
                    response = crate::ui::image_ui::show_zoomed_image_region_tooltip(
                        ui,
                        response,
                        tensor,
                        dynamic_image,
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
                let shrunken_screen_rect = rect_in_ui.shrink(4.0);
                let font_id = TextStyle::Body.resolve(ui.style());
                let galley = ui.fonts().layout(
                    (*label).to_owned(),
                    font_id,
                    paint_props.fg_stroke.color,
                    shrunken_screen_rect.width(),
                );
                let text_rect = Align2::CENTER_TOP.anchor_rect(Rect::from_min_size(
                    shrunken_screen_rect.center_top(),
                    galley.size(),
                ));
                let bg_rect = text_rect.expand2(vec2(6.0, 2.0));
                shapes.push(Shape::rect_filled(
                    bg_rect,
                    3.0,
                    paint_props.bg_stroke.color,
                ));
                shapes.push(Shape::galley(text_rect.min, galley));
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

        state.hovered_instance = closest_instance_id_hash.resolve(&ctx.log_db.data_store);

        response
    });

    state.capture_scroll(scroll_out.state.offset, available_size);

    scroll_out.inner
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
        ctx.rec_cfg.hovered_space = HoveredSpace::TwoD {
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
    if let HoveredSpace::ThreeD { target_spaces, .. } = &ctx.rec_cfg.hovered_space {
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
