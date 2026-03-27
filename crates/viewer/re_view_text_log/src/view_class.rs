use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::Color32;
use re_chunk::ChunkId;
use re_data_ui::item_ui::{self, timeline_button};
use re_log_types::{EntityPath, TimeInt, TimelineName};
use re_sdk_types::blueprint::archetypes::{TextLogColumns, TextLogFormat, TextLogRows};
use re_sdk_types::blueprint::components::{Enabled, TextLogColumn, TimelineColumn};
use re_sdk_types::blueprint::datatypes as bp_datatypes;
use re_sdk_types::components::TextLogLevel;
use re_sdk_types::{View as _, ViewClassIdentifier, datatypes};
use re_ui::list_item::LabelContent;
use re_ui::{Help, UiExt as _};
use re_viewer_context::{
    IdentifiedViewSystem as _, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewContext,
    ViewId, ViewQuery, ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError,
    ViewerContext, level_to_rich_text,
};
use re_viewport_blueprint::ViewProperty;

use super::cache::{IndexedTextLogChunk, TextLogCache, TextLogRowHandle};
use super::visualizer_system::TextLogSystem;

/// Transient state for the text-log view.
#[derive(Default)]
pub struct TextViewState {
    /// Keeps track of the latest time selection made by the user.
    ///
    /// We need this because we want the user to be able to manually scroll the
    /// text entry window however they please when the time cursor isn't moving.
    latest_time: i64,

    /// Cached set of levels exposed to the blueprint fallback provider.
    seen_levels: BTreeSet<String>,

    /// Cached per-view projection over the store-wide text-log cache.
    projection: TextLogProjectionState,
}

impl ViewState for TextViewState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Cached row projection tailored to the current view, timeline, and level filter.
#[derive(Default)]
struct TextLogProjectionState {
    timeline: Option<TimelineName>,
    included_entities: BTreeSet<EntityPath>,
    cache_revision: u64,
    chunks: BTreeMap<ChunkId, Arc<IndexedTextLogChunk>>,
    all_rows: Vec<TextLogRowHandle>,
    filtered_rows: Vec<TextLogRowHandle>,
    prefix_line_counts: Vec<u64>,
    seen_levels: BTreeSet<String>,
    active_levels: Vec<String>,
    needs_filter_refresh: bool,
}

impl TextLogProjectionState {
    /// Synchronizes the projection with the store cache while preserving append-only fast paths.
    fn refresh_from_cache(
        &mut self,
        cache: &TextLogCache,
        included_entities: &BTreeSet<EntityPath>,
        timeline: TimelineName,
    ) {
        let timeline_changed = self.timeline != Some(timeline);
        let entities_changed = self.included_entities != *included_entities;
        let additive_update = cache.is_additive_since(self.cache_revision);

        if timeline_changed || entities_changed || !additive_update {
            self.rebuild_from_cache(cache, included_entities, timeline);
            return;
        }

        if cache.revision() == self.cache_revision {
            return;
        }

        let added_chunks = cache.collect_added_chunks_since(included_entities, self.cache_revision);
        self.cache_revision = cache.revision();
        self.append_additive_chunks(added_chunks, timeline);
    }

    /// Rebuilds the projection from scratch for a new entity set, timeline, or non-additive edit.
    fn rebuild_from_cache(
        &mut self,
        cache: &TextLogCache,
        included_entities: &BTreeSet<EntityPath>,
        timeline: TimelineName,
    ) {
        self.timeline = Some(timeline);
        self.included_entities = included_entities.clone();
        self.cache_revision = cache.revision();
        self.chunks.clear();
        self.all_rows.clear();
        self.filtered_rows.clear();
        self.prefix_line_counts.clear();
        self.seen_levels.clear();

        let chunks = cache.collect_chunks_for_entities(included_entities);
        self.register_chunks(&chunks);

        for chunk in &chunks {
            chunk.append_row_handles_for_timeline(timeline, &mut self.all_rows);
        }

        self.all_rows.sort();
        self.needs_filter_refresh = true;
    }

