use std::collections::BTreeMap;
use std::sync::Arc;

use egui_extras::{Column, TableRow};
use itertools::{Either, Itertools};

use re_chunk_store::external::re_chunk::{ArrowArray, TransportChunk};
use re_chunk_store::{Chunk, ChunkStore, LatestAtQuery, RangeQuery};
use re_log_types::{StoreKind, TimeZone};
use re_types::datatypes::TimeInt;
use re_types::SizeBytes;
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{UiLayout, ViewerContext};

use crate::chunk_list_mode::{ChunkListMode, ChunkListQueryMode};

fn outer_frame() -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::same(5.0),
        ..Default::default()
    }
}

/// Browser UI for [`re_chunk_store::ChunkStore`].
pub struct DatastoreUi {
    store_kind: StoreKind,
    focused_chunk: Option<Arc<Chunk>>,

    chunk_list_mode: ChunkListMode,

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
            entity_path_filter: String::new(),
            component_filter: String::new(),
        }
    }
}

impl DatastoreUi {
    /// Show the ui.
    pub fn ui(&mut self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui, time_zone: TimeZone) {
        outer_frame().show(ui, |ui| {
            if let Some(focused_chunk) = self.focused_chunk.clone() {
                self.chunk_ui(ui, &focused_chunk, time_zone);
            } else {
                self.chunk_store_ui(
                    ui,
                    match self.store_kind {
                        StoreKind::Recording => ctx.recording_store(),
                        StoreKind::Blueprint => ctx.blueprint_store(),
                    },
                    time_zone,
                );
            }
        });
    }

    fn chunk_store_ui(&mut self, ui: &mut egui::Ui, chunk_store: &ChunkStore, time_zone: TimeZone) {
        let should_copy_chunk = self.chunk_store_info_ui(ui, chunk_store);

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

        let chunks = chunk_iterator.collect_vec();

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
                ui.strong("ID");
            });

            row.col(|ui| {
                ui.strong("EntityPath");
            });

            row.col(|ui| {
                ui.strong("Rows");
            });

            for timeline in &all_timelines {
                row.col(|ui| {
                    ui.strong(timeline.name().as_str());
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
    fn chunk_store_info_ui(&mut self, ui: &mut egui::Ui, chunk_store: &ChunkStore) -> bool {
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

    fn chunk_ui(&mut self, ui: &mut egui::Ui, chunk: &Arc<Chunk>, time_zone: TimeZone) {
        self.chunk_info_ui(ui, chunk);

        let row_ids = chunk.row_ids().collect_vec();
        let time_columns = chunk.timelines().values().collect_vec();

        let components = chunk
            .components()
            .iter()
            .map(|(component_name, list_array)| {
                (*component_name, format!("{:#?}", list_array.data_type()))
            })
            .collect::<BTreeMap<_, _>>();

        let header_ui = |mut row: TableRow<'_, '_>| {
            row.col(|ui| {
                ui.strong("Row ID");
            });

            for time_column in &time_columns {
                row.col(|ui| {
                    ui.strong(time_column.timeline().name().as_str());
                });
            }

            for (component_name, datatype) in &components {
                row.col(|ui| {
                    let response = ui.button(component_name.short_name()).on_hover_ui(|ui| {
                        ui.label(format!("{datatype}\n\nClick header to copy"));
                    });

                    if response.clicked() {
                        ui.output_mut(|o| o.copied_text = datatype.clone());
                    }
                });
            }
        };

        let row_ui = |mut row: TableRow<'_, '_>| {
            let row_index = row.index();
            let row_id = row_ids[row_index];

            row.col(|ui| {
                ui.label(row_id.to_string());
            });

            for time_column in &time_columns {
                row.col(|ui| {
                    let time = TimeInt::from(time_column.times_raw()[row_index]);
                    ui.label(time_column.timeline().typ().format(time, time_zone));
                });
            }

            for component_name in components.keys() {
                row.col(|ui| {
                    let component_data = chunk.component_batch_raw(component_name, row_index);
                    if let Some(Ok(data)) = component_data {
                        crate::arrow_ui::arrow_ui(ui, UiLayout::List, &*data);
                    } else {
                        //TODO: handle error here
                        ui.label("-");
                    }
                });
            }
        };

        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                //TODO: `TableBuilder` should have a custom ID API.
                //TODO: btw, set unique UIs in dataframe view as well.
                ui.push_id("chunk", |ui| {
                    let table_builder = egui_extras::TableBuilder::new(ui)
                        .columns(
                            Column::auto_with_initial_suggestion(200.0).clip(true),
                            1 + time_columns.len() + components.len(),
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
                                row_ids.len(),
                                row_ui,
                            );
                        });
                });
            });
    }

    fn chunk_info_ui(&mut self, ui: &mut egui::Ui, chunk: &Arc<Chunk>) {
        let metadata_ui = |ui: &mut egui::Ui, metadata: &BTreeMap<String, String>| {
            for (key, value) in metadata {
                ui.list_item_flat_noninteractive(
                    list_item::PropertyContent::new(key).value_text(value),
                );
            }
        };

        let fields_ui = |ui: &mut egui::Ui, transport: &TransportChunk| {
            for field in &transport.schema.fields {
                ui.push_id(field.name.clone(), |ui| {
                    ui.list_item_collapsible_noninteractive_label(&field.name, false, |ui| {
                        ui.list_item_collapsible_noninteractive_label("Data type", false, |ui| {
                            ui.label(format!("{:#?}", field.data_type));
                        });

                        ui.list_item_collapsible_noninteractive_label("Metadata", false, |ui| {
                            metadata_ui(ui, &field.metadata);
                        });
                    });
                });
            }
        };

        let chunk_stats_ui = |ui: &mut egui::Ui| {
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("ID").value_text(chunk.id().to_string()),
            );

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Entity")
                    .value_text(chunk.entity_path().to_string()),
            );

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Row count")
                    .value_text(chunk.num_rows().to_string()),
            );

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Heap size").value_text(re_format::format_bytes(
                    <Chunk as SizeBytes>::heap_size_bytes(chunk) as f64,
                )),
            );

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Sorted").value_text(if chunk.is_sorted() {
                    "yes"
                } else {
                    "no"
                }),
            );

            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Static").value_text(if chunk.is_static() {
                    "yes"
                } else {
                    "no"
                }),
            );
        };

        ui.horizontal(|ui| {
            if ui.button("Back").clicked() {
                self.focused_chunk = None;
            }

            if ui.button("Copy").clicked() {
                let s = chunk.to_string();
                ui.output_mut(|o| o.copied_text = s);
            }
        });

        list_item::list_item_scope(ui, "chunk_stats", |ui| {
            ui.list_item_collapsible_noninteractive_label("Stats", false, chunk_stats_ui);
            match chunk.to_transport() {
                Ok(transport) => {
                    ui.list_item_collapsible_noninteractive_label("Transport", false, |ui| {
                        ui.list_item_collapsible_noninteractive_label("Metadata", false, |ui| {
                            metadata_ui(ui, &transport.schema.metadata);
                        });
                        ui.list_item_collapsible_noninteractive_label("Fields", false, |ui| {
                            fields_ui(ui, &transport);
                        });
                    });
                }
                Err(err) => {
                    ui.error_label(&format!("Failed to convert to transport: {err}"));
                }
            }
        });
    }
}
