//! Show the data density over time for a data stream.

use std::{collections::BTreeMap, ops::RangeInclusive};

use egui::{epaint::Vertex, pos2, remap, Color32, NumExt as _, Rect, Shape};
use itertools::Itertools as _;

use re_log_types::{TimeInt, TimeRange, TimeReal};

use crate::{
    misc::{Item, ViewerContext},
    ui::Blueprint,
};

use super::time_ranges_ui::TimeRangesUi;

const MARGIN_X: f32 = 5.0;
const DENSITIES_PER_UI_PIXEL: f32 = 1.0;

fn smooth(density: &[f32]) -> Vec<f32> {
    crate::profile_function!();

    fn kernel(x: f32) -> f32 {
        (0.25 * std::f32::consts::TAU * x).cos()
    }

    let mut kernel = [
        kernel(-2.0 / 3.0),
        kernel(-1.0 / 3.0),
        kernel(0.0 / 3.0),
        kernel(1.0 / 3.0),
        kernel(2.0 / 3.0),
    ];
    let kernel_sum = kernel.iter().sum::<f32>();
    for k in &mut kernel {
        *k /= kernel_sum;
    }

    (0..density.len())
        .map(|i| {
            let mut sum = 0.0;
            for (j, &k) in kernel.iter().enumerate() {
                if let Some(&density) = density.get((i + j).saturating_sub(2)) {
                    sum += k * density;
                }
            }
            sum
        })
        .collect()
}

struct DensityGraph {
    /// 0 == min_x, n-1 == max_x
    density: Vec<f32>,
    min_x: f32,
    max_x: f32,
}

impl DensityGraph {
    pub fn new(x_range: RangeInclusive<f32>) -> Self {
        let min_x = *x_range.start() - MARGIN_X;
        let max_x = *x_range.end() + MARGIN_X;
        let n = ((max_x - min_x) * DENSITIES_PER_UI_PIXEL).ceil() as usize;
        Self {
            density: vec![0.0; n],
            min_x,
            max_x,
        }
    }

    pub fn add(&mut self, x: f32, count: f32) {
        let i = remap(
            x,
            self.min_x..=self.max_x,
            0.0..=(self.density.len() as f32 - 1.0),
        );

        if false {
            // nearest neightbor:
            let i = i.round() as usize;

            if let Some(bucket) = self.density.get_mut(i) {
                *bucket += count;
            }
        } else {
            // linearly interpolate where we add the count:
            let fract = i.fract();
            let i = i.floor() as usize;

            if let Some(bucket) = self.density.get_mut(i) {
                *bucket += (1.0 - fract) * count;
            }
            if let Some(bucket) = self.density.get_mut(i + 1) {
                *bucket += fract * count;
            }
        }
    }

