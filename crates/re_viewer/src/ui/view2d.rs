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
}

impl Default for State2D {
    fn default() -> Self {
        Self {
            hovered_instance: Default::default(),
            scene_bbox_accum: epaint::Rect::NOTHING,
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

    // Screen coordinates from space coordinates.
    let ui_from_space = egui::emath::RectTransform::from_to(scene_bbox, response.rect);
    let space_from_ui = ui_from_space.inverse();

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
