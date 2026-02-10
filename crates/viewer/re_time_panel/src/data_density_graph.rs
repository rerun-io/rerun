//! Show the data density over time for a data stream.
//!
//! The data density is the number of data points per unit of time.
//! We collect this into a histogram, blur it, and then paint it.

use std::sync::Arc;

use egui::epaint::Vertex;
use egui::{Color32, NumExt as _, Rangef, Rect, Shape, lerp, pos2, remap};
use re_chunk_store::{ChunkTrackingMode, RangeQuery};
use re_log_types::{AbsoluteTimeRange, ComponentPath, TimeInt, TimeReal, TimelineName};
use re_ui::UiExt as _;
use re_viewer_context::{Item, TimeControl, UiLayout, ViewerContext};

use super::time_ranges_ui::TimeRangesUi;
use crate::recursive_chunks_per_timeline_subscriber::PathRecursiveChunksPerTimelineStoreSubscriber;
use crate::time_panel::TimePanelItem;

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

#[derive(Clone, Copy)]
struct Bucket {
    density: f32,
    loaded: LoadState,
}

pub struct DensityGraph {
    /// Number of datapoints per bucket.
    /// `0 == min_x, n-1 == max_x`.
    buckets: Vec<Bucket>,
    min_x: f32,
    max_x: f32,
}

