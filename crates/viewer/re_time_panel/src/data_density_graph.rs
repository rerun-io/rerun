//! Show the data density over time for a data stream.
//!
//! The data density is the number of data points per unit of time.
//! We collect this into a histogram, blur it, and then paint it.

use std::ops::RangeInclusive;
use std::sync::Arc;

use egui::emath::Rangef;
use egui::{epaint::Vertex, lerp, pos2, remap, Color32, NumExt as _, Rect, Shape};

use re_chunk_store::Chunk;
use re_chunk_store::RangeQuery;
use re_data_ui::item_ui;
use re_entity_db::TimeHistogram;
use re_log_types::EntityPath;
use re_log_types::TimeInt;
use re_log_types::Timeline;
use re_log_types::{ComponentPath, ResolvedTimeRange, TimeReal};
use re_types::ComponentName;
use re_viewer_context::{Item, TimeControl, UiLayout, ViewerContext};

use crate::TimePanelItem;

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

pub struct DensityGraph {
    /// Number of datapoints per bucket.
    /// 0 == min_x, n-1 == max_x.
    buckets: Vec<f32>,
    min_x: f32,
    max_x: f32,
}

impl DensityGraph {
    pub fn new(x_range: Rangef) -> Self {
        let min_x = x_range.min - MARGIN_X;
        let max_x = x_range.max + MARGIN_X;
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
        data_density_graph_painter: &mut DataDensityGraphPainter,
        y_range: Rangef,
        painter: &egui::Painter,
        full_color: Color32,
        hovered_x_range: RangeInclusive<f32>,
    ) {
        re_tracing::profile_function!();

        let Rangef {
            min: min_y,
            max: max_y,
        } = y_range;

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

            let normalized_density = data_density_graph_painter.normalize_density(density);

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
            re_tracing::profile_scope!("triangles");
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
    re_tracing::profile_function!();

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
pub fn data_density_graph_ui2(
    data_density_graph_painter: &mut DataDensityGraphPainter,
    ctx: &ViewerContext<'_>,
    time_ctrl: &TimeControl,
    db: &re_entity_db::EntityDb,
    time_area_painter: &egui::Painter,
    ui: &egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    row_rect: Rect,
    item: &TimePanelItem,
) {
    re_tracing::profile_function!();

    let timeline = *time_ctrl.timeline();

    let mut data = build_density_graph(
        ui,
        time_ranges_ui,
        row_rect,
        db,
        item,
        timeline,
        DensityGraphBuilderConfig::default(),
    );

    data.density_graph.buckets = smooth(&data.density_graph.buckets);

    data.density_graph.paint(
        data_density_graph_painter,
        row_rect.y_range(),
        time_area_painter,
        graph_color(ctx, &item.to_item(), ui),
        // TODO(jprochazk): completely remove `hovered_x_range` and associated code from painter
        0f32..=0f32,
    );

    if let Some(hovered_time) = data.hovered_time {
        ctx.selection_state().set_hovered(item.to_item());

        if ui.ctx().dragged_id().is_none() {
            // TODO(jprochazk): check chunk.num_rows() and chunk.timeline.is_sorted()
            //                  if too many rows and unsorted, show some generic error tooltip (=too much data)
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("data_tooltip"),
                |ui| {
                    show_row_ids_tooltip2(ctx, ui, time_ctrl, db, item, hovered_time);
                },
            );
        }
    }
}

pub fn build_density_graph<'a>(
    ui: &'a egui::Ui,
    time_ranges_ui: &'a TimeRangesUi,
    row_rect: Rect,
    db: &re_entity_db::EntityDb,
    item: &TimePanelItem,
    timeline: Timeline,
    config: DensityGraphBuilderConfig,
) -> DensityGraphBuilder<'a> {
    let mut data = DensityGraphBuilder::new(ui, time_ranges_ui, row_rect);

    // Collect all relevant chunks in the visible time range.
    // We do this as a separate step so that we can also deduplicate chunks.
    let visible_time_range = time_ranges_ui
        .time_range_from_x_range((row_rect.left() - MARGIN_X)..=(row_rect.right() + MARGIN_X));

    // NOTE: These chunks are guaranteed to have data on the current timeline
    let mut chunk_ranges: Vec<(Arc<Chunk>, ResolvedTimeRange, usize)> = vec![];

    visit_relevant_chunks(
        db,
        &item.entity_path,
        item.component_name,
        timeline,
        visible_time_range,
        |chunk, time_range, num_events| {
            chunk_ranges.push((chunk, time_range, num_events));
        },
    );

    let num_chunks = chunk_ranges.len();
    for (chunk, time_range, num_events_in_chunk) in chunk_ranges {
        // Small chunk heuristics:
        // We want to render chunks as individual events, but it may be prohibitively expensive
        // for larger chunks, or if the visible time range contains many chunks.
        //
        // We split a large chunk if:
        // 1. The total number of chunks is less than some threshold
        // 2. The number of events in the chunks is less than N, where:
        //    N is relatively large for sorted chunks
        //    N is much smaller for unsorted chunks

        let fits_max_total_chunks =
            config.max_total_chunks == 0 || num_chunks < config.max_total_chunks;
        let fits_max_sorted_chunk_events = config.max_sorted_chunk_events == 0
            || (chunk.is_time_sorted() && num_events_in_chunk < config.max_sorted_chunk_events);
        let fits_max_unsorted_chunk_events = config.max_unsorted_chunk_events == 0
            || (!chunk.is_time_sorted() && num_events_in_chunk < config.max_unsorted_chunk_events);

        let render_individual_events = fits_max_total_chunks
            && (fits_max_sorted_chunk_events || fits_max_unsorted_chunk_events);

        if render_individual_events {
            for (time, num_events) in chunk.num_events_cumulative_per_unique_time(&timeline) {
                data.add_chunk_point(time, num_events as usize);
            }
        } else {
            data.add_chunk_range(time_range, num_events_in_chunk);
        }
    }

    data
}

