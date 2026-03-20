use std::collections::BTreeMap;
use std::sync::Arc;

use egui_extras::{Column, TableRow};
use itertools::{Either, Itertools as _};
use re_chunk_store::{
    Chunk, ChunkId, ChunkStore, ChunkStoreGeneration, ChunkTrackingMode, LatestAtQuery, RangeQuery,
};
use re_log_types::{
    AbsoluteTimeRange, StoreId, StoreKind, TimeType, Timeline, TimelineName, TimestampFormat,
};
use re_ui::text_edit::autocomplete_text_edit;
use re_ui::{UiExt as _, list_item};
use re_viewer_context::external::re_entity_db::EntityDb;
use re_viewer_context::{ActiveStoreContext, StorageContext};

use crate::chunk_list_mode::{ChunkListMode, ChunkListQueryMode};
use crate::chunk_ui::ChunkUi;
use crate::sort::{SortColumn, SortDirection, sortable_column_header_ui};
use crate::toolbar_ui::{
    close_button_right_ui, copy_button_ui, info_toggle_button_ui, reset_button_ui,
};

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum StaticOrTemporal {
    #[default]
    All,
    Static,
    Temporal,
}

/// Caches autocomplete suggestions for the chunk list filters.
#[derive(Default)]
struct FilterSuggestionCache {
    store_kind: Option<StoreKind>,
    store_id: Option<StoreId>,
    generation: Option<ChunkStoreGeneration>,
    entity_suggestions: Vec<String>,
    component_suggestions: Vec<String>,
}

impl FilterSuggestionCache {
    fn clear(&mut self) {
        self.store_kind = None;
        self.store_id = None;
        self.generation = None;
        self.entity_suggestions.clear();
        self.component_suggestions.clear();
    }

    fn maybe_update(&mut self, store_kind: StoreKind, chunk_store: &ChunkStore) {
        let store_id = chunk_store.id().clone();
        let generation = chunk_store.generation();

        if self.store_kind == Some(store_kind)
            && self.store_id.as_ref() == Some(&store_id)
            && self.generation.as_ref() == Some(&generation)
        {
            return;
        }

        self.entity_suggestions = chunk_store
            .all_entities_sorted()
            .into_iter()
            .map(|entity| entity.to_string())
            .collect();
        self.component_suggestions = chunk_store
            .all_components_sorted()
            .into_iter()
            .map(|component| component.as_str().to_owned())
            .collect();
        self.store_kind = Some(store_kind);
        self.store_id = Some(store_id);
        self.generation = Some(generation);
    }

    fn entity_suggestions(&self) -> &[String] {
        &self.entity_suggestions
    }

    fn component_suggestions(&self) -> &[String] {
        &self.component_suggestions
    }
}

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
    selected_recording_id: Option<StoreId>,
    last_route_recording_id: Option<StoreId>,
    focused_chunk: Option<ChunkUi>,
    show_details_panels: bool,

    chunk_list_mode: ChunkListMode,

    chunk_list_sort_column: ChunkListSortColumn,
    static_filter: StaticOrTemporal,

    // filters
    entity_path_filter: String,
    component_filter: String,
    filter_suggestion_cache: FilterSuggestionCache,
}

impl Default for DatastoreUi {
    fn default() -> Self {
        Self {
            store_kind: StoreKind::Recording,
            selected_recording_id: None,
            last_route_recording_id: None,
            focused_chunk: None,
            show_details_panels: false,
            chunk_list_mode: ChunkListMode::default(),
            chunk_list_sort_column: ChunkListSortColumn::default(),
            static_filter: StaticOrTemporal::default(),
            entity_path_filter: String::new(),
            component_filter: String::new(),
            filter_suggestion_cache: FilterSuggestionCache::default(),
        }
    }
}

pub struct DatastoreUiResult {
    pub keep_open: bool,

    /// The effective recording shown by the chunk browser.
    ///
    /// This may differ from the route's input recording because the chunk browser
    /// can switch recordings locally via its recording selector.
    pub recording_id: StoreId,

    pub selected_chunk: Option<ChunkId>,
}

impl DatastoreUi {
    fn reset_ui_state(&mut self) {
        self.store_kind = StoreKind::Recording;
        self.selected_recording_id = None;
        self.last_route_recording_id = None;
        self.focused_chunk = None;
        self.show_details_panels = false;
        self.chunk_list_mode = ChunkListMode::default();
        self.chunk_list_sort_column = ChunkListSortColumn::default();
        self.static_filter = StaticOrTemporal::default();
        self.entity_path_filter.clear();
        self.component_filter.clear();
        self.filter_suggestion_cache.clear();
    }

