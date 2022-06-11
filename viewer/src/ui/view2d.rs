use egui::*;

use log_types::*;

use crate::{log_db::SpaceSummary, space_view::ui_data, LogDb, Preview, Selection, ViewerContext};

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct State2D {
    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    hovered: Option<LogId>,
}

/// messages: latest version of each object (of all spaces).
pub(crate) fn combined_view_2d(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    state2d: &mut State2D,
    space_summary: &SpaceSummary,
    objects: &data_store::Objects<'_>,
) {
    crate::profile_function!();

    let desired_size = {
        let max_size = ui.available_size();
        let mut desired_size = space_summary.bbox2d.size();
        desired_size *= max_size.x / desired_size.x; // fill full width
        desired_size *= (max_size.y / desired_size.y).at_most(1.0); // shrink so we don't fill more than full height
        desired_size
    };

    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::click());

    // ------------------------------------------------------------------------

    if let Some(hovered_id) = state2d.hovered {
        if response.clicked() {
            context.selection = Selection::LogId(hovered_id);
        }
        if let Some(msg) = log_db.get_msg(&hovered_id) {
            egui::containers::popup::show_tooltip_at_pointer(
                ui.ctx(),
                Id::new("2d_tooltip"),
                |ui| {
                    on_hover_ui(context, ui, msg);
                },
            );
        }
    }

    // ------------------------------------------------------------------------

    let to_screen = egui::emath::RectTransform::from_to(space_summary.bbox2d, response.rect);

    // Paint background in case there is no image covering it all:
    let mut shapes = vec![Shape::rect_filled(
        to_screen.transform_rect(space_summary.bbox2d),
        3.0,
        ui.visuals().extreme_bg_color,
    )];

    // ------------------------------------------------------------------------

    let total_num_images = objects.image.len();

    let hover_radius = 5.0; // TODO: from egui?

    let mut closest_dist = hover_radius;
    let mut closest_id = None;
    let pointer_pos = response.hover_pos();

    let mut check_hovering = |log_id: &LogId, dist: f32| {
        if dist <= closest_dist {
            closest_dist = dist;
            closest_id = Some(*log_id);
        }
    };

    for (image_idx, (_type_path, props, obj)) in objects.image.iter().enumerate() {
        let data_store::Image { image } = obj;
        let paint_props =
            paint_properties(context, &state2d.hovered, props, DefaultColor::White, &None);

        let texture_id = context
            .image_cache
            .get(props.log_id, image)
            .texture_id(ui.ctx());
        let screen_rect = to_screen.transform_rect(Rect::from_min_size(
            Pos2::ZERO,
            vec2(image.size[0] as _, image.size[1] as _),
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
            check_hovering(props.log_id, dist);
        }
    }

    for (_type_path, props, obj) in objects.bbox2d.iter() {
        let data_store::BBox2D { bbox, stroke_width } = obj;
        let paint_props = paint_properties(
            context,
            &state2d.hovered,
            props,
            DefaultColor::Random,
            stroke_width,
        );

        let screen_rect =
            to_screen.transform_rect(Rect::from_min_max(bbox.min.into(), bbox.max.into()));
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

        if let Some(pointer_pos) = pointer_pos {
            check_hovering(
                props.log_id,
                screen_rect.signed_distance_to_pos(pointer_pos).abs(),
            );
        }
    }

    for (_type_path, props, obj) in objects.line_segments2d.iter() {
        let data_store::LineSegments2D {
            line_segments,
            stroke_width,
        } = obj;
        let paint_props = paint_properties(
            context,
            &state2d.hovered,
            props,
            DefaultColor::Random,
            stroke_width,
        );

        for [a, b] in line_segments.iter() {
            let a = to_screen.transform_pos(a.into());
            let b = to_screen.transform_pos(b.into());
            shapes.push(Shape::line_segment([a, b], paint_props.bg_stroke));
        }
        for [a, b] in line_segments.iter() {
            let a = to_screen.transform_pos(a.into());
            let b = to_screen.transform_pos(b.into());
            shapes.push(Shape::line_segment([a, b], paint_props.fg_stroke));
        }

        if let Some(pointer_pos) = pointer_pos {
            let mut min_dist_sq = f32::INFINITY;
            for [a, b] in line_segments.iter() {
                let a = to_screen.transform_pos(a.into());
                let b = to_screen.transform_pos(b.into());
                let line_segment_distance_sq =
                    crate::math::line_segment_distance_sq_to_point([a, b], pointer_pos);
                min_dist_sq = min_dist_sq.min(line_segment_distance_sq);
            }
            check_hovering(props.log_id, min_dist_sq.sqrt());
        }
    }

    for (_type_path, props, obj) in objects.point2d.iter() {
        let data_store::Point2D { pos, radius } = obj;
        let paint_props = paint_properties(
            context,
            &state2d.hovered,
            props,
            DefaultColor::Random,
            &None,
        );

        let radius = radius.unwrap_or(1.5);
        let radius = paint_props.boost_radius_on_hover(radius);

        let screen_pos = to_screen.transform_pos(pos2(pos[0], pos[1]));
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
            check_hovering(props.log_id, screen_pos.distance(pointer_pos));
        }
    }

    painter.extend(shapes);

    state2d.hovered = closest_id;
}

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
    context: &mut ViewerContext,
    hovered: &Option<LogId>,
    props: &data_store::ObjectProps<'_>,
    default_color: DefaultColor,
    stroke_width: &Option<f32>,
) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(128);
    let color = props.color.map_or_else(
        || match default_color {
            DefaultColor::White => Color32::WHITE,
            DefaultColor::Random => {
                let [r, g, b] = context.random_color(props);
                Color32::from_rgb(r, g, b)
            }
        },
        to_egui_color,
    );
    let is_hovered = Some(props.log_id) == hovered.as_ref();
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

pub(crate) fn on_hover_ui(context: &mut ViewerContext, ui: &mut egui::Ui, msg: &DataMsg) {
    // A very short summary:
    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("data_path:");
            ui.label(format!("{}", msg.data_path));
            ui.end_row();

            ui.monospace("data:");
            ui_data(context, ui, &msg.id, &msg.data, Preview::Medium);
            ui.end_row();
        });

    ui.label("(click for more)");
}

fn to_egui_color([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