#[derive(Clone, Copy)]
pub struct DensityGraphBuilderConfig {
    pub max_total_chunks: usize,
    pub max_unsorted_chunk_events: usize,
    pub max_sorted_chunk_events: usize,
}

impl Default for DensityGraphBuilderConfig {
    fn default() -> Self {
        Self {
            max_total_chunks: 100,
            max_unsorted_chunk_events: 5000,
            max_sorted_chunk_events: 100_000,
        }
    }
}

fn show_row_ids_tooltip2(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    time_ctrl: &TimeControl,
    db: &re_entity_db::EntityDb,
    item: &TimePanelItem,
    at_time: TimeInt,
) {
    use re_data_ui::DataUi as _;

    let ui_layout = UiLayout::Tooltip;
    let query = re_chunk_store::LatestAtQuery::new(*time_ctrl.timeline(), at_time);

    let TimePanelItem {
        entity_path,
        component_name,
    } = item;

    if let Some(component_name) = component_name {
        let component_path = ComponentPath::new(entity_path.clone(), *component_name);
        item_ui::component_path_button(ctx, ui, &component_path, db);
        ui.add_space(8.0);
        component_path.data_ui(ctx, ui, ui_layout, &query, db);
    } else {
        let instance_path = re_entity_db::InstancePath::entity_all(entity_path.clone());
        item_ui::instance_path_button(ctx, &query, db, ui, None, &instance_path);
        ui.add_space(8.0);
        instance_path.data_ui(ctx, ui, ui_layout, &query, db);
    }
}

pub struct DensityGraphBuilder<'a> {
    time_ranges_ui: &'a TimeRangesUi,
    row_rect: Rect,

    pointer_pos: Option<egui::Pos2>,
    interact_radius: f32,

    pub density_graph: DensityGraph,
    pub hovered_time: Option<TimeInt>,
}

impl<'a> DensityGraphBuilder<'a> {
    fn new(ui: &'a egui::Ui, time_ranges_ui: &'a TimeRangesUi, row_rect: Rect) -> Self {
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let interact_radius = ui.style().interaction.resize_grab_radius_side;

        Self {
            time_ranges_ui,
            row_rect,

            pointer_pos,
            interact_radius,

            density_graph: DensityGraph::new(row_rect.x_range()),
            hovered_time: None,
        }
    }

