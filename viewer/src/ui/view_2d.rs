use eframe::egui;
use egui::*;
use emath::RectTransform;

use log_types::*;

use crate::{log_db::SpaceSummary, space_view::ui_data, LogDb, Preview, Selection, ViewerContext};

/// messages: latest version of each object (of all spaces).
pub(crate) fn combined_view_2d(
    log_db: &LogDb,
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    space: &ObjectPath,
    space_summary: &SpaceSummary,
    mut messages: Vec<&LogMsg>,
) {
    if space_summary.bbox2d.is_negative() {
        return;
    }
    crate::profile_function!();

    // Show images first (behind everything else), then bboxes, then points:
    messages.sort_by_key(|msg| match &msg.data {
        Data::Image(_) => 0,
        Data::BBox2D(_) => 1,
        _ => 2,
    });

    let desired_size = {
        let max_size = ui.available_size();
        let mut desired_size = space_summary.bbox2d.size();
        desired_size *= max_size.x / desired_size.x; // fill full width
        desired_size *= (max_size.y / desired_size.y).at_most(1.0); // shrink so we don't fill more than full height
        desired_size
    };

    let (response, painter) = ui.allocate_painter(desired_size, egui::Sense::click());

    let to_screen = egui::emath::RectTransform::from_to(space_summary.bbox2d, response.rect);

    // ------------------------------------------------------------------------

    let hovered = ui
        .ctx()
        .pointer_hover_pos()
        .and_then(|pointer_pos| hovered(space, messages.as_slice(), &to_screen, pointer_pos));

    if let Some(hovered_id) = hovered {
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

    for msg in &messages {
        if msg.space.as_ref() != Some(space) {
            continue;
        }

        let is_hovered = Some(msg.id) == hovered;

        // TODO: different color when selected
        let fg_color = if is_hovered {
            Color32::WHITE
        } else {
            context.object_color(log_db, msg)
        };
        let stoke_width = if is_hovered { 2.5 } else { 1.5 };

        match &msg.data {
            Data::Pos2(pos) => {
                let screen_pos = to_screen.transform_pos(pos.into());
                let r = 1.0 + stoke_width;
                painter.circle_filled(screen_pos, r + 1.0, Color32::from_black_alpha(128));
                painter.circle_filled(screen_pos, r, fg_color);
            }
            Data::BBox2D(bbox) => {
                let screen_rect =
                    to_screen.transform_rect(Rect::from_min_max(bbox.min.into(), bbox.max.into()));
                let rounding = 2.0;
                painter.rect_stroke(
                    screen_rect,
                    rounding,
                    Stroke::new(stoke_width + 1.0, Color32::from_black_alpha(128)),
                );
                painter.rect_stroke(screen_rect, rounding, Stroke::new(stoke_width, fg_color));
            }
            Data::Image(image) => {
                let texture_id = context.image_cache.get(&msg.id, image).texture_id(ui.ctx());
                let screen_rect = to_screen.transform_rect(Rect::from_min_size(
                    Pos2::ZERO,
                    vec2(image.size[0] as _, image.size[1] as _),
                ));
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.add(egui::Shape::image(
                    texture_id,
                    screen_rect,
                    uv,
                    Color32::WHITE,
                ));

                if is_hovered {
                    painter.rect_stroke(screen_rect, 0.0, Stroke::new(stoke_width, fg_color));
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn on_hover_ui(context: &mut ViewerContext, ui: &mut egui::Ui, msg: &LogMsg) {
    // A very short summary:
    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("object_path:");
            ui.label(format!("{}", msg.object_path));
            ui.end_row();

            ui.monospace("data:");
            ui_data(context, ui, &msg.id, &msg.data, Preview::Medium);
            ui.end_row();
        });

    ui.label("(click for more)");
}

fn hovered(
    space: &ObjectPath,
    messages: &[&LogMsg],
    to_screen: &RectTransform,
    pointer_pos: Pos2,
) -> Option<LogId> {
    let hover_radius = 5.0; // TODO: from egui?

    let mut closest_dist = hover_radius;
    let mut closest_id = None;

    for msg in messages {
        if msg.space.as_ref() != Some(space) {
            continue;
        }

        let dist = match &msg.data {
            Data::Pos2(pos) => {
                let screen_pos = to_screen.transform_pos(pos.into());
                screen_pos.distance(pointer_pos)
            }
            Data::BBox2D(bbox) => {
                let screen_rect =
                    to_screen.transform_rect(Rect::from_min_max(bbox.min.into(), bbox.max.into()));
                screen_rect.signed_distance_to_pos(pointer_pos).abs()
            }
            Data::Image(image) => {
                let screen_rect = to_screen.transform_rect(Rect::from_min_size(
                    Pos2::ZERO,
                    vec2(image.size[0] as _, image.size[1] as _),
                ));
                let dist = screen_rect.distance_sq_to_pos(pointer_pos).sqrt();
                dist.at_least(hover_radius) // allow stuff on top of us to "win"
            }
            _ => continue,
        };

        if dist <= closest_dist {
            closest_dist = dist;
            closest_id = Some(msg.id);
        }
    }

    closest_id
}
