use std::sync::Arc;

use egui_extras::{Column, TableRow};
use itertools::{Either, Itertools};

use re_chunk_store::external::re_chunk::external::arrow2;
use re_chunk_store::external::re_chunk::external::arrow2::array::Utf8Array;
use re_chunk_store::{Chunk, ChunkStore};
use re_log_types::{StoreKind, TimeZone};
use re_types::datatypes::TimeInt;
use re_types::SizeBytes as _;
use re_ui::{list_item, UiExt as _};
use re_viewer_context::{UiLayout, ViewerContext};

fn outer_frame() -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::same(5.0),
        ..Default::default()
    }
}

pub struct DatastoreUi {
    store_kind: StoreKind,
    focused_chunk: Option<Arc<Chunk>>,

    // filters
    entity_path_filter: String,
    component_filter: String,
}

impl Default for DatastoreUi {
    fn default() -> Self {
        Self {
            store_kind: StoreKind::Recording,
            focused_chunk: None,
            entity_path_filter: String::new(),
            component_filter: String::new(),
        }
    }
}

impl DatastoreUi {
    pub fn ui(&mut self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui) {
        if let Some(focused_chunk) = self.focused_chunk.clone() {
            self.chunk_ui(ctx, ui, &focused_chunk);
        } else {
            self.chunk_store_ui(
                ui,
                match self.store_kind {
                    StoreKind::Recording => ctx.recording_store(),
                    StoreKind::Blueprint => ctx.blueprint_store(),
                },
            );
        }
    }

    fn chunk_store_ui(&mut self, ui: &mut egui::Ui, chunk_store: &ChunkStore) {
        let should_copy_chunk = self.chunk_store_info_ui(ui, chunk_store);

        let chunk_iterator = chunk_store.iter_chunks();

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
            Either::Right(chunk_iterator.filter(|chunk| {
                chunk
                    .entity_path()
                    .to_string()
                    .contains(&self.entity_path_filter)
            }))
        };

        let chunk_iterator = if self.component_filter.is_empty() {
            Either::Left(chunk_iterator)
        } else {
            Either::Right(chunk_iterator.filter(|chunk| {
                chunk
                    .components()
                    .keys()
                    .any(|name| name.short_name().contains(&self.component_filter))
            }))
        };

        let chunks: Vec<_> = chunk_iterator.collect_vec();

