use std::sync::Arc;

use egui::{Color32, NumExt as _, Rangef, Rect, Shape, pos2};
use re_ui::UiExt as _;

use crate::TimeRangesUi;

/// How far the zig-zag edges of a gap stick out, horizontally.
pub const MAX_ZIG_WIDTH: f32 = 4.0;

/// Visually separate the different time segments, and mark the limits of the timeline.
///
/// The regions before the first segment, between segments, and after the last
/// segment are filled with a zig-zag-edged dark band.
pub fn paint_time_ranges_gaps(
    time_ranges_ui: &TimeRangesUi,
    ui: &egui::Ui,
    painter: &egui::Painter,
    y_range: Rangef,
) {
    re_tracing::profile_function!();

    // For each gap we are painting this:
    //
    //             zig width
    //             |
    //            <->
    //    \         /  ^
    //     \       /   | zig height
    //      \     /    v
    //      /     \
    //     /       \
    //    /         \
    //    \         /
    //     \       /
    //      \     /
    //      /     \
    //     /       \
    //    /         \
    //
    //    <--------->
    //     gap width
    //
    // Filled with a dark color, plus a stroke and a small drop shadow to the left.

    use itertools::Itertools as _;

    let Rangef {
        min: top,
        max: bottom,
    } = y_range;

    let fill_color = ui.visuals().widgets.noninteractive.bg_fill;
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;

    let paint_time_gap = |gap_left: f32, gap_right: f32| {
        let gap_width = gap_right - gap_left;
        let zig_width = MAX_ZIG_WIDTH.at_most(gap_width / 3.0).at_least(1.0);
        let zig_height = zig_width;
        let shadow_width = 12.0;

        let mut y = top;
        let mut row = 0; // 0 = start wide, 1 = start narrow

        let mut mesh = egui::Mesh::default();
        let mut shadow_mesh = egui::Mesh::default();
        let mut left_line_strip = vec![];
        let mut right_line_strip = vec![];

        while y - zig_height <= bottom {
            let (left, right) = if row % 2 == 0 {
                // full width
                (gap_left, gap_right)
            } else {
                // contracted
                (gap_left + zig_width, gap_right - zig_width)
            };

            let left_pos = pos2(left, y);
            let right_pos = pos2(right, y);

            if !mesh.is_empty() {
                let next_left_vidx = mesh.vertices.len() as u32;
                let next_right_vidx = next_left_vidx + 1;
                let prev_left_vidx = next_left_vidx - 2;
                let prev_right_vidx = next_right_vidx - 2;

                mesh.add_triangle(prev_left_vidx, next_left_vidx, prev_right_vidx);
                mesh.add_triangle(next_left_vidx, prev_right_vidx, next_right_vidx);
            }

            mesh.colored_vertex(left_pos, fill_color);
            mesh.colored_vertex(right_pos, fill_color);

            shadow_mesh.colored_vertex(pos2(right - shadow_width, y), Color32::TRANSPARENT);
            shadow_mesh.colored_vertex(right_pos, ui.tokens().shadow_gradient_dark_start);

            left_line_strip.push(left_pos);
            right_line_strip.push(right_pos);

            y += zig_height;
            row += 1;
        }

        // Regular & shadow mesh have the same topology!
        shadow_mesh.indices.clone_from(&mesh.indices);

        painter.add(Shape::Mesh(Arc::new(mesh)));
        painter.add(Shape::Mesh(Arc::new(shadow_mesh)));
        painter.add(Shape::line(left_line_strip, stroke));
        painter.add(Shape::line(right_line_strip, stroke));
    };

    let zig_zag_first_and_last_edges = true;

    // Margin for the (left or right) end of a gap.
    // Don't use an arbitrarily large value since it can cause platform-specific rendering issues.
    const GAP_END_MARGIN: f32 = 100.0;

    if let Some(segment) = time_ranges_ui.segments.first() {
        let gap_edge = segment.x.start as f32;
        let gap_edge_left_side = ui.content_rect().left() - GAP_END_MARGIN;

        if zig_zag_first_and_last_edges {
            // Left side of first segment - paint as a very wide gap that we only see the right side of
            paint_time_gap(gap_edge_left_side, gap_edge);
        } else {
            // Careful with subtracting a too large number here. Nvidia @ Windows was observed not drawing the rect correctly for -100_000.0
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge - 10_000.0, top), pos2(gap_edge, bottom)),
                0.0,
                fill_color,
            );
            painter.vline(gap_edge, y_range, stroke);
        }
    }

    for (a, b) in time_ranges_ui.segments.iter().tuple_windows() {
        paint_time_gap(a.x.last as f32, b.x.start as f32);
    }

    if let Some(segment) = time_ranges_ui.segments.last() {
        let gap_edge = segment.x.last as f32;
        let gap_edge_right_side = ui.content_rect().right() + GAP_END_MARGIN;

        if zig_zag_first_and_last_edges {
            // Right side of last segment - paint as a very wide gap that we only see the left side of
            paint_time_gap(gap_edge, gap_edge_right_side);
        } else {
            painter.rect_filled(
                Rect::from_min_max(pos2(gap_edge, top), pos2(gap_edge_right_side, bottom)),
                0.0,
                fill_color,
            );
            painter.vline(gap_edge, y_range, stroke);
        }
    }
}
