//! Intermediate data structures to make `re_datastore`'s schemas and [`RecordBatch`]s more amenable
//! to displaying in a table.

use thiserror::Error;

use re_chunk_store::external::re_chunk::external::arrow2::datatypes::DataType;
use re_chunk_store::external::re_chunk::external::arrow2::{
    array::{
        Array as ArrowArray, DictionaryArray as ArrowDictionaryArray, ListArray as ArrowListArray,
        PrimitiveArray as ArrowPrimitiveArray, StructArray as ArrowStructArray,
    },
    datatypes::DataType as ArrowDataType,
};
use re_chunk_store::{
    ColumnDescriptor, ComponentColumnDescriptor, ControlColumnDescriptor, LatestAtQuery, RowId,
    TimeColumnDescriptor,
};
use re_dataframe::RecordBatch;
use re_log_types::{EntityPath, TimeInt};
use re_types::external::arrow2::datatypes::IntegerType;
use re_types_core::ComponentName;
use re_ui::UiExt;
use re_viewer_context::{UiLayout, ViewerContext};

#[derive(Error, Debug)]
pub(crate) enum DisplayRecordBatchError {
    #[error("Unknown control column: {0}")]
    UnknownControlColumn(String),

    #[error("Unexpected column data type for timeline '{0}': {1:?}")]
    UnexpectedTimeColumnDataType(String, ArrowDataType),

    #[error("Unexpected column data type for component '{0}': {1:?}")]
    UnexpectedComponentColumnDataType(String, ArrowDataType),
}

pub(crate) enum ComponentData<'a> {
    Null,
    ListArray(&'a ArrowListArray<i32>),
    DictionaryArray {
        dict: &'a ArrowDictionaryArray<u32>,
        values: &'a ArrowListArray<i32>,
    },
}

impl<'a> ComponentData<'a> {
    fn try_new(
        descriptor: &ComponentColumnDescriptor,
        column_data: &'a Box<dyn ArrowArray>,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_data.data_type() {
            DataType::Null => Ok(ComponentData::Null),
            DataType::List(_) => Ok(ComponentData::ListArray(
                column_data
                    .as_any()
                    .downcast_ref::<ArrowListArray<i32>>()
                    .expect("sanity checked"),
            )),
            DataType::Dictionary(IntegerType::UInt32, _, _) => {
                let dict = column_data
                    .as_any()
                    .downcast_ref::<ArrowDictionaryArray<u32>>()
                    .expect("sanity checked");
                let values = dict
                    .values()
                    .as_any()
                    .downcast_ref::<ArrowListArray<i32>>()
                    .expect("sanity checked");
                Ok(ComponentData::DictionaryArray { dict, values })
            }
            _ => Err(DisplayRecordBatchError::UnexpectedComponentColumnDataType(
                descriptor.component_name.to_string(),
                column_data.data_type().to_owned(),
            )),
        }
    }

    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        row_id: RowId,
        latest_at_query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
        index: usize,
    ) {
        let data = match self {
            ComponentData::Null => {
                ui.label("null");
                return;
            }
            ComponentData::ListArray(list_array) => {
                list_array.is_valid(index).then(|| list_array.value(index))
            }
            ComponentData::DictionaryArray { dict, values } => dict
                .is_valid(index)
                .then(|| values.value(dict.key_value(index))),
        };

        if let Some(data) = data {
            ctx.component_ui_registry.ui_raw(
                ctx,
                ui,
                UiLayout::List,
                &latest_at_query,
                ctx.recording(),
                entity_path,
                component_name,
                Some(row_id),
                &*data,
            );
        } else {
            ui.label("-");
        }
    }
}

pub(crate) enum DisplayColumn<'a> {
    RowId {
        descriptor: &'a ControlColumnDescriptor,
        row_id_times: &'a ArrowPrimitiveArray<u64>,
        row_id_counters: &'a ArrowPrimitiveArray<u64>,
    },
    Timeline {
        descriptor: &'a TimeColumnDescriptor,
        time_data: &'a ArrowPrimitiveArray<i64>,
    },
    Component {
        descriptor: &'a ComponentColumnDescriptor,
        //TODO: this should actually be an enum of possible component data types, eg null, dict,
        // etc.
        component_data: ComponentData<'a>,
    },
}