        //
        // Copy
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
                ui.strong("Sorted");
            });

            row.col(|ui| {
                ui.strong("Rows");
            });

            row.col(|ui| {
                ui.strong("Timelines");
            });

            row.col(|ui| {
                ui.strong("Components");
            });
        };

        let row_ui = |mut row: TableRow<'_, '_>| {
            let chunk = chunks[row.index()];
            row.col(|ui| {
                if ui.button(chunk.id().to_string()).clicked() {
                    self.focused_chunk = Some(Arc::clone(chunk));
                }
            });

            row.col(|ui| {
                ui.label(chunk.entity_path().to_string());
            });

            row.col(|ui| {
                ui.label(if chunk.is_sorted() { "yes" } else { "no" });
            });

            row.col(|ui| {
                ui.label(chunk.num_rows().to_string());
            });

            row.col(|ui| {
                if chunk.is_static() {
                    ui.label("static");
                } else {
                    ui.label(format!("{} timelines", chunk.timelines().len(),))
                        .on_hover_ui(|ui| {
                            ui.label(
                                chunk
                                    .timelines()
                                    .keys()
                                    .map(|timeline| timeline.name().as_str())
                                    .join(", "),
                            );
                        });
                }
            });

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

                outer_frame().show(ui, |ui| {
                    //TODO: `TableBuilder` should have a custom ID API.
                    //TODO: btw, set unique UIs in dataframe view as well.
                    ui.push_id("chunk_list", |ui| {
                        let table_builder = egui_extras::TableBuilder::new(ui)
                            .columns(Column::auto_with_initial_suggestion(200.0).clip(true), 6)
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

        outer_frame().show(ui, |ui| {
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
        });

        should_copy_chunks
    }

    fn chunk_ui(&mut self, ctx: &ViewerContext<'_>, ui: &mut egui::Ui, chunk: &Arc<Chunk>) {
        self.chunk_info_ui(ui, chunk);

        let row_ids = chunk.row_ids().collect_vec();
        let time_columns = chunk.timelines().values().collect_vec();
        let component_names = chunk.component_names().collect_vec();

        let header_ui = |mut row: TableRow<'_, '_>| {
            row.col(|ui| {
                ui.strong("Row ID");
            });

            for time_column in &time_columns {
                row.col(|ui| {
                    ui.strong(time_column.timeline().name().as_str());
                });
            }

            for component_name in &component_names {
                row.col(|ui| {
                    //TODO: tooltip: arrow schema
                    ui.strong(component_name.short_name()).on_hover_ui(|ui| {
                        //TODO(#1809): I wish there was a central place to look up for datatype
                        let datatype = ctx
                            .recording_store()
                            .lookup_datatype(component_name)
                            .or_else(|| ctx.blueprint_store().lookup_datatype(component_name));

                        if let Some(datatype) = datatype {
                            UiLayout::Tooltip.data_label(
                                ui,
                                re_format_arrow::DisplayDatatype(datatype).to_string(),
                            );
                        } else {
                            ui.error_label("Couldn't find a type definition.");
                        }
                    });
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

                    //TODO: use the user's timezone
                    ui.label(time_column.timeline().typ().format(time, TimeZone::Utc));
                });
            }

            for component_name in &component_names {
                row.col(|ui| {
                    let component_data = chunk.component_batch_raw(component_name, row_index);
                    if let Some(Ok(data)) = component_data {
                        arrow_ui(ui, UiLayout::List, &*data);
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

                outer_frame().show(ui, |ui| {
                    //TODO: `TableBuilder` should have a custom ID API.
                    //TODO: btw, set unique UIs in dataframe view as well.
                    ui.push_id("chunk", |ui| {
                        let table_builder = egui_extras::TableBuilder::new(ui)
                            .columns(
                                Column::auto_with_initial_suggestion(200.0).clip(true),
                                1 + time_columns.len() + component_names.len(),
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
            });
    }

    fn chunk_info_ui(&mut self, ui: &mut egui::Ui, chunk: &Arc<Chunk>) {
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
                list_item::PropertyContent::new("Heap size")
                    .value_text(re_format::format_bytes(chunk.heap_size_bytes() as f64)),
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

        outer_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Back").clicked() {
                    self.focused_chunk = None;
                }

                if ui.button("Copy").clicked() {
                    let s = chunk.to_string();
                    ui.output_mut(|o| o.copied_text = s);
                }

                // ui.help_hover_button().on_hover_ui(|ui| {
                //     list_item::list_item_scope(ui, "chunk_stats", chunk_stats_ui);
                // });

                // ui.scope(|ui| {
                //     ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                //     ui.label(format!(
                //         "ID: {} | Entity path: {} | Row count: {} | Heap size: {} | Sorted: {} | Static: {}",
                //         chunk.id().to_string(),
                //         chunk.entity_path().to_string(),
                //         chunk.num_rows(),
                //         re_format::format_bytes(chunk.heap_size_bytes() as f64),
                //         if chunk.is_sorted() { "yes" } else { "no" },
                //         if chunk.is_static() { "yes" } else { "no" },
                //     ));
            });

            list_item::list_item_scope(ui, "chunk_stats", |ui| {
                list_item::ListItem::new()
                    .interactive(false)
                    .show_hierarchical_with_children(
                        ui,
                        "chunk_stats".into(),
                        false,
                        list_item::LabelContent::new("Chunk stats"),
                        chunk_stats_ui,
                    );
            });
        });
    }
}

//TODO: adapted from `re_data_ui`
fn arrow_ui(
    ui: &mut egui::Ui,
    ui_layout: UiLayout,
    array: &dyn arrow2::array::Array,
) -> egui::Response {
    use re_types::SizeBytes as _;

    // Special-treat text.
    // Note: we match on the raw data here, so this works for any component containing text.
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i32>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            return ui_layout.data_label(ui, string);
        }
    }
    if let Some(utf8) = array.as_any().downcast_ref::<Utf8Array<i64>>() {
        if utf8.len() == 1 {
            let string = utf8.value(0);
            return ui_layout.data_label(ui, string);
        }
    }

    let num_bytes = array.total_size_bytes();
    if num_bytes < 3000 {
        //TODO: had to add that to avoid a panic
        if array.is_empty() {
            return ui_layout.data_label(ui, "[]");
        }

        // Print small items:
        let mut string = String::new();
        let display = arrow2::array::get_display(array, "null");
        if display(&mut string, 0).is_ok() {
            return ui_layout.data_label(ui, &string);
        }
    }

    // Fallback:
    let bytes = re_format::format_bytes(num_bytes as _);

    // TODO(emilk): pretty-print data type
    let data_type_formatted = format!("{:?}", array.data_type());

    if data_type_formatted.len() < 20 {
        // e.g. "4.2 KiB of Float32"
        ui_layout.data_label(ui, &format!("{bytes} of {data_type_formatted}"))
    } else {
        // Huge datatype, probably a union horror show
        ui_layout.label(ui, format!("{bytes} of data"))
    }
}