    /// Extends the projection with newly indexed chunks while keeping stable table ordering.
    fn append_additive_chunks(
        &mut self,
        chunks: Vec<Arc<IndexedTextLogChunk>>,
        timeline: TimelineName,
    ) {
        if chunks.is_empty() {
            return;
        }

        self.register_chunks(&chunks);

        let mut added_rows = Vec::new();
        for chunk in &chunks {
            chunk.append_row_handles_for_timeline(timeline, &mut added_rows);
        }

        if added_rows.is_empty() {
            return;
        }

        added_rows.sort();

        let can_append = self
            .all_rows
            .last()
            .zip(added_rows.first())
            .is_none_or(|(existing_last, added_first)| existing_last <= added_first);

        if can_append {
            self.all_rows.extend(added_rows);
        } else {
            self.all_rows = merge_sorted_rows(std::mem::take(&mut self.all_rows), added_rows);
        }

        self.needs_filter_refresh = true;
    }

    /// Registers chunk references and level metadata needed by the current projection.
    fn register_chunks(&mut self, chunks: &[Arc<IndexedTextLogChunk>]) {
        for chunk in chunks {
            self.chunks.insert(chunk.chunk.id(), Arc::clone(chunk));

            for row_meta in &chunk.row_metas {
                if let Some(level) = &row_meta.level {
                    self.seen_levels.insert(level.to_string());
                }
            }
        }
    }

    /// Recomputes the filtered row list when the level filter or source rows change.
    fn apply_level_filter(&mut self, levels: &[TextLogLevel]) {
        let active_levels = levels
            .iter()
            .map(|level| level.as_str().to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        if !self.needs_filter_refresh && self.active_levels == active_levels {
            return;
        }

        self.active_levels = active_levels.clone();
        let active_level_set = active_levels.into_iter().collect::<BTreeSet<_>>();

        self.filtered_rows = self
            .all_rows
            .iter()
            .copied()
            .filter(|row| self.level_matches(row, &active_level_set))
            .collect();

        self.prefix_line_counts.clear();
        self.prefix_line_counts
            .reserve(self.filtered_rows.len().saturating_add(1));
        self.prefix_line_counts.push(0);

        for row in &self.filtered_rows {
            let next = self
                .prefix_line_counts
                .last()
                .copied()
                .unwrap_or_default()
                + self.row_line_count(row);
            self.prefix_line_counts.push(next);
        }

        self.needs_filter_refresh = false;
    }

    /// Looks up the cached chunk and row metadata for one filtered row.
    fn resolve_row(
        &self,
        row_nr: u64,
    ) -> Option<(&Arc<IndexedTextLogChunk>, &TextLogRowHandle, &super::cache::TextLogRowMeta)> {
        let handle = self.filtered_rows.get(row_nr as usize)?;
        let chunk = self.chunks.get(&handle.chunk_id)?;
        let row_meta = chunk.row_meta(handle.row_meta_idx)?;
        Some((chunk, handle, row_meta))
    }

    /// Computes the top offset for a logical row using cached explicit line counts.
    fn row_top_offset(&self, row_nr: u64, base_row_height: f32) -> f32 {
        let prefix = self
            .prefix_line_counts
            .get(row_nr as usize)
            .copied()
            .or_else(|| self.prefix_line_counts.last().copied())
            .unwrap_or_default();

        prefix as f32 * base_row_height
    }

    /// Returns the cached explicit line count for one row.
    fn row_line_count(&self, row: &TextLogRowHandle) -> u64 {
        self.chunks
            .get(&row.chunk_id)
            .and_then(|chunk| chunk.row_meta(row.row_meta_idx))
            .map(|row_meta| row_meta.line_count as u64)
            .unwrap_or(1)
    }

    /// Applies the level filter rule that rows without a level are always visible.
    fn level_matches(&self, row: &TextLogRowHandle, active_levels: &BTreeSet<String>) -> bool {
        self.chunks
            .get(&row.chunk_id)
            .and_then(|chunk| chunk.row_meta(row.row_meta_idx))
            .and_then(|row_meta| row_meta.level.as_ref())
            .is_none_or(|level| active_levels.contains(level.as_str()))
    }
}

/// Table column description used by the virtualized table delegate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableColumn {
    Timeline(TimelineName),
    Text(bp_datatypes::TextLogColumnKind),
}

/// Cached visible-row data resolved just for the current frame.
#[derive(Default)]
struct PreparedTextLogRow {
    body: Option<String>,
}

