use std::collections::BTreeMap;
use std::sync::Arc;

use egui_extras::{Column, TableRow};
use itertools::{Either, Itertools as _};
use re_chunk_store::{ChunkStore, ChunkTrackingMode, LatestAtQuery, RangeQuery};
use re_log_types::{
    AbsoluteTimeRange, StoreKind, TimeType, Timeline, TimelineName, TimestampFormat,
};
use re_ui::{UiExt as _, list_item};
use re_viewer_context::StoreContext;

use crate::chunk_list_mode::{ChunkListMode, ChunkListQueryMode};
use crate::chunk_ui::ChunkUi;
use crate::sort::{SortColumn, SortDirection, sortable_column_header_ui};

/// Any column that can be sorted.
#[derive(Default, Clone, Copy, PartialEq)]
enum ChunkListColumn {
    #[default]
    ChunkId,
    EntityPath,
    RowCount,
    Timeline(TimelineName),
}

type ChunkListSortColumn = SortColumn<ChunkListColumn>;

impl ChunkListColumn {
    pub(crate) fn ui(&self, ui: &mut egui::Ui, sort_column: &mut ChunkListSortColumn) {
        match self {
            Self::ChunkId => sortable_column_header_ui(self, ui, sort_column, "Chunk ID"),
            Self::EntityPath => sortable_column_header_ui(self, ui, sort_column, "Entity"),
            Self::RowCount => sortable_column_header_ui(self, ui, sort_column, "# rows"),
            Self::Timeline(timeline_name) => {
                sortable_column_header_ui(self, ui, sort_column, timeline_name.as_str());
            }
        }
    }
}

/// Browser UI for [`re_chunk_store::ChunkStore`].
pub struct DatastoreUi {
    store_kind: StoreKind,
    focused_chunk: Option<ChunkUi>,

    chunk_list_mode: ChunkListMode,

    chunk_list_sort_column: ChunkListSortColumn,

    // filters
    entity_path_filter: String,
    component_filter: String,
}

impl Default for DatastoreUi {
    fn default() -> Self {
        Self {
            store_kind: StoreKind::Recording,
            focused_chunk: None,
            chunk_list_mode: ChunkListMode::default(),
            chunk_list_sort_column: ChunkListSortColumn::default(),
            entity_path_filter: String::new(),
            component_filter: String::new(),
        }
    }
}

impl DatastoreUi {
    /// Show the ui.
    ///
    /// Returns `false` if the datastore UI should be closed (e.g., the close button was clicked),
    /// or `true` if the datastore UI should remain open.
    pub fn ui(
        &mut self,
        ctx: &StoreContext<'_>,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
    ) -> bool {
        let mut datastore_ui_active = true;

        egui::Frame {
            inner_margin: egui::Margin::same(5),
            ..Default::default()
        }
        .show(ui, |ui| {
            let exit_focused_chunk = if let Some(focused_chunk) = &mut self.focused_chunk {
                focused_chunk.ui(
                    ui,
                    timestamp_format,
                    match self.store_kind {
                        StoreKind::Recording => ctx.recording.storage_engine(),
                        StoreKind::Blueprint => ctx.blueprint.storage_engine(),
                    }
                    .store(),
                )
            } else {
                self.chunk_store_ui(
                    ui,
                    match self.store_kind {
                        StoreKind::Recording => ctx.recording.storage_engine(),
                        StoreKind::Blueprint => ctx.blueprint.storage_engine(),
                    }
                    .store(),
                    &mut datastore_ui_active,
                    timestamp_format,
                );

                false
            };

            if exit_focused_chunk {
                self.focused_chunk = None;
            }
        });

        datastore_ui_active
    }