    pub fn paint(
        mut self,
        y_range: RangeInclusive<f32>,
        painter: &egui::Painter,
        inactive_color: Color32,
    ) {
        self.density = smooth(&self.density);
        crate::profile_function!();

        let (min_y, max_y) = (*y_range.start(), *y_range.end());
        let center_y = (min_y + max_y) / 2.0;
        let max_radius = 0.9 * (max_y - min_y) / 2.0;

        // We paint a symmetric thing, like so:
        //
        // 0  1 2   3
        // x
        //  \   x---x
        //   \ /
        //    x
        //
        //    x
        //   / \
        //  /   x---x
        // x
        // 0  1 2   3

        fn height_from_density(density: f32) -> f32 {
            5.0 * density // TODO
        }

        let color = inactive_color; // TODO: different color for select regions ?
        let uv = egui::Pos2::ZERO;

        let mut mesh = egui::Mesh::default();
        mesh.vertices.reserve(2 * self.density.len());
        mesh.indices.reserve(6 * (self.density.len() - 1));

        for (i, &density) in self.density.iter().enumerate() {
            // TODO: early-out if density is 0 for long stretches

            // let x = self.min_x + i as f32;
            let x = remap(
                i as f32,
                0.0..=(self.density.len() as f32 - 1.0),
                self.min_x..=self.max_x,
            );

            let radius = height_from_density(density).at_most(max_radius);
            // TODO: color from density

            mesh.vertices.push(Vertex {
                pos: pos2(x, center_y - radius),
                color,
                uv,
            });
            mesh.vertices.push(Vertex {
                pos: pos2(x, center_y + radius),
                color,
                uv,
            });

            if 0 < i {
                let i = i as u32;
                let base = 2 * (i - 1);
                mesh.add_triangle(base, base + 1, base + 2);
                mesh.add_triangle(base + 1, base + 2, base + 3);
            }
        }

        painter.add(Shape::Mesh(mesh));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn data_density_graph_ui(
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    time_area_response: &egui::Response,
    time_area_painter: &egui::Painter,
    ui: &mut egui::Ui,
    num_timeless_messages: usize,
    num_messages_at_time: &BTreeMap<TimeInt, usize>,
    full_width_rect: Rect,
    time_ranges_ui: &TimeRangesUi,
    select_on_click: Item,
) {
    crate::profile_function!();

    let hover_radius = 5.0; // TODO
    let center_y = full_width_rect.center().y;

    // Density over x-axis in UI points.
    let mut density_graph = DensityGraph::new(full_width_rect.x_range());

    // TODO(andreas): Should pass through underlying instance id and be clever about selection vs hover state.
    let is_selected = ctx.selection().iter().contains(&select_on_click);

    // // painting each data point as a separate circle is slow (too many circles!)
    // // so we join time points that are close together.
    // let points_per_time = time_ranges_ui.points_per_time().unwrap_or(f64::INFINITY);
    // let hovered_color = ui.visuals().widgets.hovered.text_color();
    // let selected_time_range = ctx.rec_cfg.time_ctrl.active_loop_selection();

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    let inactive_color = if is_selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals()
            .widgets
            .inactive
            .text_color()
            .linear_multiply(0.75)
    };

    let mut num_hovered_messages = 0;
    let mut hovered_time_range = TimeRange::EMPTY;

    let mut add_data_point = |time_int: TimeInt, count: usize| {
        if count == 0 {
            return;
        }
        let time_real = TimeReal::from(time_int);
        if let Some(x) = time_ranges_ui.x_from_time_f32(time_real) {
            density_graph.add(x, count as _);

            // TODO(emilk): handle hovering better
            let is_hovered = pointer_pos.map_or(false, |pointer_pos| {
                pos2(x, center_y).distance(pointer_pos) < hover_radius
            });
            if is_hovered {
                hovered_time_range = hovered_time_range.union(TimeRange::point(time_int));
                num_hovered_messages += count;
            }
        }
    };

    add_data_point(TimeInt::BEGINNING, num_timeless_messages);

    let visible_time_range = time_ranges_ui.time_range_from_x_range(
        (time_area_painter.clip_rect().left() - MARGIN_X)
            ..=(time_area_painter.clip_rect().right() + MARGIN_X),
    );

    for (&time, &num_messages_at_time) in
        num_messages_at_time.range(visible_time_range.min..=visible_time_range.max)
    {
        add_data_point(time, num_messages_at_time);
    }

    density_graph.paint(full_width_rect.y_range(), time_area_painter, inactive_color);

    if 0 < num_hovered_messages {
        if time_area_response.clicked_by(egui::PointerButton::Primary) {
            ctx.set_single_selection(select_on_click);
            ctx.rec_cfg.time_ctrl.set_time(hovered_time_range.min);
            ctx.rec_cfg.time_ctrl.pause();
        } else if !ui.ctx().memory(|mem| mem.is_anything_being_dragged()) {
            show_msg_ids_tooltip(
                ctx,
                blueprint,
                ui.ctx(),
                &select_on_click,
                hovered_time_range,
                num_hovered_messages,
            );
        }
    }
}

fn show_msg_ids_tooltip(
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    egui_ctx: &egui::Context,
    item: &Item,
    time_range: TimeRange,
    num_messages: usize,
) {
    use crate::ui::data_ui::DataUi as _;

    egui::show_tooltip_at_pointer(egui_ctx, egui::Id::new("data_tooltip"), |ui| {
        if time_range.min == time_range.max {
            if num_messages > 1 {
                ui.label(format!("{num_messages} messages"));
                ui.add_space(8.0);
                // Could be an entity made up of many components logged at the same time.
                // Still show a preview!
            }
            crate::ui::selection_panel::what_is_selected_ui(ui, ctx, blueprint, item);
            ui.add_space(8.0);

            let timeline = *ctx.rec_cfg.time_ctrl.timeline();
            let query = re_arrow_store::LatestAtQuery::new(timeline, time_range.min);
            item.data_ui(ctx, ui, crate::ui::UiVerbosity::Reduced, &query);
        } else {
            ui.label(format!("{num_messages} messages"));
        }
    });
}
