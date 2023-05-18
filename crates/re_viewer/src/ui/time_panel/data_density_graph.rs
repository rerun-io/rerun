//! Show the data density over time for a data stream.
//!
//! The data density is the number of data points per unit of time.
//! We collect this into a histogram, blur it, and then paint it.

use std::ops::RangeInclusive;

use egui::{epaint::Vertex, lerp, pos2, remap, Color32, NumExt as _, Rect, Shape};

use re_data_store::TimeHistogram;
use re_log_types::{TimeInt, TimeRange, TimeReal};

use crate::{
    misc::{Item, ViewerContext},
    ui::Blueprint,
};

use super::time_ranges_ui::TimeRangesUi;

// ----------------------------------------------------------------------------

/// We need some margin because of the blurring.
const MARGIN_X: f32 = 2.0;

/// Higher = slower, but more accurate.
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

        let dt = egui_ctx.input(|input| input.stable_dt).at_most(0.1);

        let new = lerp(
            self.previous_max_density..=self.next_max_density,
            egui::emath::exponential_smooth_factor(0.90, 0.1, dt),
        );

        if (self.previous_max_density - new).abs() > 0.01 {
            egui_ctx.request_repaint();
        }

        self.previous_max_density = new;

        // If we set this to zero, then a single data point will look weirdly high,
        // so we set it to a small value instead.
        self.next_max_density = 2.0;
    }

    /// Return something in the 0-1 range.
    pub fn normalize_density(&mut self, density: f32) -> f32 {
        debug_assert!(density >= 0.0);

        self.next_max_density = self.next_max_density.max(density);

        if self.previous_max_density > 0.0 {
            (density / self.previous_max_density).at_most(1.0)
        } else {
            density.at_most(1.0)
        }
    }
}

// ----------------------------------------------------------------------------

struct DensityGraph {
    /// Number of datapoints per bucket.
    /// 0 == min_x, n-1 == max_x.
    buckets: Vec<f32>,
    min_x: f32,
    max_x: f32,
}

impl DensityGraph {
    pub fn new(x_range: RangeInclusive<f32>) -> Self {
        let min_x = *x_range.start() - MARGIN_X;
        let max_x = *x_range.end() + MARGIN_X;
        let n = ((max_x - min_x) * DENSITIES_PER_UI_PIXEL).ceil() as usize;
        Self {
            buckets: vec![0.0; n],
            min_x,
            max_x,
        }
    }

    /// We return a float so user can interpolate between buckets.
    fn bucket_index_from_x(&self, x: f32) -> f32 {
        remap(
            x,
            self.min_x..=self.max_x,
            0.0..=(self.buckets.len() as f32 - 1.0),
        )
    }

    fn x_from_bucket_index(&self, i: usize) -> f32 {
        remap(
            i as f32,
            0.0..=(self.buckets.len() as f32 - 1.0),
            self.min_x..=self.max_x,
        )
    }

    pub fn add_point(&mut self, x: f32, count: f32) {
        debug_assert!(0.0 <= count);

        let i = self.bucket_index_from_x(x);

        // linearly interpolate where we add the count:
        let fract = i - i.floor();
        debug_assert!(0.0 <= fract && fract <= 1.0);
        let i = i.floor() as i64;

        if let Ok(i) = usize::try_from(i) {
            if let Some(bucket) = self.buckets.get_mut(i) {
                *bucket += (1.0 - fract) * count;
            }
        }
        if let Ok(i) = usize::try_from(i + 1) {
            if let Some(bucket) = self.buckets.get_mut(i) {
                *bucket += fract * count;
            }
        }
    }