/// Horizontal marker placement for the current time indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CurrentTimeMarker {
    AboveRow(u64),
    BelowRow(u64),
}

/// Virtualized table delegate for the text-log view.
struct TextLogTableDelegate<'a> {
    ctx: &'a ViewerContext<'a>,
    projection: &'a TextLogProjectionState,
    visible_columns: &'a [TableColumn],
    monospace_body: bool,
    row_height: f32,
    cursor_color: Color32,
    current_time_marker: Option<CurrentTimeMarker>,
    prepared_rows: BTreeMap<u64, PreparedTextLogRow>,
}

impl TextLogTableDelegate<'_> {
    /// Resolves and caches visible body strings for the current frame.
    fn prepare_rows(&mut self, visible_rows: std::ops::Range<u64>) {
        self.prepared_rows.clear();

        if !self.visible_columns.iter().any(|column| {
            matches!(column, TableColumn::Text(bp_datatypes::TextLogColumnKind::Body))
        }) {
            return;
        }

        for row_nr in visible_rows {
            let body = self
                .projection
                .resolve_row(row_nr)
                .and_then(|(chunk, handle, _)| chunk.resolve_body(handle.row_meta_idx))
                .map(Into::into);

            self.prepared_rows.insert(row_nr, PreparedTextLogRow { body });
        }
    }

    /// Returns the body text prepared for the requested row.
    fn prepared_body(&self, row_nr: u64) -> Option<&str> {
        self.prepared_rows
            .get(&row_nr)
            .and_then(|row| row.body.as_deref())
    }
}

impl egui_table::TableDelegate for TextLogTableDelegate<'_> {
    /// Prepares only the visible body rows before cell rendering begins.
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        self.prepare_rows(info.visible_rows.clone());
    }

    /// Renders the single header row for the text-log table.
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        let Some(column) = self.visible_columns.get(cell.col_range.start) else {
            return;
        };

        match column {
            TableColumn::Timeline(timeline) => {
                timeline_button(&self.ctx.app_ctx, ui, timeline);
            }
            TableColumn::Text(kind) => {
                column_name_ui(ui, kind);
            }
        };
    }

    /// Draws the current-time indicator on the matching visible row boundary.
    fn row_ui(&mut self, ui: &mut egui::Ui, row_nr: u64) {
        let Some(marker) = self.current_time_marker else {
            return;
        };

        let y = match marker {
            CurrentTimeMarker::AboveRow(marker_row) if marker_row == row_nr => Some(ui.max_rect().top()),
            CurrentTimeMarker::BelowRow(marker_row) if marker_row == row_nr => Some(ui.max_rect().bottom()),
            _ => None,
        };

        if let Some(y) = y {
            ui.painter()
                .hline(ui.max_rect().x_range(), y, (1.0, self.cursor_color));
        }
    }

    /// Renders one visible text-log cell from cached metadata and lazily resolved bodies.
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let Some((chunk, _handle, row_meta)) = self.projection.resolve_row(cell.row_nr) else {
            return;
        };
        let Some(column) = self.visible_columns.get(cell.col_nr) else {
            return;
        };

        match column {
            TableColumn::Timeline(timeline) => {
                let row_time = row_meta
                    .timepoint
                    .get(timeline)
                    .map(TimeInt::from)
                    .unwrap_or(TimeInt::STATIC);
                item_ui::time_button(self.ctx, ui, timeline, row_time);
            }
            TableColumn::Text(bp_datatypes::TextLogColumnKind::EntityPath) => {
                item_ui::entity_path_button(
                    &self.ctx.active_recording_store_view_context(),
                    ui,
                    None,
                    chunk.entity_path(),
                );
            }
            TableColumn::Text(bp_datatypes::TextLogColumnKind::LogLevel) => {
                if let Some(level) = &row_meta.level {
                    ui.label(level_to_rich_text(ui, level));
                } else {
                    ui.label("-");
                }
            }
            TableColumn::Text(bp_datatypes::TextLogColumnKind::Body) => {
                let mut text = egui::RichText::new(self.prepared_body(cell.row_nr).unwrap_or(""));

                if self.monospace_body {
                    text = text.monospace();
                }
                if let Some(color) = row_meta.color {
                    text = text.color(color);
                }

                ui.label(text);
            }
        }
    }

    /// Uses cached prefix sums so row layout scales with visible rows instead of total history.
    fn row_top_offset(&self, _ctx: &egui::Context, _table_id: egui::Id, row_nr: u64) -> f32 {
        self.projection.row_top_offset(row_nr, self.row_height)
    }

    /// Returns the base single-line row height used by the prefix-sum offsets.
    fn default_row_height(&self) -> f32 {
        self.row_height
    }
}

