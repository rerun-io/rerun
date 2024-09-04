use crate::table_ui::row_id_ui;
use egui::ahash::HashMap;
use itertools::izip;
use re_chunk_store::external::re_chunk::external::arrow2::array::{
    Array as ArrowArray, ListArray, PrimitiveArray as ArrowPrimitiveArray, StructArray,
};
use re_chunk_store::{ColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::{RangeQueryHandle, RecordBatch};
use re_log_types::{EntityPath, TimeInt, Timeline, TimelineName};
use re_types_core::ComponentName;
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

struct DowncastRecordBatch<'a> {
    record_batch: &'a RecordBatch,

    row_id_times: &'a ArrowPrimitiveArray<u64>,
    row_id_counters: &'a ArrowPrimitiveArray<u64>,
    times: HashMap<TimelineName, &'a ArrowPrimitiveArray<i64>>,
    components: HashMap<(EntityPath, ComponentName), &'a ListArray<i32>>,
}

impl<'a> DowncastRecordBatch<'a> {
    fn from_record_batch_and_schema(
        record_batch: &'a RecordBatch,
        schema: &[ColumnDescriptor],
    ) -> Self {
        let mut row_id_times = None;
        let mut row_id_counters = None;
        let mut times = HashMap::default();
        let mut components = HashMap::default();

        for (schema_column, (_, column_data)) in schema.iter().zip(record_batch.all_columns()) {
            match schema_column {
                ColumnDescriptor::Control(desc) => {
                    if desc.component_name == ComponentName::from("rerun.controls.RowId") {
                        let row_ids = column_data.as_any().downcast_ref::<StructArray>().unwrap();
                        let [times, counters] = row_ids.values() else {
                            panic!(
                                "RowIds are corrupt -- this should be impossible (sanity checked)"
                            );
                        };

                        #[allow(clippy::unwrap_used)]
                        let times = times
                            .as_any()
                            .downcast_ref::<ArrowPrimitiveArray<u64>>()
                            .unwrap(); // sanity checked

                        #[allow(clippy::unwrap_used)]
                        let counters = counters
                            .as_any()
                            .downcast_ref::<ArrowPrimitiveArray<u64>>()
                            .unwrap(); // sanity checked

                        row_id_times = Some(times);
                        row_id_counters = Some(counters);
                    } else {
                        panic!("Unknown control column");
                    }
                }
                ColumnDescriptor::Time(desc) => {
                    let time = column_data
                        .as_any()
                        .downcast_ref::<ArrowPrimitiveArray<i64>>()
                        .unwrap();
                    times.insert(*desc.timeline.name(), time);
                }
                ColumnDescriptor::Component(desc) => {
                    let list = column_data
                        .as_any()
                        .downcast_ref::<ListArray<i32>>()
                        .unwrap();
                    components.insert(
                        (desc.entity_path.clone(), desc.component_name.clone()),
                        list,
                    );
                }
            }
        }

        Self {
            record_batch,
            row_id_times: row_id_times.unwrap(),
            row_id_counters: row_id_counters.unwrap(),
            times,
            components,
        }
    }

    fn row_id(&self, row_index: usize) -> RowId {
        let row_id = RowId::from_u128(
            (self.row_id_times.value(row_index) as u128) << 64
                | (self.row_id_counters.value(row_index) as u128),
        );
        row_id
    }
}

pub(crate) fn range_dataframe_ui(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    query_timeline: Timeline,
    query_handle: RangeQueryHandle<'_>,
) {
    re_tracing::profile_function!();

    let schema = query_handle.schema();
    let num_rows = query_handle.num_rows();
    dbg!(num_rows);

    let record_batches = query_handle.get(0, num_rows);

    dbg!(record_batches.len());

    let total_batch_rows = record_batches
        .iter()
        .map(|batch| batch.num_rows())
        .sum::<usize>();
    dbg!(total_batch_rows);

    if total_batch_rows != num_rows as usize {
        ui.error_label(&format!(
            "Row count mismatch: sum of record batch {total_batch_rows} (in {} batches) != query  {num_rows}", record_batches.len()
        ));
        return;
    }

    let downcast_record_batches = record_batches
        .iter()
        .map(|batch| DowncastRecordBatch::from_record_batch_and_schema(batch, schema))
        .collect::<Vec<_>>();

    let get_batch_and_index = |row: usize| -> (&DowncastRecordBatch, usize) {
        assert!(row < num_rows as usize);

        let mut row = row;
        for batch in &downcast_record_batches {
            if row < batch.record_batch.num_rows() {
                return (batch, row);
            }
            row -= batch.record_batch.num_rows();
        }
        panic!("row out of bounds");
    };

    let header_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        for column in schema {
            row.col(|ui| {
                ui.strong(column.short_name());
            });
        }
    };

    let row_ui = |mut row: egui_extras::TableRow<'_, '_>| {
        re_tracing::profile_scope!("row_ui");
        let (batch, batch_index) = get_batch_and_index(row.index());

        for schema_column in schema {
            row.col(|ui| match schema_column {
                ColumnDescriptor::Control(desc) => {
                    if desc.component_name == ComponentName::from("rerun.controls.RowId") {
                        row_id_ui(ui, &batch.row_id(batch_index));
                    } else {
                        // shouldn't happen
                        ui.error_label("Unknown control column");
                    }
                }
                ColumnDescriptor::Time(desc) => {
                    if let Some(times) = batch.times.get(desc.timeline.name()) {
                        let timestamp = times.value(batch_index);
                        ui.label(format!("{timestamp}"));
                    } else {
                        ui.error_label("Unknown timeline");
                    }
                }
                ColumnDescriptor::Component(desc) => {
                    if let Some(column_data) = batch
                        .components
                        .get(&(desc.entity_path.clone(), desc.component_name.clone()))
                    {
                        let data = column_data
                            .is_valid(batch_index)
                            .then(|| column_data.value(batch_index));

                        if let Some(data) = data {
                            //TODO: use correct time!!!!
                            let latest_at_query = LatestAtQuery::new(query_timeline, TimeInt::MAX);
                            ctx.component_ui_registry.ui_raw(
                                ctx,
                                ui,
                                UiLayout::List,
                                &latest_at_query,
                                ctx.recording(),
                                &desc.entity_path,
                                desc.component_name,
                                None, //TODO: provide correct row_id
                                &*data,
                            );
                        } else {
                            ui.label("null");
                        }
                    } else {
                        // shouldn't happen
                        ui.error_label("Unknown component column");
                    }
                }
            });
        }
    };

    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

            egui::Frame {
                inner_margin: egui::Margin::same(5.0),
                ..Default::default()
            }
            .show(ui, |ui| {
                let mut table_builder = egui_extras::TableBuilder::new(ui)
                    .columns(
                        egui_extras::Column::auto_with_initial_suggestion(200.0).clip(true),
                        schema.len(),
                    )
                    .resizable(true)
                    .vscroll(true)
                    //TODO(ab): remove when https://github.com/emilk/egui/pull/4817 is merged/released
                    .max_scroll_height(f32::INFINITY)
                    .auto_shrink([false, false])
                    .striped(true);

                // if let Some(scroll_to_row) = scroll_to_row {
                //     table_builder =
                //         table_builder.scroll_to_row(scroll_to_row, Some(egui::Align::TOP));
                // }

                table_builder
                    .header(re_ui::DesignTokens::table_line_height(), header_ui)
                    .body(|body| {
                        body.rows(
                            re_ui::DesignTokens::table_line_height(),
                            //TODO: minor annoyance
                            num_rows as usize,
                            row_ui,
                        );
                    });
            });
        });
}