    fn add_chunk_point(&mut self, time: TimeInt, num_events: usize) {
        let Some(x) = self.time_ranges_ui.x_from_time_f32(time.into()) else {
            return;
        };

        self.density_graph.add_point(x, num_events as _);

        if let Some(pointer_pos) = self.pointer_pos {
            let is_hovered = {
                // Are we close enough to the point?
                let distance_sq = pos2(x, self.row_rect.center().y).distance_sq(pointer_pos);

                distance_sq < self.interact_radius.powi(2)
            };

            if is_hovered {
                if let Some(at_time) = self.time_ranges_ui.time_from_x_f32(pointer_pos.x) {
                    self.hovered_time = Some(at_time.round());
                }
            }
        }
    }

    fn add_chunk_range(&mut self, time_range: ResolvedTimeRange, num_events: usize) {
        if num_events == 0 {
            return;
        }

        let (Some(min_x), Some(max_x)) = (
            self.time_ranges_ui.x_from_time_f32(time_range.min().into()),
            self.time_ranges_ui.x_from_time_f32(time_range.max().into()),
        ) else {
            return;
        };

        self.density_graph
            .add_range((min_x, max_x), num_events as _);

        if let Some(pointer_pos) = self.pointer_pos {
            let is_hovered = if (max_x - min_x).abs() < 1.0 {
                // Are we close enough to center?
                let center_x = (max_x + min_x) / 2.0;
                let distance_sq = pos2(center_x, self.row_rect.center().y).distance_sq(pointer_pos);

                distance_sq < self.interact_radius.powi(2)
            } else {
                // Are we within time range rect?
                let time_range_rect = Rect {
                    min: egui::pos2(min_x, self.row_rect.min.y),
                    max: egui::pos2(max_x, self.row_rect.max.y),
                };

                time_range_rect.contains(pointer_pos)
            };

            if is_hovered {
                if let Some(at_time) = self.time_ranges_ui.time_from_x_f32(pointer_pos.x) {
                    self.hovered_time = Some(at_time.round());
                }
            }
        }
    }
}