#[derive(Default)]
pub struct TextView;

type ViewType = re_sdk_types::blueprint::views::TextLogView;

impl ViewClass for TextView {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "Text log"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_LOG
    }

    fn help(&self, _os: egui::os::OperatingSystem) -> Help {
        Help::new("Text log view")
            .docs_link("https://rerun.io/docs/reference/types/views/text_log_view")
            .markdown(
                "TextLog entries over time.

Filter message types and toggle column visibility in a selection panel.",
            )
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_array_fallback_provider(
            TextLogColumns::descriptor_timeline_columns().component,
            |ctx| {
                ctx.viewer_ctx()
                    .recording()
                    .timelines()
                    .keys()
                    .map(|timeline| {
                        TimelineColumn(bp_datatypes::TimelineColumn {
                            visible: true.into(),
                            timeline: timeline.as_str().into(),
                        })
                    })
                    .collect::<Vec<_>>()
            },
        );

        system_registry.register_array_fallback_provider(
            TextLogColumns::descriptor_text_log_columns().component,
            |_ctx| {
                [
                    bp_datatypes::TextLogColumnKind::EntityPath,
                    bp_datatypes::TextLogColumnKind::LogLevel,
                    bp_datatypes::TextLogColumnKind::Body,
                ]
                .map(|kind| {
                    TextLogColumn(bp_datatypes::TextLogColumn {
                        kind,
                        visible: true.into(),
                    })
                })
                .to_vec()
            },
        );

        system_registry.register_array_fallback_provider(
            TextLogRows::descriptor_filter_by_log_level().component,
            |ctx| {
                let Ok(state) = ctx.view_state().downcast_ref::<TextViewState>() else {
                    re_log::error_once!(
                        "Failed to get `TextViewState` in text log view fallback, this is a bug."
                    );
                    return Vec::new();
                };

                state
                    .seen_levels
                    .iter()
                    .map(|level| TextLogLevel(datatypes::Utf8::from(level.as_str())))
                    .collect::<Vec<_>>()
            },
        );

        system_registry.register_visualizer::<TextLogSystem>()
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<TextViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, _state: &dyn ViewState) -> Option<f32> {
        Some(2.0)
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::Low
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> ViewSpawnHeuristics {
        re_tracing::profile_function!();

        if ctx
            .indicated_entities_per_visualizer
            .get(&TextLogSystem::identifier())
            .is_some_and(|entities| entities.iter().any(include_entity))
        {
            ViewSpawnHeuristics::root()
        } else {
            ViewSpawnHeuristics::empty()
        }
    }

    fn selection_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<TextViewState>()?;

        ui.list_item_scope("text_log_selection_ui", |ui| {
            let ctx = self.view_context(ctx, view_id, state, space_origin);
            re_view::view_property_ui::<TextLogColumns>(&ctx, ui);
            view_property_ui_rows(&ctx, ui);
            re_view::view_property_ui::<TextLogFormat>(&ctx, ui);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        _missing_chunk_reporter: &re_viewer_context::MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        _system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let tokens = ui.tokens();
        let table_style = re_ui::TableStyle::Dense;
        let row_height = tokens.table_row_height(table_style);

        let state = state.downcast_mut::<TextViewState>()?;
        let columns_property = ViewProperty::from_archetype::<TextLogColumns>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let rows_property = ViewProperty::from_archetype::<TextLogRows>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );
        let format_property = ViewProperty::from_archetype::<TextLogFormat>(
            ctx.blueprint_db(),
            ctx.blueprint_query,
            query.view_id,
        );

        let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);
        let monospace_body = format_property.component_or_fallback::<Enabled>(
            &view_ctx,
            TextLogFormat::descriptor_monospace_body().component,
        )?;
        let columns = columns_property.component_array_or_fallback::<TextLogColumn>(
            &view_ctx,
            TextLogColumns::descriptor_text_log_columns().component,
        )?;
        let timeline_columns = columns_property.component_array_or_fallback::<TimelineColumn>(
            &view_ctx,
            TextLogColumns::descriptor_timeline_columns().component,
        )?;

        let included_entities = collect_included_entities(query);
        let recording_ctx = ctx.active_recording_store_view_context();
        recording_ctx.caches.entry::<TextLogCache, _>(|cache| {
            cache.ensure_initialized(recording_ctx.db);
            state
                .projection
                .refresh_from_cache(cache, &included_entities, query.timeline);
        });

        state.seen_levels = state.projection.seen_levels.clone();

        let view_ctx = self.view_context(ctx, query.view_id, state, query.space_origin);
        let levels = rows_property.component_array_or_fallback::<TextLogLevel>(
            &view_ctx,
            TextLogRows::descriptor_filter_by_log_level().component,
        )?;
        state.projection.apply_level_filter(&levels);

        let time = ctx.time_ctrl.time_i64().unwrap_or(state.latest_time);
        let time_cursor_moved = state.latest_time != time;
        let scroll_to_row = time_cursor_moved
            .then(|| scroll_target_row(&state.projection.filtered_rows, time))
            .flatten();
        let current_time_marker =
            current_time_marker(&state.projection.filtered_rows, ctx.time_ctrl.time_int());

        egui::Frame {
            inner_margin: tokens.view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            table_ui(
                ctx,
                ui,
                query.view_id,
                &state.projection,
                &timeline_columns,
                &columns,
                **monospace_body,
                row_height,
                current_time_marker,
                scroll_to_row,
            );
        });

        state.latest_time = time;

        Ok(())
    }
}