impl<'a> DisplayColumn<'a> {
    fn try_new(
        column_schema: &'a ColumnDescriptor,
        column_data: &'a Box<dyn ArrowArray>,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_schema {
            ColumnDescriptor::Control(desc) => {
                if desc.component_name == ComponentName::from("rerun.controls.RowId") {
                    let row_ids = column_data
                        .as_any()
                        .downcast_ref::<ArrowStructArray>()
                        .unwrap();
                    let [times, counters] = row_ids.values() else {
                        panic!("RowIds are corrupt -- this should be impossible (sanity checked)");
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

                    Ok(DisplayColumn::RowId {
                        descriptor: desc,
                        row_id_times: times,
                        row_id_counters: counters,
                    })
                } else {
                    Err(DisplayRecordBatchError::UnknownControlColumn(
                        desc.component_name.to_string(),
                    ))
                }
            }
            ColumnDescriptor::Time(desc) => {
                let time = column_data
                    .as_any()
                    .downcast_ref::<ArrowPrimitiveArray<i64>>()
                    .ok_or_else(|| {
                        DisplayRecordBatchError::UnexpectedTimeColumnDataType(
                            desc.timeline.name().as_str().to_owned(),
                            column_data.data_type().to_owned(),
                        )
                    })?;

                Ok(DisplayColumn::Timeline {
                    descriptor: desc,
                    time_data: time,
                })
            }
            ColumnDescriptor::Component(desc) => Ok(DisplayColumn::Component {
                descriptor: desc,
                component_data: ComponentData::try_new(desc, column_data)?,
            }),
        }
    }

    pub(crate) fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        row_id: RowId,
        latest_at_query: &LatestAtQuery,
        index: usize,
    ) {
        match self {
            DisplayColumn::RowId {
                row_id_times,
                row_id_counters,
                ..
            } => {
                let row_id = RowId::from_u128(
                    (row_id_times.value(index) as u128) << 64
                        | (row_id_counters.value(index) as u128),
                );
                row_id_ui(ui, &row_id);
            }
            DisplayColumn::Timeline {
                time_data,
                descriptor,
            } => {
                let timestamp = TimeInt::try_from(time_data.value(index));
                match timestamp {
                    Ok(timestamp) => {
                        ui.label(
                            descriptor
                                .timeline
                                .typ()
                                .format(timestamp, ctx.app_options.time_zone),
                        );
                    }
                    Err(err) => {
                        ui.error_label(&format!("{err}"));
                    }
                }
            }
            DisplayColumn::Component {
                component_data,
                descriptor,
            } => {
                component_data.data_ui(
                    ctx,
                    ui,
                    row_id,
                    latest_at_query,
                    &descriptor.entity_path,
                    descriptor.component_name,
                    index,
                );
            }
        }
    }
}

pub(crate) struct DisplayRecordBatch<'a> {
    record_batch: &'a RecordBatch,
    columns: Vec<DisplayColumn<'a>>,
}

impl<'a> DisplayRecordBatch<'a> {
    /// Create a new `DisplayRecordBatch` from a `RecordBatch` and its schema.
    ///
    /// The columns in the record batch must match the schema. This is guaranteed by `re_datastore`.
    pub(crate) fn try_new(
        record_batch: &'a RecordBatch,
        schema: &'a [ColumnDescriptor],
    ) -> Result<Self, DisplayRecordBatchError> {
        let columns: Result<Vec<_>, _> = schema
            .iter()
            .zip(record_batch.all_columns())
            .map(|(column_schema, (_, column_data))| {
                DisplayColumn::try_new(column_schema, column_data)
            })
            .collect();

        Ok(Self {
            record_batch,
            columns: columns?,
        })
    }

    pub(crate) fn num_rows(&self) -> usize {
        self.record_batch.num_rows()
    }

    pub(crate) fn columns(&self) -> &[DisplayColumn<'a>] {
        &self.columns
    }
}

fn row_id_ui(ui: &mut egui::Ui, row_id: &RowId) {
    let s = row_id.to_string();
    let split_pos = s.char_indices().nth_back(5);

    ui.label(match split_pos {
        Some((pos, _)) => &s[pos..],
        None => &s,
    })
    .on_hover_text(s);
}
