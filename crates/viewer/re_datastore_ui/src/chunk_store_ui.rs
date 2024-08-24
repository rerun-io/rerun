use std::collections::BTreeMap;
use std::sync::Arc;

use egui_extras::{Column, TableRow};
use itertools::{Either, Itertools};

use re_chunk_store::{Chunk, ChunkStore, LatestAtQuery, RangeQuery};
use re_log_types::{StoreKind, TimeZone, TimelineName};
use re_ui::{list_item, UiExt as _};
use re_viewer_context::ViewerContext;

use crate::chunk_list_mode::{ChunkListMode, ChunkListQueryMode};
use crate::SortDirection;

fn outer_frame() -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::same(5.0),
        ..Default::default()
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
enum ChunkListColumn {
    #[default]
    RowId,
    EntityPath,
    RowCount,
    Timeline(TimelineName),
}

#[derive(Default, Clone, Copy)]
struct ChunkListSortColumn {
    column: ChunkListColumn,
    direction: SortDirection,
}

impl ChunkListColumn {
    pub(crate) fn ui(&self, ui: &mut egui::Ui, sort_column: &mut ChunkListSortColumn) {
        match self {
            Self::RowId => self.ui_impl(ui, sort_column, "ID"),
            Self::EntityPath => self.ui_impl(ui, sort_column, "Entity"),
            Self::RowCount => self.ui_impl(ui, sort_column, "Row#"),
            Self::Timeline(name) => self.ui_impl(ui, sort_column, name.as_str()),
        }
    }

    fn ui_impl(
        &self,
        ui: &mut egui::Ui,
        sort_column: &mut ChunkListSortColumn,
        label: &'static str,
    ) {
        let label = format!(
            "{label}{}",
            if self == &sort_column.column {
                format!(" {}", sort_column.direction)
            } else {
                String::new()
            }
        );

        if ui
            .add(egui::Button::new(egui::WidgetText::from(label).strong()))
            .clicked()
        {
            if &sort_column.column == self {
                sort_column.direction.toggle();
            } else {
                sort_column.column = *self;
                sort_column.direction = SortDirection::default();
            }
        }
    }
}

/// Browser UI for [`re_chunk_store::ChunkStore`].
pub struct DatastoreUi {
    store_kind: StoreKind,
    focused_chunk: Option<Arc<Chunk>>,

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
        outer_frame().show(ui, |ui| {
            if let Some(focused_chunk) = self.focused_chunk.clone() {
                if crate::chunk_ui::chunk_ui(ui, &focused_chunk, time_zone) {
                    self.focused_chunk = None;
                }
            } else {
                self.chunk_store_ui(
                    ui,
                    match self.store_kind {
                        StoreKind::Recording => ctx.recording_store(),
                        StoreKind::Blueprint => ctx.blueprint_store(),
                    },
                    datastore_ui_active,
                    time_zone,
                );
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

            ui.label("Entity:");
            ui.text_edit_singleline(&mut self.entity_path_filter);

            ui.label("Component:");
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
            ChunkListColumn::RowId => {} // already sorted by row IDs
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
            let s = chunks.iter().map(|chunk| chunk.to_string()).join("\n\n");
            ui.output_mut(|o| o.copied_text = s);
        }

        //
        // Table
        //

        let header_ui = |mut row: TableRow<'_, '_>| {
            row.col(|ui| {
                ChunkListColumn::RowId.ui(ui, &mut self.chunk_list_sort_column);
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
                ui.strong("Components");
            });
        };

        let row_ui = |mut row: TableRow<'_, '_>| {
            let chunk = &chunks[row.index()];

            row.col(|ui| {
                if ui.button(chunk.id().to_string()).clicked() {
                    self.focused_chunk = Some(Arc::clone(chunk));
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
                .map(|(timeline, time_column)| {
                    (
                        timeline,
                        timeline.format_time_range(&time_column.time_range(), time_zone),
                    )
                })
                .collect::<BTreeMap<_, _>>();

            for timeline in &all_timelines {
                if let Some(range) = timeline_ranges.get(timeline) {
                    row.col(|ui| {
                        ui.label(range);
                    });
                } else {
                    row.col(|ui| {
                        ui.label("-");
                    });
                }
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

                //TODO: `TableBuilder` should have a custom ID API.
                //TODO: btw, set unique UIs in dataframe view as well.
                ui.push_id("chunk_list", |ui| {
                    let table_builder = egui_extras::TableBuilder::new(ui)
                        .columns(
                            Column::auto_with_initial_suggestion(200.0).clip(true),
                            4 + all_timelines.len(),
                        )
                        .resizable(true)
                        .vscroll(true)
                        //TODO(ab): remove when https://github.com/emilk/egui/pull/4817 is merged/released
                        .max_scroll_height(f32::INFINITY)
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

        let chunk_store_stats_ui = |ui: &mut egui::Ui| {
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("ID").value_text(chunk_store.id().to_string()),
            );

            //TODO: config

            let stats = chunk_store.stats().total();
            list_item::ListItem::new()
                .interactive(false)
                .show_hierarchical_with_children(
                    ui,
                    "stats".into(),
                    true,
                    list_item::LabelContent::new("Stats"),
                    |ui| {
                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Chunk count")
                                .value_text(stats.num_chunks.to_string()),
                        );

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Heap size")
                                .value_text(re_format::format_bytes(stats.total_size_bytes as f64)),
                        );

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Rows")
                                .value_text(stats.num_rows.to_string()),
                        );

                        ui.list_item_flat_noninteractive(
                            list_item::PropertyContent::new("Events")
                                .value_text(stats.num_events.to_string()),
                        );
                    },
                );
        };

        ui.horizontal(|ui| {
            ui.selectable_toggle(|ui| {
                ui.selectable_value(&mut self.store_kind, StoreKind::Recording, "Recording");
                ui.selectable_value(&mut self.store_kind, StoreKind::Blueprint, "Blueprint");
            });

            if ui.button("Close").clicked() {
                *datastore_ui_active = false;
            }

            if ui.button("Copy").clicked() {
                should_copy_chunks = true;
            }
        });

        list_item::list_item_scope(ui, "chunk_store_stats", |ui| {
            list_item::ListItem::new()
                .interactive(false)
                .show_hierarchical_with_children(
                    ui,
                    "chunk_store_stats".into(),
                    false,
                    list_item::LabelContent::new("Chunk store stats"),
                    chunk_store_stats_ui,
                );
        });

        should_copy_chunks
    }
}