/// Builds and renders the virtualized text-log table.
#[expect(clippy::too_many_arguments)]
fn table_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    view_id: ViewId,
    projection: &TextLogProjectionState,
    timeline_columns: &[TimelineColumn],
    columns: &[TextLogColumn],
    monospace_body: bool,
    row_height: f32,
    current_time_marker: Option<CurrentTimeMarker>,
    scroll_to_row: Option<u64>,
) {
    let visible_columns = build_visible_columns(timeline_columns, columns);
    let mut table = egui_table::Table::new()
        .id_salt(egui::Id::new("__text_log__").with(view_id))
        .headers(vec![egui_table::HeaderRow::new(
            ui.tokens().table_header_height(),
        )])
        .num_rows(projection.filtered_rows.len() as u64)
        .columns(
            visible_columns
                .iter()
                .map(|column| match column {
                    TableColumn::Timeline(timeline) => {
                        egui_table::Column::new(120.0)
                            .resizable(true)
                            .id(egui::Id::new(("timeline", timeline.as_str())))
                    }
                    TableColumn::Text(bp_datatypes::TextLogColumnKind::EntityPath) => {
                        egui_table::Column::new(180.0)
                            .resizable(true)
                            .id(egui::Id::new("entity_path"))
                    }
                    TableColumn::Text(bp_datatypes::TextLogColumnKind::LogLevel) => {
                        egui_table::Column::new(90.0)
                            .resizable(true)
                            .id(egui::Id::new("log_level"))
                    }
                    TableColumn::Text(bp_datatypes::TextLogColumnKind::Body) => {
                        egui_table::Column::new(420.0)
                            .resizable(true)
                            .id(egui::Id::new("body"))
                    }
                })
                .collect::<Vec<_>>(),
        );

    if let Some(scroll_to_row) = scroll_to_row {
        table = table.scroll_to_row(scroll_to_row, Some(egui::Align::Center));
    }

    let mut table_delegate = TextLogTableDelegate {
        ctx,
        projection,
        visible_columns: &visible_columns,
        monospace_body,
        row_height,
        cursor_color: ui.tokens().strong_fg_color,
        current_time_marker,
        prepared_rows: BTreeMap::default(),
    };

    table.show(ui, &mut table_delegate);
}

