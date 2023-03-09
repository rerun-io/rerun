//! Show the data density over time for a data stream.

use std::{collections::BTreeMap, ops::RangeInclusive};

use egui::{epaint::Vertex, pos2, remap, Color32, NumExt as _, Rect, Shape};

use re_log_types::{TimeInt, TimeRange, TimeReal};

use crate::{
    misc::{Item, ViewerContext},
    ui::Blueprint,
};

use super::time_ranges_ui::TimeRangesUi;

const MARGIN_X: f32 = 2.0;
const DENSITIES_PER_UI_PIXEL: f32 = 1.0;

// ----------------------------------------------------------------------------

/// Persistent data for painting the data density graph.
///
/// Used to dynamically normalize the data density graph based on
/// the output of the previous frame.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct DataDensityGraphPainter {
    /// The maximum density of the previous frame.
    /// This is what we use to normalize the density graphs.
    previous_max_density: f32,

    /// The maximum density we've seen so far this frame.
    next_max_density: f32,
}

impl DataDensityGraphPainter {
    pub fn begin_frame(&mut self, egui_ctx: &egui::Context) {
        if self.next_max_density == 0.0 {
            return;
        }

        let dt = egui_ctx.input(|input| input.stable_dt.at_most(0.1));

        let new = egui::lerp(
            self.previous_max_density..=self.next_max_density,
            egui::emath::exponential_smooth_factor(0.90, 0.2, dt),
        );

        if self.previous_max_density != new {
            egui_ctx.request_repaint();
        }

        self.previous_max_density = new;
        self.next_max_density = 0.0;
    }

    /// Return something in the 0-1 range.
    pub fn normalize_density(&mut self, density: f32) -> f32 {
        debug_assert!(density >= 0.0);

        self.next_max_density = self.next_max_density.max(density);

        if self.previous_max_density > 0.0 {
            (density / self.previous_max_density).at_most(1.0)
        } else {
            density
        }
    }
}

// ----------------------------------------------------------------------------

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
        debug_assert!(0.0 <= count);

        let i = remap(
            x,
            self.min_x..=self.max_x,
            0.0..=(self.density.len() as f32 - 1.0),
        );

        if i <= -1.0 {
            return;
        }

        if false {
            // nearest neightbor:
            let i = i.round() as usize;

            if let Some(bucket) = self.density.get_mut(i) {
                *bucket += count;
            }
        } else {
            // linearly interpolate where we add the count:
            let fract = i - i.floor();
            debug_assert!(0.0 <= fract && fract <= 1.0);
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
        &self,
        data_dentity_graph_painter: &mut DataDensityGraphPainter,
        y_range: RangeInclusive<f32>,
        painter: &egui::Painter,
        full_color: Color32,
        hovered_x_range: RangeInclusive<f32>,
    ) {
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

            let normalized_density = data_dentity_graph_painter.normalize_density(density);
            let radius = if normalized_density == 0.0 {
                0.0
            } else {
                // Make sure we see small things even when they are dwarfed by the max:
                const MIN_RADIUS: f32 = 1.0;
                (max_radius * normalized_density).at_least(MIN_RADIUS)
            };
            let color = if hovered_x_range.contains(&x) {
                Color32::WHITE
            } else {
                full_color.gamma_multiply(remap(normalized_density, 0.0..=1.0, 0.35..=1.0))
            };

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

// ----------------------------------------------------------------------------

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
        debug_assert!(k.is_finite() && 0.0 < *k);
    }

    (0..density.len())
        .map(|i| {
            let mut sum = 0.0;
            for (j, &k) in kernel.iter().enumerate() {
                if let Some(&density) = density.get((i + j).saturating_sub(2)) {
                    debug_assert!(density >= 0.0);
                    sum += k * density;
                }
            }
            debug_assert!(sum.is_finite() && 0.0 <= sum);
            sum
        })
        .collect()
}