/// This is a wrapper over `range_relevant_chunks` which also supports querying the entire entity.
/// Relevant chunks are those which:
/// - Contain data for `entity_path`
/// - Contain a `component_name` column (if provided)
/// - Have data on the given `timeline`
/// - Have data in the given `time_range`
///
/// The does not deduplicates chunks when no `component_name` is provided.
fn visit_relevant_chunks(
    db: &re_entity_db::EntityDb,
    entity_path: &EntityPath,
    component_name: Option<ComponentName>,
    timeline: Timeline,
    time_range: ResolvedTimeRange,
    mut visitor: impl FnMut(Arc<Chunk>, ResolvedTimeRange, usize),
) {
    re_tracing::profile_function!();

    let query = RangeQuery::new(timeline, time_range);

    if let Some(component_name) = component_name {
        let chunks = db
            .store()
            .range_relevant_chunks(&query, entity_path, component_name);

        for chunk in chunks {
            let Some(num_events) = chunk.num_events_for_component(component_name) else {
                continue;
            };

            let Some(chunk_timeline) = chunk.timelines().get(&timeline) else {
                continue;
            };

            visitor(Arc::clone(&chunk), chunk_timeline.time_range(), num_events);
        }
    } else if let Some(subtree) = db.tree().subtree(entity_path) {
        subtree.visit_children_recursively(&mut |entity_path, _| {
            for chunk in db
                .store()
                .range_relevant_chunks_for_all_components(&query, entity_path)
            {
                let Some(chunk_timeline) = chunk.timelines().get(&timeline) else {
                    continue;
                };

                visitor(
                    Arc::clone(&chunk),
                    chunk_timeline.time_range(),
                    chunk.num_events_cumulative(),
                );
            }
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub fn data_density_graph_ui(
    data_density_graph_painter: &mut DataDensityGraphPainter,
    ctx: &ViewerContext<'_>,
    time_ctrl: &mut TimeControl,
    db: &re_entity_db::EntityDb,
    time_area_response: &egui::Response,
    time_area_painter: &egui::Painter,
    ui: &egui::Ui,
    time_histogram: &TimeHistogram,
    row_rect: Rect,
    time_ranges_ui: &TimeRangesUi,
    item: &TimePanelItem,
) {
    re_tracing::profile_function!();

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let interact_radius_sq = ui.style().interaction.resize_grab_radius_side.powi(2);
    let center_y = row_rect.center().y;

    // Density over x-axis in UI points.
    let mut density_graph = DensityGraph::new(row_rect.x_range());

    let mut num_hovered_messages = 0;
    let mut hovered_time_range = ResolvedTimeRange::EMPTY;

    {
        let mut add_data_point = |time_range: ResolvedTimeRange, count: usize| {
            if count == 0 {
                return;
            }

            if let (Some(min_x), Some(max_x)) = (
                time_ranges_ui.x_from_time_f32(time_range.min().into()),
                time_ranges_ui.x_from_time_f32(time_range.max().into()),
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

        let visible_time_range = time_ranges_ui
            .time_range_from_x_range((row_rect.left() - MARGIN_X)..=(row_rect.right() + MARGIN_X));

        // The more zoomed out we are, the bigger chunks of time_histogram we can process at a time.
        // Larger chunks is faster.
        let chunk_size_in_ui_points = 4.0;
        let time_chunk_size =
            (chunk_size_in_ui_points / time_ranges_ui.points_per_time).round() as _;
        let ranges: Vec<_> = {
            re_tracing::profile_scope!("time_histogram.range");
            time_histogram
                .range(
                    visible_time_range.min().as_i64()..=visible_time_range.max().as_i64(),
                    time_chunk_size,
                )
                .collect()
        };

        re_tracing::profile_scope!("add_data_point");
        for (time_range, num_messages_at_time) in ranges {
            add_data_point(
                ResolvedTimeRange::new(time_range.min, time_range.max),
                num_messages_at_time as _,
            );
        }
    }

    let hovered_x_range = (time_ranges_ui
        .x_from_time_f32(hovered_time_range.min().into())
        .unwrap_or(f32::MAX)
        - MARGIN_X)
        ..=(time_ranges_ui
            .x_from_time_f32(hovered_time_range.max().into())
            .unwrap_or(f32::MIN)
            + MARGIN_X);

    density_graph.buckets = smooth(&density_graph.buckets);

    density_graph.paint(
        data_density_graph_painter,
        row_rect.y_range(),
        time_area_painter,
        graph_color(ctx, &item.to_item(), ui),
        hovered_x_range,
    );

    if 0 < num_hovered_messages {
        ctx.selection_state().set_hovered(item.to_item());

        if time_area_response.clicked_by(egui::PointerButton::Primary) {
            ctx.selection_state().set_selection(item.to_item());
            time_ctrl.set_time(hovered_time_range.min());
            time_ctrl.pause();
        } else if ui.ctx().dragged_id().is_none() && 0 < num_hovered_messages {
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("data_tooltip"),
                |ui| {
                    show_row_ids_tooltip(
                        ctx,
                        ui,
                        time_ctrl,
                        db,
                        item,
                        hovered_time_range,
                        num_hovered_messages,
                    );
                },
            );
        }
    }
}

fn graph_color(ctx: &ViewerContext<'_>, item: &Item, ui: &egui::Ui) -> Color32 {
    let is_selected = ctx.selection().contains_item(item);
    if is_selected {
        make_brighter(ui.visuals().widgets.active.fg_stroke.color)
    } else {
        //TODO(ab): tokenize that!
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
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    time_ctrl: &TimeControl,
    db: &re_entity_db::EntityDb,
    item: &TimePanelItem,
    time_range: ResolvedTimeRange,
    num_events: usize,
) {
    use re_data_ui::DataUi as _;

    if num_events == 1 {
        ui.label(format!("{num_events} event"));
    } else {
        ui.label(format!("{num_events} events"));
    }

    let ui_layout = UiLayout::Tooltip;
    let query = re_chunk_store::LatestAtQuery::new(*time_ctrl.timeline(), time_range.center());

    let TimePanelItem {
        entity_path,
        component_name,
    } = item;

    if let Some(component_name) = component_name {
        let component_path = ComponentPath::new(entity_path.clone(), *component_name);
        item_ui::component_path_button(ctx, ui, &component_path, db);
        ui.add_space(8.0);
        component_path.data_ui(ctx, ui, ui_layout, &query, db);
    } else {
        let instance_path = re_entity_db::InstancePath::entity_all(entity_path.clone());
        item_ui::instance_path_button(ctx, &query, db, ui, None, &instance_path);
        ui.add_space(8.0);
        instance_path.data_ui(ctx, ui, ui_layout, &query, db);
    }
}
