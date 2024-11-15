use std::collections::BTreeMap;
use std::sync::Arc;

use egui_extras::{Column, TableRow};
use itertools::{Either, Itertools};

use re_chunk_store::{ChunkStore, LatestAtQuery, RangeQuery};
use re_log_types::{ResolvedTimeRange, StoreKind, TimeType, TimeZone, Timeline, TimelineName};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::ViewerContext;

use crate::chunk_list_mode::{ChunkListMode, ChunkListQueryMode};
use crate::chunk_ui::ChunkUi;
use crate::sort::{sortable_column_header_ui, SortColumn, SortDirection};

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
    pub fn ui(
        &mut self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        datastore_ui_active: &mut bool,
        time_zone: TimeZone,
    ) {
        egui::Frame {
            inner_margin: egui::Margin::same(5.0),
            ..Default::default()
        }
        .show(ui, |ui| {
            let exit_focused_chunk = if let Some(focused_chunk) = &mut self.focused_chunk {
                focused_chunk.ui(ui, time_zone)
            } else {
                self.chunk_store_ui(
                    ui,
                    match self.store_kind {
                        StoreKind::Recording => ctx.recording_engine(),
                        StoreKind::Blueprint => ctx.blueprint_engine(),
                    }
                    .store(),
                    datastore_ui_active,
                    time_zone,
                );

                false
            };

            if exit_focused_chunk {
                self.focused_chunk = None;
            }
        });
    }

    fn chunk_store_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        datastore_ui_active: &mut bool,
        time_zone: TimeZone,
    ) {
        let should_copy_chunk = self.chunk_store_info_ui(ui, chunk_store, datastore_ui_active);

        // Each of these must be a column that contains the corresponding time range.
        let all_timelines = chunk_store.all_timelines();

        self.chunk_list_mode.ui(ui, chunk_store, time_zone);

        //
        // Collect chunks based on query mode
        //

        let chunk_iterator = match &self.chunk_list_mode {
            ChunkListMode::All => Either::Left(chunk_store.iter_chunks().map(Arc::clone)),
            ChunkListMode::Query {
                timeline,
                entity_path,
                component_name,
                query: ChunkListQueryMode::LatestAt(at),
                ..
            } => Either::Right(
                chunk_store
                    .latest_at_relevant_chunks(
                        &LatestAtQuery::new(*timeline, *at),
                        entity_path,
                        *component_name,
                    )
                    .into_iter(),
            ),
            ChunkListMode::Query {
                timeline,
                entity_path,
                component_name,
                query: ChunkListQueryMode::Range(range),
                ..
            } => Either::Right(
                chunk_store
                    .range_relevant_chunks(
                        &RangeQuery::new(*timeline, *range),
                        entity_path,
                        *component_name,
                    )
                    .into_iter(),
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

            if ui.small_icon_button(&re_ui::icons::CLOSE).clicked() {
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
                    .any(|name| name.short_name().to_lowercase().contains(&component_filter))
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
                    .find(|(timeline, _)| timeline.name() == timeline_name)
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
            ui.output_mut(|o| o.copied_text = s);
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

            for timeline in &all_timelines {
                row.col(|ui| {
                    ChunkListColumn::Timeline(*timeline.name())
                        .ui(ui, &mut self.chunk_list_sort_column);
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

            for timeline in &all_timelines {
                row.col(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    if let Some(time_range) = timeline_ranges.get(timeline) {
                        ui.label(format_time_range(timeline, time_range, time_zone));
                    } else {
                        ui.label("-");
                    };
                });
            }

            row.col(|ui| {
                ui.label(
                    chunk
                        .components()
                        .keys()
                        .map(|name| name.short_name())
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
                    .header(re_ui::DesignTokens::table_line_height(), header_ui)
                    .body(|body| {
                        body.rows(
                            re_ui::DesignTokens::table_line_height(),
                            chunks.len(),
                            row_ui,
                        );
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
                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Store ID")
                        .value_text(chunk_store.id().to_string()),
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
                        .value_text(re_format::format_uint(chunk_store.config().chunk_max_rows)),
                );

                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new("Chunk max rows (unsorted)").value_text(
                        re_format::format_uint(chunk_store.config().chunk_max_rows_if_unsorted),
                    ),
                );
            });
        });

        should_copy_chunks
    }
}

fn format_time_range(
    timeline: &Timeline,
    time_range: &ResolvedTimeRange,
    time_zone: TimeZone,
) -> String {
    if time_range.min() == time_range.max() {
        timeline.typ().format(time_range.min(), time_zone)
    } else {
        format!(
            "{} ({})",
            timeline.format_time_range(time_range, time_zone),
            match timeline.typ() {
                TimeType::Time => {
                    format!(
                        "{}s",
                        re_format::format_f64(
                            (time_range.max().as_f64() - time_range.min().as_f64())
                                / 1_000_000_000.0
                        )
                    )
                }
                TimeType::Sequence => {
                    format!("{} ticks", re_format::format_uint(time_range.abs_length()))
                }
            }
        )
    }
}