/// Collects the entity set currently included by the text-log visualizer instructions.
fn collect_included_entities(query: &ViewQuery<'_>) -> BTreeSet<EntityPath> {
    query
        .iter_visualizer_instruction_for(TextLogSystem::identifier())
        .map(|(data_result, _)| data_result.entity_path.clone())
        .collect()
}

/// Expands the currently visible timeline and text columns into a single table layout.
fn build_visible_columns(
    timeline_columns: &[TimelineColumn],
    columns: &[TextLogColumn],
) -> Vec<TableColumn> {
    timeline_columns
        .iter()
        .filter(|column| *column.visible)
        .map(|column| TableColumn::Timeline(TimelineName::new(&column.timeline)))
        .chain(
            columns
                .iter()
                .filter(|column| *column.visible)
                .map(|column| TableColumn::Text(column.kind)),
        )
        .collect()
}

/// Returns the row that should be scrolled into view when the time cursor moves.
fn scroll_target_row(rows: &[TextLogRowHandle], time: i64) -> Option<u64> {
    if rows.is_empty() {
        return None;
    }

    Some(rows.partition_point(|row| row.sort_time.as_i64() < time) as u64)
}

/// Computes where the current-time indicator should be drawn for the filtered rows.
fn current_time_marker(
    rows: &[TextLogRowHandle],
    global_time: Option<TimeInt>,
) -> Option<CurrentTimeMarker> {
    let global_time = global_time?;
    let boundary = rows.partition_point(|row| row.sort_time <= global_time);

    if rows.is_empty() {
        None
    } else if boundary < rows.len() {
        Some(CurrentTimeMarker::AboveRow(boundary as u64))
    } else {
        Some(CurrentTimeMarker::BelowRow((rows.len() - 1) as u64))
    }
}

/// Merges two sorted row lists while keeping deterministic ordering for ties.
fn merge_sorted_rows(
    existing_rows: Vec<TextLogRowHandle>,
    added_rows: Vec<TextLogRowHandle>,
) -> Vec<TextLogRowHandle> {
    let mut merged_rows = Vec::with_capacity(existing_rows.len() + added_rows.len());
    let mut existing_index = 0;
    let mut added_index = 0;

    while existing_index < existing_rows.len() && added_index < added_rows.len() {
        if existing_rows[existing_index] <= added_rows[added_index] {
            merged_rows.push(existing_rows[existing_index]);
            existing_index += 1;
        } else {
            merged_rows.push(added_rows[added_index]);
            added_index += 1;
        }
    }

    merged_rows.extend(existing_rows[existing_index..].iter().copied());
    merged_rows.extend(added_rows[added_index..].iter().copied());
    merged_rows
}

/// Renders a bold column label in the text-log header row.
fn column_name_ui(ui: &mut egui::Ui, column: &bp_datatypes::TextLogColumnKind) -> egui::Response {
    ui.strong(column.name())
}

