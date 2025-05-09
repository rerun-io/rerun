//! Show the data density over time for a data stream.
//!
//! The data density is the number of data points per unit of time.
//! We collect this into a histogram, blur it, and then paint it.

use std::sync::Arc;

use egui::emath::Rangef;
use egui::{Color32, NumExt as _, Rect, Shape, Tooltip, epaint::Vertex, lerp, pos2, remap};

use re_chunk_store::Chunk;
use re_chunk_store::RangeQuery;
use re_log_types::{ComponentPath, ResolvedTimeRange, TimeInt, TimelineName};
use re_viewer_context::{Item, TimeControl, UiLayout, ViewerContext};

use crate::recursive_chunks_per_timeline_subscriber::PathRecursiveChunksPerTimelineStoreSubscriber;
use crate::time_panel::TimePanelItem;

use super::time_ranges_ui::TimeRangesUi;

// ----------------------------------------------------------------------------

/// We need some margin because of the blurring.
const MARGIN_X: f32 = 2.0;

/// Higher = slower, but more accurate.
const DENSITIES_PER_UI_PIXEL: f32 = 1.0;

const DEBUG_PAINT: bool = false;

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
    /// `0 == min_x, n-1 == max_x`.
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

        if max_x < self.min_x || self.max_x < min_x {
            return;
        }

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

        let min_full_bucket = min_bucket.ceil();
        let first_bucket = min_bucket.floor();
        let max_full_bucket = max_bucket.floor();
        let last_bucket = max_bucket.ceil();
        let first_bucket_factor = 1.0 - (min_bucket - first_bucket);
        let num_full_buckets = 1.0 + max_full_bucket - min_full_bucket;
        let last_bucket_factor = 1.0 - (last_bucket - max_bucket);
        let count_per_bucket =
            count / (first_bucket_factor + num_full_buckets + last_bucket_factor);

        // For filling self.buckets, we need to account for min_bucket/max_bucket being out of range!
        // (everything before & beyond can be seen as a "virtual" bucket that we can't fill)

        // first bucket, partially filled:
        if let Ok(i) = usize::try_from(first_bucket as i64) {
            if let Some(bucket) = self.buckets.get_mut(i) {
                *bucket += first_bucket_factor * count_per_bucket;
            }
        }

        // full buckets:
        if min_full_bucket != max_full_bucket {
            let min_full_bucket_idx =
                (min_full_bucket as i64).clamp(0, self.buckets.len() as i64 - 1) as usize;
            let max_full_bucket_idx =
                (max_full_bucket as i64).clamp(0, self.buckets.len() as i64 - 1) as usize;
            for bucket in &mut self.buckets[min_full_bucket_idx..=max_full_bucket_idx] {
                *bucket += count_per_bucket;
            }
        }

        // last bucket, partially filled:
        if let Ok(i) = usize::try_from(last_bucket as i64) {
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

                let inner_color = full_color.gamma_multiply(lerp(0.5..=1.0, normalized_density));

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

        painter.add(Shape::Mesh(Arc::new(mesh)));
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
pub fn data_density_graph_ui(
    data_density_graph_painter: &mut DataDensityGraphPainter,
    ctx: &ViewerContext<'_>,
    time_ctrl: &TimeControl,
    db: &re_entity_db::EntityDb,
    time_area_painter: &egui::Painter,
    ui: &egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    row_rect: Rect,
    item: &TimePanelItem,
    tooltips_enabled: bool,
) {
    re_tracing::profile_function!();

    let timeline = *time_ctrl.timeline();

    let mut data = build_density_graph(
        ui,
        time_ranges_ui,
        row_rect,
        db,
        item,
        timeline.name(),
        DensityGraphBuilderConfig::default(),
    );

    data.density_graph.buckets = smooth(&data.density_graph.buckets);

    data.density_graph.paint(
        data_density_graph_painter,
        row_rect.y_range(),
        time_area_painter,
        graph_color(ctx, &item.to_item(), ui),
    );

    if tooltips_enabled {
        if let Some(hovered_time) = data.hovered_time {
            ctx.selection_state().set_hovered(item.to_item());

            if ui.ctx().dragged_id().is_none() {
                // TODO(jprochazk): check chunk.num_rows() and chunk.timeline.is_sorted()
                //                  if too many rows and unsorted, show some generic error tooltip (=too much data)
                Tooltip::new(
                    egui::Id::new("data_tooltip"),
                    ui.ctx().clone(),
                    egui::PopupAnchor::Pointer,
                    ui.layer_id(),
                )
                .gap(12.0)
                .show(|ui| {
                    show_row_ids_tooltip(ctx, ui, time_ctrl, db, item, hovered_time);
                });
            }
        }
    }
}

pub fn build_density_graph<'a>(
    ui: &'a egui::Ui,
    time_ranges_ui: &'a TimeRangesUi,
    row_rect: Rect,
    db: &re_entity_db::EntityDb,
    item: &TimePanelItem,
    timeline: &TimelineName,
    config: DensityGraphBuilderConfig,
) -> DensityGraphBuilder<'a> {
    re_tracing::profile_function!();

    let mut data = DensityGraphBuilder::new(ui, time_ranges_ui, row_rect);

    // Collect all relevant chunks in the visible time range.
    // We do this as a separate step so that we can also deduplicate chunks.
    let visible_time_range = time_ranges_ui
        .time_range_from_x_range((row_rect.left() - MARGIN_X)..=(row_rect.right() + MARGIN_X));

    // NOTE: These chunks are guaranteed to have data on the current timeline
    let (chunk_ranges, total_events): (Vec<(Arc<Chunk>, ResolvedTimeRange, u64)>, u64) = {
        re_tracing::profile_scope!("collect chunks");

        let engine = db.storage_engine();
        let store = engine.store();
        let query = RangeQuery::new(*timeline, visible_time_range);

        if let Some(component_descr) = item.component_descr.as_ref() {
            let mut total_num_events = 0;
            (
                store
                    .range_relevant_chunks(&query, &item.entity_path, component_descr)
                    .into_iter()
                    .filter_map(|chunk| {
                        let time_range = chunk.timelines().get(timeline)?.time_range();
                        chunk
                            .num_events_for_component(component_descr)
                            .map(|num_events| {
                                total_num_events += num_events;
                                (chunk, time_range, num_events)
                            })
                    })
                    .collect(),
                total_num_events,
            )
        } else {
            PathRecursiveChunksPerTimelineStoreSubscriber::access(
                &store.id(),
                |chunks_per_timeline| {
                    let Some(info) = chunks_per_timeline
                        .path_recursive_chunks_for_entity_and_timeline(&item.entity_path, timeline)
                    else {
                        return Default::default();
                    };

                    (
                        info.recursive_chunks_info
                            .values()
                            .map(|info| {
                                (
                                    info.chunk.clone(),
                                    info.resolved_time_range,
                                    info.num_events,
                                )
                            })
                            .collect(),
                        info.total_num_events,
                    )
                },
            )
            .unwrap_or_default()
        }
    };

    // Small chunk heuristics:
    // We want to render chunks as individual events, but it may be prohibitively expensive
    // for larger chunks, or if the visible time range contains many chunks.
    //
    // We split a large chunk if:
    // 1. The total number of events is less than some threshold
    // 2. The number of events in the chunks is less than N, where:
    //    N is relatively large for sorted chunks
    //    N is much smaller for unsorted chunks

    {
        re_tracing::profile_scope!("add_data");

        let can_render_individual_events = total_events < config.max_total_chunk_events;

        if DEBUG_PAINT {
            ui.ctx().debug_painter().debug_rect(
                row_rect,
                egui::Color32::LIGHT_BLUE,
                format!(
                    "{} chunks, {total_events} events, render individual: {can_render_individual_events}",
                    chunk_ranges.len()
                ),
            );
        }

        for (chunk, time_range, num_events_in_chunk) in chunk_ranges {
            let should_render_individual_events = can_render_individual_events
                && if chunk.is_timeline_sorted(timeline) {
                    num_events_in_chunk < config.max_events_in_sorted_chunk
                } else {
                    num_events_in_chunk < config.max_events_in_unsorted_chunk
                };

            if should_render_individual_events {
                for (time, num_events) in chunk.num_events_cumulative_per_unique_time(timeline) {
                    data.add_chunk_point(time, num_events as usize);
                }
            } else {
                data.add_chunk_range(time_range, num_events_in_chunk);
            }
        }
    }

    data
}

