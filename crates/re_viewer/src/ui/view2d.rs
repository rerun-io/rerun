use eframe::emath::RectTransform;
use egui::*;

use re_log_types::*;

use crate::{misc::HoveredSpace, Selection, ViewerContext};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct State2D {
    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered_obj: Option<ObjPath>,

    /// Estimated bounding box of all data. Accumulated.
    #[serde(skip)]
    scene_bbox_accum: epaint::Rect,
}

impl Default for State2D {
    fn default() -> Self {
        Self {
            hovered_obj: Default::default(),
            scene_bbox_accum: epaint::Rect::NOTHING,
        }
    }
}

impl State2D {
    /// Size of the 2D bounding box, if any.
    pub fn size(&self) -> Option<egui::Vec2> {
        if self.scene_bbox_accum.is_positive() {
            Some(self.scene_bbox_accum.size())
        } else {
            None
        }
    }
}

/// messages: latest version of each object (of all spaces).
pub(crate) fn view_2d(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    state: &mut State2D,
    space: Option<&ObjPath>,
    objects: &re_data_store::Objects<'_>, // Note: only the objects that belong to this space
) -> egui::Response {
    crate::profile_function!();

    state.scene_bbox_accum = state
        .scene_bbox_accum
        .union(crate::misc::calc_bbox_2d(objects));
    let scene_bbox = if state.scene_bbox_accum.is_positive() {
        state.scene_bbox_accum
    } else {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
    };

    let desired_size = {
        let max_size = ui.available_size();
        let mut desired_size = scene_bbox.size();
        desired_size *= max_size.x / desired_size.x; // fill full width
        desired_size *= (max_size.y / desired_size.y).at_most(1.0); // shrink so we don't fill more than full height

        if desired_size.is_finite() {
            desired_size
        } else {
            max_size
        }
    };

    let (mut response, painter) = ui.allocate_painter(desired_size, egui::Sense::click());

    // ------------------------------------------------------------------------

    if let Some(obj_path) = &state.hovered_obj {
        if response.clicked() {
            ctx.rec_cfg.selection = Selection::ObjPath(obj_path.clone());
        }
        egui::containers::popup::show_tooltip_at_pointer(ui.ctx(), Id::new("2d_tooltip"), |ui| {
            ctx.obj_path_button(ui, obj_path);
            crate::ui::context_panel::view_object(ctx, ui, obj_path, crate::ui::Preview::Small);
        });
    }

    // ------------------------------------------------------------------------

    // Screen coordinates from space coordinates.
    let screen_from_space = egui::emath::RectTransform::from_to(scene_bbox, response.rect);
    let space_from_screen = screen_from_space.inverse();

    // Paint background in case there is no image covering it all:
    let mut shapes = vec![Shape::rect_filled(
        screen_from_space.transform_rect(scene_bbox),
        3.0,
        ui.visuals().extreme_bg_color,
    )];

    // ------------------------------------------------------------------------

    let total_num_images = objects.image.len();

    let hover_radius = 5.0; // TODO(emilk): from egui?

    let mut closest_dist = hover_radius;
    let mut closest_obj_path = None;
    let pointer_pos = response.hover_pos();

    let mut check_hovering = |obj_path: &ObjPath, dist: f32| {
        if dist <= closest_dist {
            closest_dist = dist;
            closest_obj_path = Some(obj_path.clone());
        }
    };

    let mut depths_at_pointer = vec![];

    for (image_idx, (props, obj)) in objects.image.iter().enumerate() {
        let re_data_store::Image { tensor, meter } = obj;
        let paint_props =
            paint_properties(ctx, &state.hovered_obj, props, DefaultColor::White, &None);

        if tensor.shape.len() < 2 {
            continue; // not an image. don't know how to display this!
        }

        let texture_id = ctx
            .cache
            .image
            .get(props.msg_id, tensor)
            .texture_id(ui.ctx());
        let screen_rect = screen_from_space.transform_rect(Rect::from_min_size(
            Pos2::ZERO,
            vec2(tensor.shape[1] as _, tensor.shape[0] as _),
        ));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

        let opacity = if image_idx == 0 {
            1.0 // bottom image
        } else {
            // make top images transparent
            1.0 / total_num_images.at_most(20) as f32 // avoid precision problems in framebuffer
        };
        let tint = paint_props.fg_stroke.color.linear_multiply(opacity);
        shapes.push(egui::Shape::image(texture_id, screen_rect, uv, tint));

        if paint_props.is_hovered {
            shapes.push(Shape::rect_stroke(screen_rect, 0.0, paint_props.fg_stroke));
        }

        if let Some(pointer_pos) = pointer_pos {
            let dist = screen_rect.distance_sq_to_pos(pointer_pos).sqrt();
            let dist = dist.at_least(hover_radius); // allow stuff on top of us to "win"
            check_hovering(props.obj_path, dist);

            if Some(props.obj_path) == state.hovered_obj.as_ref()
                && screen_rect.contains(pointer_pos)
            {
                let (dynamic_image, _) = ctx.cache.image.get_pair(props.msg_id, tensor);
                response = crate::ui::context_panel::show_zoomed_image_region_tooltip(
                    ui,
                    response,
                    tensor,
                    dynamic_image,
                    screen_rect,
                    pointer_pos,
                    *meter,
                );

                if let Some(meter) = *meter {
                    let pixel_pos = space_from_screen.transform_pos(pointer_pos);
                    if let Some(raw_value) =
                        tensor.get(&[pixel_pos.y.round() as _, pixel_pos.x.round() as _])
                    {
                        let raw_value = raw_value.as_f64();
                        let depth_in_meters = raw_value / meter as f64;
                        depths_at_pointer.push(depth_in_meters);
                    }
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
            &state.hovered_obj,
            props,
            DefaultColor::Random,
            stroke_width,
        );

        let screen_rect =
            screen_from_space.transform_rect(Rect::from_min_max(bbox.min.into(), bbox.max.into()));
        let rounding = 2.0;
        shapes.push(Shape::rect_stroke(
            screen_rect,
            rounding,
            paint_props.bg_stroke,
        ));
        shapes.push(Shape::rect_stroke(
            screen_rect,
            rounding,
            paint_props.fg_stroke,
        ));

        let mut hover_dist = f32::INFINITY;

        if let Some(pointer_pos) = pointer_pos {
            hover_dist = screen_rect.signed_distance_to_pos(pointer_pos).abs();
        }

        if let Some(label) = label {
            let shrunken_screen_rect = screen_rect.shrink(4.0);
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

        check_hovering(props.obj_path, hover_dist);
    }

    for (props, obj) in objects.line_segments2d.iter() {
        let re_data_store::LineSegments2D {
            points,
            stroke_width,
        } = obj;
        let paint_props = paint_properties(
            ctx,
            &state.hovered_obj,
            props,
            DefaultColor::Random,
            stroke_width,
        );

        let mut min_dist_sq = f32::INFINITY;

        for &[a, b] in bytemuck::cast_slice::<_, [egui::Pos2; 2]>(points) {
            let a = screen_from_space.transform_pos(a);
            let b = screen_from_space.transform_pos(b);
            shapes.push(Shape::line_segment([a, b], paint_props.bg_stroke));
            shapes.push(Shape::line_segment([a, b], paint_props.fg_stroke));

            if let Some(pointer_pos) = pointer_pos {
                let line_segment_distance_sq =
                    crate::math::line_segment_distance_sq_to_point_2d([a, b], pointer_pos);
                min_dist_sq = min_dist_sq.min(line_segment_distance_sq);
            }
        }

        check_hovering(props.obj_path, min_dist_sq.sqrt());
    }

    for (props, obj) in objects.point2d.iter() {
        let re_data_store::Point2D { pos, radius } = obj;
        let paint_props =
            paint_properties(ctx, &state.hovered_obj, props, DefaultColor::Random, &None);

        let radius = radius.unwrap_or(1.5);
        let radius = paint_props.boost_radius_on_hover(radius);

        let screen_pos = screen_from_space.transform_pos(pos2(pos[0], pos[1]));
        shapes.push(Shape::circle_filled(
            screen_pos,
            radius + 1.0,
            paint_props.bg_stroke.color,
        ));
        shapes.push(Shape::circle_filled(
            screen_pos,
            radius,
            paint_props.fg_stroke.color,
        ));

        if let Some(pointer_pos) = pointer_pos {
            check_hovering(props.obj_path, screen_pos.distance(pointer_pos));
        }
    }

    // ------------------------------------------------------------------------

    let depth_at_pointer = if depths_at_pointer.len() == 1 {
        depths_at_pointer[0] as f32
    } else {
        f32::INFINITY
    };
    project_onto_other_spaces(ctx, space, &response, &space_from_screen, depth_at_pointer);
    show_projections_from_3d_space(ctx, ui, space, &screen_from_space, &mut shapes);

    // ------------------------------------------------------------------------

    painter.extend(shapes);

    state.hovered_obj = closest_obj_path;

    response
}

// ------------------------------------------------------------------------

fn project_onto_other_spaces(
    ctx: &mut ViewerContext<'_>,
    space: Option<&ObjPath>,
    response: &Response,
    space_from_screen: &RectTransform,
    z: f32,
) {
    if let Some(pointer_in_screen) = response.hover_pos() {
        let pointer_in_space = space_from_screen.transform_pos(pointer_in_screen);
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
    screen_from_space: &RectTransform,
    shapes: &mut Vec<Shape>,
) {
    if let HoveredSpace::ThreeD { target_spaces, .. } = &ctx.rec_cfg.hovered_space {
        for (space_2d, ray_2d, pos_2d) in target_spaces {
            if Some(space_2d) == space {
                if let Some(pos_2d) = pos_2d {
                    // User is hovering a 2D point inside a 3D view.
                    let screen_pos = screen_from_space.transform_pos(pos2(pos_2d.x, pos_2d.y));
                    let radius = 4.0;
                    shapes.push(Shape::circle_filled(
                        screen_pos,
                        radius + 2.0,
                        Color32::BLACK,
                    ));
                    shapes.push(Shape::circle_filled(screen_pos, radius, Color32::WHITE));

                    let text = format!("Depth: {:.3} m", pos_2d.z);
                    let font_id = egui::TextStyle::Body.resolve(ui.style());
                    let galley = ui.fonts().layout_no_wrap(text, font_id, Color32::WHITE);
                    let rect = Align2::CENTER_TOP.anchor_rect(Rect::from_min_size(
                        screen_pos + vec2(0.0, 5.0),
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

                        let origin = screen_from_space.transform_pos(origin);
                        let end = screen_from_space.transform_pos(end);

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
    hovered: &Option<ObjPath>,
    props: &re_data_store::ObjectProps<'_>,
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
    let is_hovered = Some(props.obj_path) == hovered.as_ref();
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