/// We need this to be a custom ui to be able to use the view state to get seen text log levels.
///
/// This could potentially be avoided if we could add component ui's from this crate.
fn view_property_ui_rows(ctx: &ViewContext<'_>, ui: &mut egui::Ui) {
    let property = ViewProperty::from_archetype::<TextLogRows>(
        ctx.blueprint_db(),
        ctx.blueprint_query(),
        ctx.view_id,
    );

    let reflection = ctx.viewer_ctx.reflection();
    let Some(reflection) = reflection.archetypes.get(&property.archetype_name) else {
        ui.error_label(format!(
            "Missing reflection data for archetype {:?}.",
            property.archetype_name
        ));
        return;
    };

    let query_ctx = property.query_context(ctx);

    let sub_prop_ui = |ui: &mut egui::Ui| {
        for field in &reflection.fields {
            if field
                .component_descriptor(property.archetype_name)
                .component
                == TextLogRows::descriptor_filter_by_log_level().component
            {
                re_view::view_property_component_ui_custom(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                    &|_| {},
                    Some(&|ui| {
                        let Ok(state) = ctx.view_state.downcast_ref::<TextViewState>() else {
                            ui.error_label("Failed to get text log view state");
                            return;
                        };

                        let Ok(levels) = property.component_array_or_fallback::<TextLogLevel>(
                            ctx,
                            TextLogRows::descriptor_filter_by_log_level().component,
                        ) else {
                            ui.error_label("Failed to query text log levels component");
                            return;
                        };

                        let mut new_levels = state
                            .seen_levels
                            .iter()
                            .map(|level| {
                                let level_active = levels.iter().any(|enabled| enabled.as_str() == level);
                                (level.clone(), level_active)
                            })
                            .chain(
                                levels
                                    .iter()
                                    .filter(|level| !state.seen_levels.contains(level.as_str()))
                                    .map(|level| (level.as_str().to_owned(), true)),
                            )
                            .collect::<Vec<_>>();

                        let mut any_change = false;
                        for (level, active) in &mut new_levels {
                            any_change |= ui
                                .re_checkbox(active, level_to_rich_text(ui, level))
                                .changed();
                        }

                        if any_change {
                            let log_levels = new_levels
                                .into_iter()
                                .filter(|(_, active)| *active)
                                .map(|(level, _)| TextLogLevel(level.into()))
                                .collect::<Vec<_>>();

                            property.save_blueprint_component(
                                ctx.viewer_ctx,
                                &TextLogRows::descriptor_filter_by_log_level(),
                                &log_levels,
                            );
                        }
                    }),
                );
            } else {
                re_view::view_property_component_ui(
                    &query_ctx,
                    ui,
                    &property,
                    field.display_name,
                    field,
                );
            }
        }
    };

    if reflection.fields.len() == 1 {
        sub_prop_ui(ui);
    } else {
        ui.list_item()
            .interactive(false)
            .show_hierarchical_with_children(
                ui,
                ui.make_persistent_id(property.archetype_name.full_name()),
                true,
                LabelContent::new(reflection.display_name),
                sub_prop_ui,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CurrentTimeMarker, TextLogProjectionState, current_time_marker, merge_sorted_rows,
        scroll_target_row,
    };
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId, Timeline};
    use re_log_types::{TimeInt, TimePoint, TimelineName};
    use re_sdk_types::archetypes::TextLog;
    use re_sdk_types::components::{Text, TextLogLevel};

    use crate::cache::{IndexedTextLogChunk, TextLogRowHandle, TextLogRowMeta};

    /// Builds a minimal indexed chunk for projection tests.
    fn indexed_chunk(
        entity_path: &str,
        row_metas: Vec<TextLogRowMeta>,
    ) -> Arc<IndexedTextLogChunk> {
        let chunk = Chunk::builder(entity_path)
            .with_component_batches(
                RowId::new(),
                [(Timeline::log_time(), 0)],
                [(
                    TextLog::descriptor_text(),
                    &[Text::from("placeholder")] as _,
                )],
            )
            .build()
            .expect("chunk should build");

        Arc::new(IndexedTextLogChunk {
            chunk: Arc::new(chunk),
            row_metas,
        })
    }

    /// Creates one cached row metadata entry for projection tests.
    fn row_meta(time: i64, line_count: u32, level: Option<&str>) -> TextLogRowMeta {
        TextLogRowMeta {
            row_idx: 0,
            instance_idx: 0,
            row_id: RowId::new(),
            timepoint: TimePoint::default().with(Timeline::log_time(), time),
            level: level.map(TextLogLevel::from),
            color: None,
            line_count,
        }
    }

    /// Verifies that level filtering keeps rows without levels and caches prefix sums.
    #[test]
    fn projection_filter_rebuilds_prefix_sums() {
        let chunk_a = indexed_chunk("logs/a", vec![row_meta(1, 1, Some(TextLogLevel::INFO))]);
        let chunk_b = indexed_chunk("logs/b", vec![row_meta(2, 3, None)]);
        let mut projection = TextLogProjectionState::default();

        projection
            .chunks
            .insert(chunk_a.chunk.id(), Arc::clone(&chunk_a));
        projection
            .chunks
            .insert(chunk_b.chunk.id(), Arc::clone(&chunk_b));
        chunk_a.append_row_handles_for_timeline(TimelineName::log_time(), &mut projection.all_rows);
        chunk_b.append_row_handles_for_timeline(TimelineName::log_time(), &mut projection.all_rows);
        projection.all_rows.sort();
        projection.needs_filter_refresh = true;

        projection.apply_level_filter(&[TextLogLevel::from(TextLogLevel::INFO)]);

        assert_eq!(projection.filtered_rows.len(), 2);
        assert_eq!(projection.prefix_line_counts, vec![0, 1, 4]);
    }

    /// Verifies that the sorted-row merge keeps deterministic ordering.
    #[test]
    fn merge_sorted_rows_preserves_order() {
        let existing_rows = vec![
            TextLogRowHandle {
                chunk_id: Chunk::builder("logs/a")
                    .with_component_batches(
                        RowId::new(),
                        [(Timeline::log_time(), 0)],
                        [(
                            TextLog::descriptor_text(),
                            &[Text::from("a")] as _,
                        )],
                    )
                    .build()
                    .expect("chunk should build")
                    .id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(1),
                row_id: RowId::new(),
                instance_idx: 0,
            },
            TextLogRowHandle {
                chunk_id: Chunk::builder("logs/b")
                    .with_component_batches(
                        RowId::new(),
                        [(Timeline::log_time(), 0)],
                        [(
                            TextLog::descriptor_text(),
                            &[Text::from("b")] as _,
                        )],
                    )
                    .build()
                    .expect("chunk should build")
                    .id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(3),
                row_id: RowId::new(),
                instance_idx: 0,
            },
        ];
        let added_rows = vec![TextLogRowHandle {
            chunk_id: Chunk::builder("logs/c")
                .with_component_batches(
                    RowId::new(),
                    [(Timeline::log_time(), 0)],
                    [(
                        TextLog::descriptor_text(),
                        &[Text::from("c")] as _,
                    )],
                )
                .build()
                .expect("chunk should build")
                .id(),
            row_meta_idx: 0,
            sort_time: TimeInt::new_temporal(2),
            row_id: RowId::new(),
            instance_idx: 0,
        }];

        let merged_rows = merge_sorted_rows(existing_rows, added_rows);

        assert_eq!(
            merged_rows
                .iter()
                .map(|row| row.sort_time.as_i64())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    /// Verifies the scroll target keeps the first row at or after the current time.
    #[test]
    fn scroll_target_uses_first_row_at_current_time() {
        let rows = vec![
            TextLogRowHandle {
                chunk_id: indexed_chunk("logs/a", vec![row_meta(1, 1, None)]).chunk.id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(10),
                row_id: RowId::new(),
                instance_idx: 0,
            },
            TextLogRowHandle {
                chunk_id: indexed_chunk("logs/b", vec![row_meta(2, 1, None)]).chunk.id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(20),
                row_id: RowId::new(),
                instance_idx: 0,
            },
            TextLogRowHandle {
                chunk_id: indexed_chunk("logs/c", vec![row_meta(3, 1, None)]).chunk.id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(20),
                row_id: RowId::new(),
                instance_idx: 0,
            },
        ];

        assert_eq!(scroll_target_row(&rows, 20), Some(1));
    }

    /// Verifies the time marker lands after the last row at or before the current time.
    #[test]
    fn current_time_marker_tracks_boundary_after_last_matching_row() {
        let rows = vec![
            TextLogRowHandle {
                chunk_id: indexed_chunk("logs/a", vec![row_meta(1, 1, None)]).chunk.id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(10),
                row_id: RowId::new(),
                instance_idx: 0,
            },
            TextLogRowHandle {
                chunk_id: indexed_chunk("logs/b", vec![row_meta(2, 1, None)]).chunk.id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(20),
                row_id: RowId::new(),
                instance_idx: 0,
            },
            TextLogRowHandle {
                chunk_id: indexed_chunk("logs/c", vec![row_meta(3, 1, None)]).chunk.id(),
                row_meta_idx: 0,
                sort_time: TimeInt::new_temporal(20),
                row_id: RowId::new(),
                instance_idx: 0,
            },
        ];

        assert_eq!(
            current_time_marker(&rows, Some(TimeInt::new_temporal(20))),
            Some(CurrentTimeMarker::BelowRow(2))
        );
        assert_eq!(
            current_time_marker(&rows, Some(TimeInt::new_temporal(15))),
            Some(CurrentTimeMarker::AboveRow(1))
        );
    }
}

#[test]
fn test_help_view() {
    re_test_context::TestContext::test_help_view(|ctx| TextView.help(ctx));
}