// ----------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn data_density_graph_ui(
    data_dentity_graph_painter: &mut DataDensityGraphPainter,
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

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let interact_radius_sq = ui.style().interaction.resize_grab_radius_side.powi(2);
    let center_y = full_width_rect.center().y;

    // Density over x-axis in UI points.
    let mut density_graph = DensityGraph::new(full_width_rect.x_range());

    let mut num_hovered_messages = 0;
    let mut hovered_time_range = TimeRange::EMPTY;

    {
        let mut add_data_point = |time_int: TimeInt, count: usize| {
            if count == 0 {
                return;
            }
            let time_real = TimeReal::from(time_int);
            if let Some(x) = time_ranges_ui.x_from_time_f32(time_real) {
                density_graph.add(x, count as _);

                if let Some(pointer_pos) = pointer_pos {
                    let distance_sq = pos2(x, center_y).distance_sq(pointer_pos);
                    let is_hovered = distance_sq < interact_radius_sq;

                    if is_hovered {
                        hovered_time_range = hovered_time_range.union(TimeRange::point(time_int));
                        num_hovered_messages += count;
                    }
                }
            }
        };
        add_data_point(TimeInt::BEGINNING, num_timeless_messages);
        let visible_time_range = time_ranges_ui.time_range_from_x_range(
            (full_width_rect.left() - MARGIN_X)..=(full_width_rect.right() + MARGIN_X),
        );
        for (&time, &num_messages_at_time) in
            num_messages_at_time.range(visible_time_range.min..=visible_time_range.max)
        {
            add_data_point(time, num_messages_at_time);
        }
    }

    let is_hovered = num_hovered_messages > 0;
    let graph_color = graph_color(ctx, &select_on_click, ui);

    let hovered_x_range = (time_ranges_ui
        .x_from_time_f32(hovered_time_range.min.into())
        .unwrap_or(f32::MAX)
        - MARGIN_X)
        ..=(time_ranges_ui
            .x_from_time_f32(hovered_time_range.max.into())
            .unwrap_or(f32::MIN)
            + MARGIN_X);

    time_area_painter.hline(
        full_width_rect.x_range(),
        full_width_rect.center().y,
        (
            0.5,
            egui::Color32::from_additive_luminance(if is_hovered { 64 } else { 32 }),
        ),
    );

    density_graph.density = smooth(&density_graph.density);
    density_graph.paint(
        data_dentity_graph_painter,
        full_width_rect.y_range(),
        time_area_painter,
        graph_color,
        hovered_x_range,
    );

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

fn graph_color(ctx: &mut ViewerContext<'_>, select_on_click: &Item, ui: &mut egui::Ui) -> Color32 {
    let is_selected = ctx.selection().contains(select_on_click);
    // let hovered_color = ui.visuals().widgets.hovered.text_color();
    if is_selected {
        make_brighter(ui.visuals().selection.bg_fill)
    } else {
        Color32::from_gray(196)
    }
}

fn make_brighter(color: Color32) -> Color32 {
    let [r, g, b, _] = color.to_array();
    egui::Color32::from_rgb(
        r.saturating_add(64),
        g.saturating_add(64),
        b.saturating_add(64),
    )
}

fn show_msg_ids_tooltip(
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    egui_ctx: &egui::Context,
    item: &Item,
    time_range: TimeRange,
    num_messages: usize,
) {
    if num_messages == 0 {
        return;
    }

    use crate::ui::data_ui::DataUi as _;

    egui::show_tooltip_at_pointer(egui_ctx, egui::Id::new("data_tooltip"), |ui| {
        if num_messages == 1 {
            ui.label(format!("{num_messages} message"));
        } else {
            ui.label(format!("{num_messages} messages"));
        }

        ui.add_space(8.0);
        crate::ui::selection_panel::what_is_selected_ui(ui, ctx, blueprint, item);
        ui.add_space(8.0);

        let timeline = *ctx.rec_cfg.time_ctrl.timeline();
        let query = re_arrow_store::LatestAtQuery::new(timeline, time_range.max);
        item.data_ui(ctx, ui, crate::ui::UiVerbosity::Reduced, &query);
    });
}