    pub fn add_range(&mut self, (min_x, max_x): (f32, f32), count: f32) {
        debug_assert!(min_x <= max_x);

        if min_x == max_x {
            let center_x = lerp(min_x..=max_x, 0.5);
            self.add_point(center_x, count);
            return;
        }

        // box filter:

        let min_bucket = self.bucket_index_from_x(min_x);
        let max_bucket = self.bucket_index_from_x(max_x);

        // example: we want to add to the range [3.7, 5.2].
        // We then want to add to the buckets [3, 4, 5, 6],
        // but not in equal amounts.

        let first_bucket_factor = 1.0 - (min_bucket - min_bucket.floor());
        let num_full_buckets = 1.0 + max_bucket.floor() - min_bucket.ceil();
        let last_bucket_factor = 1.0 - (max_bucket.ceil() - max_bucket);
        let count_per_bucket =
            count / (first_bucket_factor + num_full_buckets + last_bucket_factor);

        // first bucket, partially filled:
        if let Ok(i) = usize::try_from(min_bucket.floor() as i64) {
            if let Some(bucket) = self.buckets.get_mut(i) {
                *bucket += first_bucket_factor * count_per_bucket;
            }
        }

        // full buckets:
        for i in (min_bucket.ceil() as i64)..=(max_bucket.floor() as i64) {
            if let Ok(i) = usize::try_from(i) {
                if let Some(bucket) = self.buckets.get_mut(i) {
                    *bucket += count_per_bucket;
                }
            }
        }

        // last bucket, partially filled:
        if let Ok(i) = usize::try_from(max_bucket.ceil() as i64) {
            if let Some(bucket) = self.buckets.get_mut(i) {
                *bucket += last_bucket_factor * count_per_bucket;
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
        let max_radius = (max_y - min_y) / 2.0;

        // We paint a symmetric plot, with extra feathering for anti-aliasing:
        //
        // bucket: 0  1 2   3
        //
        //         0
        //          \   x---x
        //         1 \ /
        //          \ 4 x---x
        //           \ /
        //            5
        //
        //            6
        //           / \
        //          / 7 x---x
        //         2 / \
        //          /   x---x
        //         3
        //
        // bucket: 0  1 2   3
        //
        // This means we have an inner radius, and an outer radius.
        // We have four vertices per bucket, and six triangles.

        let pixel_size = 1.0 / painter.ctx().pixels_per_point();
        let feather_radius = 0.5 * pixel_size;

        let uv = egui::Pos2::ZERO;

        let mut mesh = egui::Mesh::default();
        mesh.vertices.reserve(4 * self.buckets.len());

        for (i, &density) in self.buckets.iter().enumerate() {
            // TODO(emilk): early-out if density is 0 for long stretches

            let x = self.x_from_bucket_index(i);

            let normalized_density = data_dentity_graph_painter.normalize_density(density);

            let (inner_radius, inner_color) = if normalized_density == 0.0 {
                (0.0, Color32::TRANSPARENT)
            } else {
                // Make sure we see small things even when they are dwarfed
                // by the max due to the normalization:
                const MIN_RADIUS: f32 = 1.5;
                let inner_radius =
                    (max_radius * normalized_density).at_least(MIN_RADIUS) - feather_radius;

                let inner_color = if hovered_x_range.contains(&x) {
                    Color32::WHITE
                } else {
                    full_color.gamma_multiply(lerp(0.5..=1.0, normalized_density))
                };
                (inner_radius, inner_color)
            };
            let outer_radius = inner_radius + feather_radius;

            mesh.vertices.extend_from_slice(&[
                Vertex {
                    pos: pos2(x, center_y - outer_radius),
                    color: Color32::TRANSPARENT,
                    uv,
                },
                Vertex {
                    pos: pos2(x, center_y - inner_radius),
                    color: inner_color,
                    uv,
                },
                Vertex {
                    pos: pos2(x, center_y + inner_radius),
                    color: inner_color,
                    uv,
                },
                Vertex {
                    pos: pos2(x, center_y + outer_radius),
                    color: Color32::TRANSPARENT,
                    uv,
                },
            ]);
        }

        {
            // I also tried writing this as `flat_map + collect`, but it got much slower in debug builds.
            crate::profile_scope!("triangles");
            mesh.indices.reserve(6 * 3 * (self.buckets.len() - 1));
            for i in 1..self.buckets.len() {
                let i = i as u32;
                let base = 4 * (i - 1);

                // See the numbering in the ASCII art above.
                // Also note that egui/epaint don't care about winding order.
                mesh.indices.extend_from_slice(&[
                    // top:
                    base,
                    base + 1,
                    base + 4,
                    base + 1,
                    base + 4,
                    base + 5,
                    // middle:
                    base + 1,
                    base + 2,
                    base + 5,
                    base + 2,
                    base + 5,
                    base + 6,
                    // bottom:
                    base + 2,
                    base + 3,
                    base + 6,
                    base + 3,
                    base + 6,
                    base + 7,
                ]);
            }
        }

        painter.add(Shape::Mesh(mesh));
    }
}

// ----------------------------------------------------------------------------

/// Blur the input slightly.
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
    time_histogram: &TimeHistogram,
    row_rect: Rect,
    time_ranges_ui: &TimeRangesUi,
    item: Item,
) {
    crate::profile_function!();

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let interact_radius_sq = ui.style().interaction.resize_grab_radius_side.powi(2);
    let center_y = row_rect.center().y;

    // Density over x-axis in UI points.
    let mut density_graph = DensityGraph::new(row_rect.x_range());

    let mut num_hovered_messages = 0;
    let mut hovered_time_range = TimeRange::EMPTY;

    {
        let mut add_data_point = |time_range: TimeRange, count: usize| {
            if count == 0 {
                return;
            }

            if let (Some(min_x), Some(max_x)) = (
                time_ranges_ui.x_from_time_f32(time_range.min.into()),
                time_ranges_ui.x_from_time_f32(time_range.max.into()),
            ) {
                density_graph.add_range((min_x, max_x), count as _);

                // Hover:
                if let Some(pointer_pos) = pointer_pos {
                    let center_x = (min_x + max_x) / 2.0;
                    let distance_sq = pos2(center_x, center_y).distance_sq(pointer_pos);
                    let is_hovered = distance_sq < interact_radius_sq;

                    if is_hovered {
                        hovered_time_range = hovered_time_range.union(time_range);
                        num_hovered_messages += count;
                    }
                }
            } else {
                // We (correctly) assume the time range is narrow, and can be approximated with its center:
                let time_real = TimeReal::from(time_range.center());
                if let Some(x) = time_ranges_ui.x_from_time_f32(time_real) {
                    density_graph.add_point(x, count as _);

                    if let Some(pointer_pos) = pointer_pos {
                        let distance_sq = pos2(x, center_y).distance_sq(pointer_pos);
                        let is_hovered = distance_sq < interact_radius_sq;

                        if is_hovered {
                            hovered_time_range = hovered_time_range.union(time_range);
                            num_hovered_messages += count;
                        }
                    }
                }
            }
        };

        add_data_point(TimeRange::point(TimeInt::BEGINNING), num_timeless_messages);

        let visible_time_range = time_ranges_ui
            .time_range_from_x_range((row_rect.left() - MARGIN_X)..=(row_rect.right() + MARGIN_X));

        // The more zoomed out we are, the bigger chunks of time_histogram we can process at a time.
        // Larger chunks is faster.
        let chunk_size_in_ui_points = 4.0;
        let time_chunk_size =
            (chunk_size_in_ui_points / time_ranges_ui.points_per_time).round() as _;
        let ranges: Vec<_> = {
            crate::profile_scope!("time_histogram.range");
            time_histogram
                .range(
                    visible_time_range.min.as_i64()..=visible_time_range.max.as_i64(),
                    time_chunk_size,
                )
                .collect()
        };

        crate::profile_scope!("add_data_point");
        for (time_range, num_messages_at_time) in ranges {
            add_data_point(
                TimeRange::new(time_range.min.into(), time_range.max.into()),
                num_messages_at_time as _,
            );
        }
    }

    let hovered_x_range = (time_ranges_ui
        .x_from_time_f32(hovered_time_range.min.into())
        .unwrap_or(f32::MAX)
        - MARGIN_X)
        ..=(time_ranges_ui
            .x_from_time_f32(hovered_time_range.max.into())
            .unwrap_or(f32::MIN)
            + MARGIN_X);

    density_graph.buckets = smooth(&density_graph.buckets);

    density_graph.paint(
        data_dentity_graph_painter,
        row_rect.y_range(),
        time_area_painter,
        graph_color(ctx, &item, ui),
        hovered_x_range,
    );

    if 0 < num_hovered_messages {
        ctx.rec_cfg
            .selection_state
            .set_hovered(std::iter::once(item.clone()));

        if time_area_response.clicked_by(egui::PointerButton::Primary) {
            ctx.set_single_selection(item);
            ctx.rec_cfg.time_ctrl.set_time(hovered_time_range.min);
            ctx.rec_cfg.time_ctrl.pause();
        } else if !ui.ctx().memory(|mem| mem.is_anything_being_dragged()) {
            show_row_ids_tooltip(
                ctx,
                blueprint,
                ui.ctx(),
                &item,
                hovered_time_range,
                num_hovered_messages,
            );
        }
    }
}

fn graph_color(ctx: &mut ViewerContext<'_>, item: &Item, ui: &mut egui::Ui) -> Color32 {
    let is_selected = ctx.selection().contains(item);
    if is_selected {
        make_brighter(ui.visuals().selection.bg_fill)
    } else {
        Color32::from_gray(225)
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

fn show_row_ids_tooltip(
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
        crate::ui::selection_panel::what_is_selected_ui(ui, ctx, &mut blueprint.viewport, item);
        ui.add_space(8.0);

        let timeline = *ctx.rec_cfg.time_ctrl.timeline();
        let query = re_arrow_store::LatestAtQuery::new(timeline, time_range.max);
        item.data_ui(ctx, ui, crate::ui::UiVerbosity::Reduced, &query);
    });
}