#[derive(Clone, Copy)]
pub struct DensityGraphBuilderConfig {
    /// If there are more chunks than this then we NEVER show individual events of any chunk.
    pub max_total_chunk_events: u64,

    /// If a sorted chunk has fewer events than this we show its individual events.
    pub max_events_in_sorted_chunk: u64,

    /// If an unsorted chunk has fewer events than this we show its individual events.
    pub max_events_in_unsorted_chunk: u64,
}

impl DensityGraphBuilderConfig {
    /// All chunks will be rendered whole.
    pub const NEVER_SHOW_INDIVIDUAL_EVENTS: Self = Self {
        max_total_chunk_events: 0,
        max_events_in_unsorted_chunk: 0,
        max_events_in_sorted_chunk: 0,
    };

    /// All sorted chunks will be rendered as individual events,
    /// and all unsorted chunks will be rendered whole.
    pub const ALWAYS_SPLIT_SORTED_CHUNKS: Self = Self {
        max_total_chunk_events: u64::MAX,
        max_events_in_unsorted_chunk: 0,
        max_events_in_sorted_chunk: u64::MAX,
    };

    /// All chunks will be rendered as individual events.
    pub const ALWAYS_SPLIT_ALL_CHUNKS: Self = Self {
        max_total_chunk_events: u64::MAX,
        max_events_in_unsorted_chunk: u64::MAX,
        max_events_in_sorted_chunk: u64::MAX,
    };
}