    /// Syncs the local focused chunk UI with the chunk selected in navigation.
    ///
    /// Falls back to the chunk list if that chunk is no longer available.
    fn sync_focused_chunk(&mut self, chunk_store: &ChunkStore, selected_chunk: Option<ChunkId>) {
        match selected_chunk {
            Some(selected_chunk)
                if self
                    .focused_chunk
                    .as_ref()
                    .is_some_and(|chunk_ui| chunk_ui.chunk_id() == selected_chunk) => {}
            Some(selected_chunk) => {
                self.focused_chunk = chunk_store
                    .physical_chunk(&selected_chunk)
                    .map(ChunkUi::new);
            }
            None => {
                self.focused_chunk = None;
            }
        }
    }

    /// Show the ui.
    ///
    /// Returns `false` if the datastore UI should be closed (e.g., the close button was clicked),
    /// or `true` if the datastore UI should remain open.
    pub fn ui(
        &mut self,
        ctx: &ActiveStoreContext<'_>,
        storage_context: &StorageContext<'_>,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        selected_chunk: Option<ChunkId>,
    ) -> DatastoreUiResult {
        let mut datastore_ui_active = true;
        let route_recording_id = ctx.recording.store_id().clone();

        // Keep the local recording override in sync with navigation changes such as
        // back/forward actions that replace the chunk browser route from the outside.
        if self.last_route_recording_id.as_ref() != Some(&route_recording_id)
            && self.selected_recording_id.as_ref() != Some(&route_recording_id)
        {
            self.selected_recording_id = None;
        }
        self.last_route_recording_id = Some(route_recording_id);

        // Resolve the recording to display: use the selected one if valid, otherwise
        // fall back to the active recording.
        let recording = self
            .selected_recording_id
            .as_ref()
            .and_then(|id| storage_context.bundle.get(id))
            .unwrap_or(ctx.recording);

        let storage_engine = match self.store_kind {
            StoreKind::Recording => recording.storage_engine(),
            StoreKind::Blueprint => ctx.blueprint.storage_engine(),
        };
        let chunk_store = storage_engine.store();

        self.sync_focused_chunk(chunk_store, selected_chunk);
        let mut selected_chunk = self.focused_chunk.as_ref().map(ChunkUi::chunk_id);

        egui::Frame {
            inner_margin: egui::Margin::same(5),
            ..Default::default()
        }
        .show(ui, |ui| {
            let should_close_datastore_ui = if let Some(focused_chunk) = &mut self.focused_chunk {
                focused_chunk.ui(
                    ui,
                    timestamp_format,
                    &mut self.show_details_panels,
                    chunk_store,
                )
            } else {
                self.chunk_store_ui(
                    ui,
                    chunk_store,
                    storage_context,
                    &mut datastore_ui_active,
                    timestamp_format,
                    &mut selected_chunk,
                );

                false
            };

            if should_close_datastore_ui {
                datastore_ui_active = false;
                self.selected_recording_id = None;
                self.focused_chunk = None;
                selected_chunk = None;
            }
        });

        if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !egui::Popup::is_any_open(ui.ctx()) {
            datastore_ui_active = false;
            self.selected_recording_id = None;
            self.focused_chunk = None;
            selected_chunk = None;
        }

        DatastoreUiResult {
            keep_open: datastore_ui_active,
            recording_id: recording.store_id().clone(),
            selected_chunk,
        }
    }