    fn chunk_store_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        datastore_ui_active: &mut bool,
        timestamp_format: TimestampFormat,
    ) {
        let tokens = ui.tokens();

        let should_copy_chunk = self.chunk_store_info_ui(ui, chunk_store, datastore_ui_active);

        // Each of these must be a column that contains the corresponding time range.
        let all_timelines = chunk_store.timelines();

        self.chunk_list_mode.ui(ui, chunk_store, timestamp_format);

        let table_style = re_ui::TableStyle::Dense;

        //
        // Collect chunks based on query mode
        //

        let chunk_iterator = match &self.chunk_list_mode {
            ChunkListMode::All => Either::Left(chunk_store.iter_physical_chunks().map(Arc::clone)),
            ChunkListMode::Query {
                timeline,
                entity_path,
                component,
                query: ChunkListQueryMode::LatestAt(at),
                ..
            } => Either::Right(
                chunk_store
                    .latest_at_relevant_chunks(
                        ChunkTrackingMode::Report,
                        &LatestAtQuery::new(*timeline.name(), *at),
                        entity_path,
                        *component,
                    )
                    .into_iter_verbose(),
            ),
            ChunkListMode::Query {
                timeline,
                entity_path,
                component,
                query: ChunkListQueryMode::Range(range),
                ..
            } => Either::Right(
                chunk_store
                    .range_relevant_chunks(
                        ChunkTrackingMode::Report,
                        &RangeQuery::new(*timeline.name(), *range),
                        entity_path,
                        *component,
                    )
                    .into_iter_verbose(),
            ),
        };

        //
        // Filters
        //

        ui.horizontal(|ui| {
            ui.spacing_mut().text_edit_width = 120.0;

            ui.label("Filter:").on_hover_text(
                "Filter the chunk list by entity path and/or component. Filtering is \
                case-insensitive text-based.",
            );
            ui.label("entity:");
            ui.text_edit_singleline(&mut self.entity_path_filter);

            ui.label("component:");
            ui.text_edit_singleline(&mut self.component_filter);

            if ui
                .small_icon_button(&re_ui::icons::CLOSE, "Close")
                .clicked()
            {
                self.entity_path_filter = String::new();
                self.component_filter = String::new();
            }
        });

        let chunk_iterator = if self.entity_path_filter.is_empty() {
            Either::Left(chunk_iterator)
        } else {
            let entity_path_filter = self.entity_path_filter.to_lowercase();
            Either::Right(chunk_iterator.filter(move |chunk| {
                chunk
                    .entity_path()
                    .to_string()
                    .to_lowercase()
                    .contains(&entity_path_filter)
            }))
        };

        let chunk_iterator = if self.component_filter.is_empty() {
            Either::Left(chunk_iterator)
        } else {
            let component_filter = self.component_filter.to_lowercase();
            Either::Right(chunk_iterator.filter(move |chunk| {
                chunk
                    .components()
                    .keys()
                    .any(|name| name.as_str().to_lowercase().contains(&component_filter))
            }))
        };

        let mut chunks = chunk_iterator.collect_vec();

        //
        // Sort
        //

        match &self.chunk_list_sort_column.column {
            ChunkListColumn::ChunkId => {} // already sorted by row IDs
            ChunkListColumn::EntityPath => {
                chunks.sort_by_key(|chunk| chunk.entity_path().to_string());
            }
            ChunkListColumn::RowCount => chunks.sort_by_key(|chunk| chunk.num_rows()),
            ChunkListColumn::Timeline(timeline_name) => chunks.sort_by_key(|chunk| {
                chunk
                    .timelines()
                    .iter()
                    .find(|(timeline, _)| *timeline == timeline_name)
                    .map_or(re_log_types::TimeInt::MIN, |(_, time_column)| {
                        time_column.time_range().min()
                    })
            }),
        }

        if self.chunk_list_sort_column.direction == SortDirection::Descending {
            chunks.reverse();
        }

        //
        // Copy to clipboard
        //

        if should_copy_chunk {
            //TODO(#7282): make sure the output is not dependant on the parent terminal's width
            let s = chunks.iter().map(|chunk| chunk.to_string()).join("\n\n");
            ui.ctx().copy_text(s);
        }

        //
        // Table
        //

        let header_ui = |mut row: TableRow<'_, '_>| {
            row.col(|ui| {
                ChunkListColumn::ChunkId.ui(ui, &mut self.chunk_list_sort_column);
            });

            row.col(|ui| {
                ChunkListColumn::EntityPath.ui(ui, &mut self.chunk_list_sort_column);
            });

            row.col(|ui| {
                ChunkListColumn::RowCount.ui(ui, &mut self.chunk_list_sort_column);
            });

            for &timeline in all_timelines.keys() {
                row.col(|ui| {
                    ChunkListColumn::Timeline(timeline).ui(ui, &mut self.chunk_list_sort_column);
                });
            }

            row.col(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                ui.strong("Components");
            });
        };

        let row_ui = |mut row: TableRow<'_, '_>| {
            let chunk = &chunks[row.index()];

            row.col(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                if ui.button(chunk.id().to_string()).clicked() {
                    self.focused_chunk = Some(ChunkUi::new(chunk));
                }
            });

            row.col(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                ui.label(chunk.entity_path().to_string());
            });

            row.col(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                ui.label(chunk.num_rows().to_string());
            });

            let timeline_ranges = chunk
                .timelines()
                .iter()
                .map(|(timeline, time_column)| (timeline, time_column.time_range()))
                .collect::<BTreeMap<_, _>>();

            for timeline in all_timelines.values() {
                row.col(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    if let Some(time_range) = timeline_ranges.get(timeline.name()) {
                        ui.label(format_time_range(timeline, time_range, timestamp_format));
                    } else {
                        ui.label("-");
                    }
                });
            }

            row.col(|ui| {
                ui.label(
                    chunk
                        .components()
                        .keys()
                        .map(|name| name.as_str())
                        .join(", "),
                );
            });
        };

        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                let table_builder = egui_extras::TableBuilder::new(ui)
                    .id_salt(chunk_store.id())
                    .columns(
                        Column::auto_with_initial_suggestion(200.0).clip(true),
                        4 + all_timelines.len(),
                    )
                    .resizable(true)
                    .vscroll(true)
                    .auto_shrink([false, false])
                    .striped(true);

                table_builder
                    .header(tokens.table_row_height(table_style), header_ui)
                    .body(|body| {
                        body.rows(tokens.table_row_height(table_style), chunks.len(), row_ui);
                    });
            });
    }

    // copy the (filtered) chunks to clipboard if this returns true
    fn chunk_store_info_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        datastore_ui_active: &mut bool,
    ) -> bool {
        let mut should_copy_chunks = false;

        ui.horizontal(|ui| {
            ui.selectable_toggle(|ui| {
                ui.selectable_value(&mut self.store_kind, StoreKind::Recording, "Recording")
                    .on_hover_text("Display the current recording's data store");
                ui.selectable_value(&mut self.store_kind, StoreKind::Blueprint, "Blueprint")
                    .on_hover_text("Display the current recording's blueprint store");
            });

            if ui
                .button("Close")
                .on_hover_text("Close the datastore browser")
                .clicked()
            {
                *datastore_ui_active = false;
            }

            if ui
                .button("Copy")
                .on_hover_text("Copy the currently listed chunks as text")
                .clicked()
            {
                should_copy_chunks = true;
            }
        });

        list_item::list_item_scope(ui, "chunk store info", |ui| {
            let stats = chunk_store.stats().total();
            ui.list_item_collapsible_noninteractive_label("Info", false, |ui| {
                // Note: no need to print the store kind, because it's selected by the top-level toggle.
                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Application ID")
                        .value_text(chunk_store.id().application_id().to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Recording ID")
                        .value_text(chunk_store.id().recording_id().to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Chunk count")
                        .value_text(stats.num_chunks.to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Heap size")
                        .value_text(re_format::format_bytes(stats.total_size_bytes as f64)),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Rows").value_text(stats.num_rows.to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Events")
                        .value_text(stats.num_events.to_string()),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Generation")
                        .value_text(format!("{:?}", chunk_store.generation())),
                );
            });

            ui.list_item_collapsible_noninteractive_label("Config", false, |ui| {
                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Enable changelog")
                        .value_bool(chunk_store.config().enable_changelog),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Chunk max byte").value_text(
                        re_format::format_bytes(chunk_store.config().chunk_max_bytes as f64),
                    ),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Chunk max rows")
                        .value_uint(chunk_store.config().chunk_max_rows),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Chunk max rows (unsorted)")
                        .value_uint(chunk_store.config().chunk_max_rows_if_unsorted),
                );
            });
        });

        should_copy_chunks
    }
}

fn format_time_range(
    timeline: &Timeline,
    time_range: &AbsoluteTimeRange,
    timestamp_format: TimestampFormat,
) -> String {
    if time_range.min() == time_range.max() {
        timeline.typ().format(time_range.min(), timestamp_format)
    } else {
        let length = match timeline.typ() {
            TimeType::Sequence => {
                format!("{} ticks", re_format::format_uint(time_range.abs_length()))
            }

            // The relartive time for both these are duration:
            TimeType::DurationNs | TimeType::TimestampNs => {
                format!(
                    "{}s",
                    re_format::format_f64(
                        (time_range.max().as_f64() - time_range.min().as_f64()) / 1_000_000_000.0
                    )
                )
            }
        };

        format!(
            "{} ({length})",
            timeline.format_time_range(time_range, timestamp_format)
        )
    }
}