impl DensityGraph {
    pub fn new(x_range: Rangef) -> Self {
        let min_x = x_range.min - MARGIN_X;
        let max_x = x_range.max + MARGIN_X;
        let n = ((max_x - min_x) * DENSITIES_PER_UI_PIXEL).ceil() as usize;
        Self {
            buckets: vec![
                Bucket {
                    density: 0.0,
                    loaded: LoadState::Loaded,
                };
                n
            ],
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

    pub fn add_point(&mut self, x: f32, count: f32, loaded: LoadState) {
        debug_assert!(0.0 <= count);

        let i = self.bucket_index_from_x(x);

        // linearly interpolate where we add the count:
        let fract = i - i.floor();
        debug_assert!(0.0 <= fract && fract <= 1.0);
        let i = i.floor() as i64;

        if let Ok(i) = usize::try_from(i)
            && let Some(bucket) = self.buckets.get_mut(i)
        {
            bucket.density += (1.0 - fract) * count;
            bucket.loaded = bucket.loaded.and(loaded);
        }
        if let Ok(i) = usize::try_from(i + 1)
            && let Some(bucket) = self.buckets.get_mut(i)
        {
            bucket.density += fract * count;
            bucket.loaded = bucket.loaded.and(loaded);
        }
    }

    pub fn add_range(&mut self, (min_x, max_x): (f32, f32), count: f32, loaded: LoadState) {
        #![expect(clippy::cast_possible_wrap)] // usize -> i64 is fine

        debug_assert!(min_x <= max_x);

        if max_x < self.min_x || self.max_x < min_x {
            return;
        }

        if min_x == max_x {
            let center_x = lerp(min_x..=max_x, 0.5);
            self.add_point(center_x, count, loaded);
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
        if let Ok(i) = usize::try_from(first_bucket as i64)
            && let Some(bucket) = self.buckets.get_mut(i)
        {
            bucket.density += first_bucket_factor * count_per_bucket;
            bucket.loaded = bucket.loaded.and(loaded);
        }

        // full buckets:
        if min_full_bucket != max_full_bucket {
            let min_full_bucket_idx =
                (min_full_bucket as i64).clamp(0, self.buckets.len() as i64 - 1) as usize;
            let max_full_bucket_idx =
                (max_full_bucket as i64).clamp(0, self.buckets.len() as i64 - 1) as usize;
            for bucket in &mut self.buckets[min_full_bucket_idx..=max_full_bucket_idx] {
                bucket.density += count_per_bucket;
                bucket.loaded = bucket.loaded.and(loaded);
            }
        }

        // last bucket, partially filled:
        if let Ok(i) = usize::try_from(last_bucket as i64)
            && let Some(bucket) = self.buckets.get_mut(i)
        {
            bucket.density += last_bucket_factor * count_per_bucket;
            bucket.loaded = bucket.loaded.and(loaded);
        }
    }

    pub fn paint(
        &self,
        data_density_graph_painter: &mut DataDensityGraphPainter,
        y_range: Rangef,
        painter: &egui::Painter,
        loaded_color: Color32,
        unloaded_color: Color32,
    ) {
        re_tracing::profile_function!();

        let Rangef {
            min: min_y,
            max: max_y,
        } = y_range;

        let center_y = fast_midpoint(min_y, max_y);
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

        for (i, bucket) in self.buckets.iter().enumerate() {
            // TODO(emilk): early-out if density is 0 for long stretches

            let x = self.x_from_bucket_index(i);

            let normalized_density = data_density_graph_painter.normalize_density(bucket.density);

            let (inner_radius, inner_color) = if normalized_density == 0.0 {
                (0.0, Color32::TRANSPARENT)
            } else {
                // Make sure we see small things even when they are dwarfed
                // by the max due to the normalization:
                const MIN_RADIUS: f32 = 1.5;
                let inner_radius =
                    (max_radius * normalized_density).at_least(MIN_RADIUS) - feather_radius;

                let color = match bucket.loaded {
                    LoadState::Loaded => loaded_color,
                    LoadState::Unloaded => unloaded_color,
                };

                // Color different if we're outside of a segment.
                let inner_color = color.gamma_multiply(lerp(0.5..=1.0, normalized_density));

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

/// This is faster than `f32::midpoint`, but less accurate.
#[inline(always)]
fn fast_midpoint(min_y: f32, max_y: f32) -> f32 {
    0.5 * (min_y + max_y)
}

// ----------------------------------------------------------------------------

/// Blur the input slightly.
fn smooth(buckets: &[Bucket]) -> Vec<Bucket> {
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

    (0..buckets.len())
        .map(|i| {
            let mut sum = 0.0;
            let mut loaded = LoadState::Loaded;
            for (j, &k) in kernel.iter().enumerate() {
                if let Some(bucket) = buckets.get((i + j).saturating_sub(2)) {
                    debug_assert!(bucket.density >= 0.0);
                    sum += k * bucket.density;
                    loaded = loaded.and(bucket.loaded);
                }
            }
            debug_assert!(sum.is_finite() && 0.0 <= sum);

            Bucket {
                density: sum,
                loaded,
            }
        })
        .collect()
}

// ----------------------------------------------------------------------------

/// Paints a one point thick line in the given range, indicating which sections
/// on the time panel have only loaded chunks.
///
/// If the time cursor is over unloaded chunks, this paints a dashed line as a
/// loading indicator.
///
/// `paint_fully_loaded_ranges` indicates if fully loaded ranges from the rrd
/// manifest should be filled in.
pub fn paint_loaded_indicator_bar(
    ui: &egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    db: &re_entity_db::EntityDb,
    time_ctrl: &TimeControl,
    y: f32,
    full_x_range: Rangef,
    paint_fully_loaded_ranges: bool,
) {
    let Some(timeline) = time_ctrl.timeline() else {
        return;
    };

    re_tracing::profile_function!();

    let full_time_range = db
        .rrd_manifest_index()
        .timeline_range(time_ctrl.timeline_name())
        .unwrap_or(AbsoluteTimeRange::EMPTY);

    let is_loading = db.can_fetch_chunks_from_redap()
        && db
            .rrd_manifest_index()
            .chunk_prioritizer()
            .any_missing_chunks();

    if is_loading
        && let Some(start) = time_ranges_ui.x_from_time(full_time_range.min.into())
        && let Some(end) = time_ranges_ui.x_from_time(full_time_range.max.into())
    {
        re_tracing::profile_scope!("draw loading");
        // How many points the gap is in the dashed line
        let gap = 5.0;
        // How many points each line is in the dashed line
        let line = 3.0;
        // Animation speed of the loading in points per second
        let speed = 20.0;

        let x_range = full_x_range.intersection(Rangef::new(start as f32, end as f32));

        if x_range.span() > 0.0 {
            let dashed_line = egui::Shape::dashed_line_with_offset(
                &[egui::pos2(x_range.min, y), egui::pos2(x_range.max, y)],
                ui.visuals().widgets.noninteractive.fg_stroke,
                &[line],
                &[gap],
                ui.input(|i| (i.time * speed) % (gap as f64 + line as f64) - line as f64) as f32,
            );

            ui.painter()
                // Need to clip because offsetting the dashed line may end up outside otherwise
                .with_clip_rect(egui::Rect::from_x_y_ranges(x_range, Rangef::EVERYTHING))
                .add(dashed_line);
        }
    }

    if paint_fully_loaded_ranges {
        let loaded_ranges_on_timeline = db
            .rrd_manifest_index()
            .loaded_ranges_on_timeline(timeline.name());

        for range in loaded_ranges_on_timeline {
            let Some(start) = time_ranges_ui.x_from_time(range.min.into()) else {
                continue;
            };
            let Some(end) = time_ranges_ui.x_from_time(range.max.into()) else {
                continue;
            };
            debug_assert!(start <= end, "Negative x-range");
            let x = Rangef::new(start as f32, end as f32).intersection(full_x_range);

            if x.span() <= 0.0 {
                continue;
            }

            ui.painter()
                .hline(x, y, ui.visuals().widgets.noninteractive.fg_stroke);
        }
    }
}

/// Returns the hovered time, if any.
#[expect(clippy::too_many_arguments)]
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
) -> Option<TimeInt> {
    re_tracing::profile_function!();

    let num_missing_chunk_ids_before = db.storage_engine().store().num_missing_chunk_ids();

    let mut data = build_density_graph(
        ui,
        time_ranges_ui,
        row_rect,
        db,
        item,
        time_ctrl.timeline()?.name(),
        DensityGraphBuilderConfig::default(),
    );

    debug_assert_eq!(
        num_missing_chunk_ids_before,
        db.storage_engine().store().num_missing_chunk_ids(),
        "DEBUG ASSERT: The density graph should not request new chunks. (This assert assumes single-threaded access to the store)."
    );

    data.density_graph.buckets = smooth(&data.density_graph.buckets);

    data.density_graph.paint(
        data_density_graph_painter,
        row_rect.y_range(),
        time_area_painter,
        graph_color(ctx, &item.to_item(), ui),
        ui.tokens().density_graph_outside_valid_ranges,
    );

    if let Some(pointer) = data.hovered_pos {
        time_ranges_ui
            .snapped_time_from_x(ui, pointer.x)
            .map(|t| t.round())
    } else {
        data.hovered_time.map(|t| t.round())
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

    {
        re_tracing::profile_scope!("unloaded chunks");
        let entries = db.rrd_manifest_index().unloaded_temporal_entries_for(
            timeline,
            &item.entity_path,
            item.component,
        );

        re_tracing::profile_scope!("add_chunk_range");
        for entry in entries {
            data.add_chunk_range(entry.time_range, entry.num_rows, LoadState::Unloaded);
        }
    }

    // NOTE: These chunks are guaranteed to have data on the current timeline
    let (chunk_ranges, total_events): (
        Vec<(Arc<re_chunk_store::Chunk>, AbsoluteTimeRange, u64)>,
        u64,
    ) = {
        re_tracing::profile_scope!("collect chunks");

        let engine = db.storage_engine();
        let store = engine.store();
        let query = RangeQuery::new(*timeline, visible_time_range);

        if let Some(component) = item.component {
            let mut total_num_events = 0;
            (
                store
                    .range_relevant_chunks(
                        // Don't cause chunks to be downloaded just to show the density graph
                        ChunkTrackingMode::Ignore,
                        &query,
                        &item.entity_path,
                        component,
                    )
                    // TODO(RR-3295): what should we do with virtual chunks here?
                    .into_iter_verbose()
                    .filter_map(|chunk| {
                        let time_range = chunk.timelines().get(timeline)?.time_range();
                        chunk.num_events_for_component(component).map(|num_events| {
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
                    data.add_chunk_point(time, num_events as usize, LoadState::Loaded);
                }
            } else {
                data.add_chunk_range(time_range, num_events_in_chunk, LoadState::Loaded);
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

pub fn show_row_ids_tooltip(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    time_ctrl: &TimeControl,
    db: &re_entity_db::EntityDb,
    item: &TimePanelItem,
    at_time: TimeInt,
) {
    use re_data_ui::DataUi as _;

    let ui_layout = UiLayout::Tooltip;
    let query = re_chunk_store::LatestAtQuery::new(*time_ctrl.timeline_name(), at_time);

    let TimePanelItem {
        entity_path,
        component,
    } = item;

    if let Some(component) = *component {
        ComponentPath::new(entity_path.clone(), component).data_ui(ctx, ui, ui_layout, &query, db);
    } else {
        re_entity_db::InstancePath::entity_all(entity_path.clone())
            .data_ui(ctx, ui, ui_layout, &query, db);
    }
}

#[derive(Clone, Copy)]
pub enum LoadState {
    Loaded,
    Unloaded,
}

impl LoadState {
    fn and(&self, other: Self) -> Self {
        match (self, other) {
            (Self::Loaded, Self::Loaded) => Self::Loaded,
            _ => Self::Unloaded,
        }
    }
}

pub struct DensityGraphBuilder<'a> {
    time_ranges_ui: &'a TimeRangesUi,
    row_rect: Rect,

    pointer_pos: Option<egui::Pos2>,

    pub density_graph: DensityGraph,

    closest_event_x_distance: f32,
    pub hovered_time: Option<TimeReal>,
    pub hovered_pos: Option<egui::Pos2>, // needed so we can do late-snapping
}

impl<'a> DensityGraphBuilder<'a> {
    fn new(ui: &'a egui::Ui, time_ranges_ui: &'a TimeRangesUi, row_rect: Rect) -> Self {
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let interact_radius = ui.style().interaction.interact_radius;

        Self {
            time_ranges_ui,
            row_rect,

            pointer_pos,

            density_graph: DensityGraph::new(row_rect.x_range()),

            closest_event_x_distance: interact_radius,
            hovered_time: None,
            hovered_pos: None,
        }
    }

    fn add_chunk_point(&mut self, time: TimeInt, num_events: usize, loaded: LoadState) {
        let Some(x) = self.time_ranges_ui.x_from_time_f32(time.into()) else {
            return;
        };

        self.density_graph.add_point(x, num_events as _, loaded);

        if let Some(pointer_pos) = self.pointer_pos
            && self.row_rect.y_range().contains(pointer_pos.y)
        {
            let x_dist = (x - pointer_pos.x).abs();

            if x_dist < self.closest_event_x_distance {
                self.closest_event_x_distance = x_dist;
                self.hovered_time = Some(time.into());
                self.hovered_pos = None;
            }
        }
    }

    fn add_chunk_range(
        &mut self,
        time_range: AbsoluteTimeRange,
        num_events: u64,
        loaded: LoadState,
    ) {
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
            .add_range((min_x, max_x), num_events as _, loaded);

        if let Some(pointer_pos) = self.pointer_pos
            && self.row_rect.y_range().contains(pointer_pos.y)
        {
            let very_thin_range = (max_x - min_x).abs() < 1.0;
            if very_thin_range {
                // Are we close enough to center?
                let center_x = fast_midpoint(max_x, min_x);
                let x_dist = (center_x - pointer_pos.x).abs();

                if x_dist < self.closest_event_x_distance {
                    self.closest_event_x_distance = x_dist;
                    self.hovered_pos = Some(pointer_pos);
                    self.hovered_time = None;
                }
            } else if (min_x..=max_x).contains(&pointer_pos.x) {
                self.closest_event_x_distance = 0.0;
                self.hovered_pos = Some(pointer_pos);
                self.hovered_time = None;
            }
        }
    }
}

fn graph_color(ctx: &ViewerContext<'_>, item: &Item, ui: &egui::Ui) -> Color32 {
    let is_selected = ctx.selection().contains_item(item);

    if is_selected {
        ui.tokens().density_graph_selected
    } else {
        ui.tokens().density_graph_unselected
    }
}