    fn chunk_store_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        storage_context: &StorageContext<'_>,
        datastore_ui_active: &mut bool,
        timestamp_format: TimestampFormat,
        selected_chunk: &mut Option<ChunkId>,
    ) {
        let tokens = ui.tokens();
        let mut content_margin = tokens.panel_margin();
        content_margin.top = content_margin.top.max(6);

        let should_copy_chunk = egui::Panel::top("chunk_store_top_controls_panel")
            .show_inside(ui, |ui| {
                let should_copy_chunk = self.chunk_store_info_ui(
                    ui,
                    chunk_store,
                    storage_context,
                    datastore_ui_active,
                    selected_chunk,
                );
                let _ = self
                    .chunk_list_mode
                    .query_ui(ui, chunk_store, timestamp_format);
                should_copy_chunk
            })
            .inner;

        egui::Frame {
            inner_margin: content_margin,
            ..Default::default()
        }
        .show(ui, |ui| {
            self.chunk_store_details_ui(ui, chunk_store);
            if self.show_details_panels {
                ui.separator();
            }

            self.filter_controls_ui(ui, chunk_store);

            // Each of these must be a column that contains the corresponding time range.
            let all_timelines = chunk_store.schema().timelines();

            let table_style = re_ui::TableStyle::Dense;

            //
            // Collect chunks based on query mode
            //

            let chunks = self.collect_filtered_sorted_chunks(chunk_store);

            //
            // Copy to clipboard
            //

            if should_copy_chunk {
                //TODO(#7282): make sure the output is not dependant on the parent terminal's width
                let s = chunks.iter().map(|chunk| chunk.to_string()).join("\n\n");
                ui.copy_text(s);
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
                        ChunkListColumn::Timeline(timeline)
                            .ui(ui, &mut self.chunk_list_sort_column);
                    });
                }

                row.col(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    ui.strong("Components");
                });
            };

            let row_ui = ui
                .scope(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                    |mut row: TableRow<'_, '_>| {
                        let chunk = &chunks[row.index()];

                        row.col(|ui| {
                            if ui.button(chunk.id().to_string()).clicked() {
                                *selected_chunk = Some(chunk.id());
                            }
                        });

                        row.col(|ui| {
                            ui.label(chunk.entity_path().to_string());
                        });

                        row.col(|ui| {
                            ui.label(chunk.num_rows().to_string());
                        });

                        let timeline_ranges = chunk
                            .timelines()
                            .iter()
                            .map(|(timeline, time_column)| (timeline, time_column.time_range()))
                            .collect::<BTreeMap<_, _>>();

                        for timeline in all_timelines.values() {
                            row.col(|ui| {
                                if let Some(time_range) = timeline_ranges.get(timeline.name()) {
                                    ui.label(format_time_range(
                                        timeline,
                                        time_range,
                                        timestamp_format,
                                    ));
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
                    }
                })
                .inner;

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
        });
    }

    fn filter_controls_ui(&mut self, ui: &mut egui::Ui, chunk_store: &ChunkStore) {
        self.filter_suggestion_cache
            .maybe_update(self.store_kind, chunk_store);

        ui.horizontal(|ui| {
            // Make the filter text edits wider by default to accommodate long entity/component names.
            // Note that this is a handcrafted heuristic and may need adjustments if the UI layout changes.
            ui.spacing_mut().text_edit_width = (400f32).min(ui.available_width() / 3.);

            let filter_icon_rect = ui.small_icon(&re_ui::icons::FILTER, None);
            ui.interact(
                filter_icon_rect,
                ui.id().with("chunk_list_filter_icon"),
                egui::Sense::hover(),
            )
            .on_hover_text(
                "Filter the chunk list by entity path and/or component. Filtering is \
                case-insensitive text-based.",
            );
            ui.label("Entity:");
            autocomplete_text_edit(
                ui,
                &mut self.entity_path_filter,
                self.filter_suggestion_cache.entity_suggestions(),
                None::<&str>,
            );

            ui.label("Component:");
            autocomplete_text_edit(
                ui,
                &mut self.component_filter,
                self.filter_suggestion_cache.component_suggestions(),
                None::<&str>,
            );

            if ui
                .small_icon_button(&re_ui::icons::TRASH, "Clear filters")
                .clicked()
            {
                self.entity_path_filter.clear();
                self.component_filter.clear();
            }
        });
    }

    fn collect_filtered_sorted_chunks(&self, chunk_store: &ChunkStore) -> Vec<Arc<Chunk>> {
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

        let mut chunks: Vec<Arc<Chunk>> = chunk_iterator.collect();

        match self.static_filter {
            StaticOrTemporal::All => {}
            StaticOrTemporal::Static => chunks.retain(|chunk| chunk.is_static()),
            StaticOrTemporal::Temporal => chunks.retain(|chunk| !chunk.is_static()),
        }

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

        chunks
    }

    // copy the (filtered) chunks to clipboard if this returns true
    fn chunk_store_info_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        storage_context: &StorageContext<'_>,
        datastore_ui_active: &mut bool,
        selected_chunk: &mut Option<ChunkId>,
    ) -> bool {
        let store_kind_before = self.store_kind;
        let selected_recording_before = self.selected_recording_id.clone();

        let should_copy_chunks = ui
            .horizontal(|ui| {
                ui.selectable_toggle(|ui| {
                    ui.selectable_value(&mut self.store_kind, StoreKind::Recording, "Recording")
                        .on_hover_text("Display the current recording's data store");
                    ui.selectable_value(&mut self.store_kind, StoreKind::Blueprint, "Blueprint")
                        .on_hover_text("Display the current recording's blueprint store");
                });

                ui.separator();

                let _ = self.chunk_list_mode.selector_ui(ui, chunk_store);

                info_toggle_button_ui(
                    ui,
                    "Toggle info panels",
                    "Show/hide info and config sections",
                    &mut self.show_details_panels,
                );

                let should_copy_chunks = copy_button_ui(
                    ui,
                    "Copy chunks",
                    "Copy the currently listed chunks as text",
                );

                let reset_clicked =
                    reset_button_ui(ui, "Reset chunk browser", "Reset chunk browser state");

                ui.selectable_toggle(|ui| {
                    ui.selectable_value(&mut self.static_filter, StaticOrTemporal::All, "All")
                        .on_hover_text("Show all chunks regardless of static/temporal status");
                    ui.selectable_value(
                        &mut self.static_filter,
                        StaticOrTemporal::Static,
                        "Static",
                    )
                    .on_hover_text("Show only static chunks");
                    ui.selectable_value(
                        &mut self.static_filter,
                        StaticOrTemporal::Temporal,
                        "Temporal",
                    )
                    .on_hover_text("Show only non-static chunks");
                });

                ui.separator();
                self.switch_recording_ui(ui, chunk_store, storage_context);

                if reset_clicked {
                    self.reset_ui_state();
                    *selected_chunk = None;
                }

                let close_clicked = close_button_right_ui(
                    ui,
                    "Close datastore browser",
                    "Close the datastore browser",
                );

                if close_clicked {
                    *datastore_ui_active = false;
                    // Reset the selected recording when closing the chunk store UI,
                    // to avoid confusion when reopening it again from a different recording.
                    self.selected_recording_id = None;
                    *selected_chunk = None;
                }

                should_copy_chunks
            })
            .inner;

        if self.store_kind != store_kind_before
            || self.selected_recording_id != selected_recording_before
        {
            self.focused_chunk = None;
            *selected_chunk = None;
        }

        should_copy_chunks
    }

    fn chunk_store_details_ui(&self, ui: &mut egui::Ui, chunk_store: &ChunkStore) {
        if self.show_details_panels {
            egui::ScrollArea::vertical()
                .id_salt("chunk_store_info_scroll_area")
                .show(ui, |ui| {
                    list_item::list_item_scope(ui, "chunk store info", |ui| {
                        let stats = chunk_store.stats().total();
                        ui.list_item_collapsible_noninteractive_label("Info", true, |ui| {
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
                                list_item::PropertyContent::new("Heap size").value_text(
                                    re_format::format_bytes(stats.total_size_bytes as f64),
                                ),
                            );

                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("Rows")
                                    .value_text(stats.num_rows.to_string()),
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

                        ui.list_item_collapsible_noninteractive_label("Config", true, |ui| {
                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("Enable changelog")
                                    .value_bool(chunk_store.config().enable_changelog),
                            );

                            ui.list_item_flat_noninteractive(
                                list_item::PropertyContent::new("Chunk max byte").value_text(
                                    re_format::format_bytes(
                                        chunk_store.config().chunk_max_bytes as f64,
                                    ),
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
                });
        }
    }

    fn switch_recording_ui(
        &mut self,
        ui: &mut egui::Ui,
        chunk_store: &ChunkStore,
        storage_context: &StorageContext<'_>,
    ) {
        let recordings: Vec<&EntityDb> = storage_context.bundle.recordings().collect();
        if recordings.is_empty() {
            return;
        }

        let selected_text = format!(
            "{} ({})",
            chunk_store.id().application_id(),
            chunk_store.id().recording_id(),
        );
        let current_store_id = chunk_store.id();

        let icon_rect = ui.small_icon(&re_ui::icons::DATASET, None);
        ui.interact(
            icon_rect,
            ui.id().with("selected_recording_icon"),
            egui::Sense::hover(),
        )
        .on_hover_text("Selected recording");
        egui::ComboBox::new("recording_selector", "")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for recording in &recordings {
                    let store_id = recording.store_id();
                    let label = format!(
                        "{} ({})",
                        store_id.application_id(),
                        store_id.recording_id(),
                    );
                    let is_selected = *store_id == current_store_id;
                    if ui
                        .add(re_ui::ComboItem::new(label).selected(is_selected))
                        .clicked()
                    {
                        self.selected_recording_id = Some(store_id.clone());
                    }
                }
            });
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

            // The relative time for both these are duration:
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