impl Default for DensityGraphBuilderConfig {
    fn default() -> Self {
        Self {
            // This is an arbitrary threshold meant to ensure that building a data density graph never takes too long.
            //
            // Our very basic benchmarks suggest that at 100k sorted events the graph building takes on average 1.5ms,
            // measured on a high-end x86_64 CPU from 2022 (Ryzen 9 7950x).
            // It does not seem to matter how many chunks there are, only how many total events we're showing.
            //
            // We want to stay around 1ms if possible, preferring to instead spend our frame budget on actually
            // visualizing the data, and we also want to support multiple data density graphs on the screen at once.
            max_total_chunk_events: 10_000,

            // For individual chunks, the limits are completely arbitrary, and help preserve visual clarity of the data
            // when there are too many events in a given chunk.
            max_events_in_sorted_chunk: 10_000,

            // Processing unsorted events is about 20% slower than sorted events.
            max_events_in_unsorted_chunk: 8_000,
        }
    }
}

fn show_row_ids_tooltip(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    time_ctrl: &TimeControl,
    db: &re_entity_db::EntityDb,
    item: &TimePanelItem,
    at_time: TimeInt,
) {
    use re_data_ui::DataUi as _;

    let ui_layout = UiLayout::Tooltip;
    let query = re_chunk_store::LatestAtQuery::new(*time_ctrl.timeline().name(), at_time);

    let TimePanelItem {
        entity_path,
        component_descr,
    } = item;

    if let Some(component_descr) = component_descr.as_ref() {
        ComponentPath::new(entity_path.clone(), component_descr.clone())
            .data_ui(ctx, ui, ui_layout, &query, db);
    } else {
        re_entity_db::InstancePath::entity_all(entity_path.clone())
            .data_ui(ctx, ui, ui_layout, &query, db);
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
                self.hovered_time = Some(time);
            }
        }
    }

    fn add_chunk_range(&mut self, time_range: ResolvedTimeRange, num_events: u64) {
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
