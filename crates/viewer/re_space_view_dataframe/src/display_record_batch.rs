//! Intermediate data structures to make `re_datastore`'s schemas and [`RecordBatch`]s more amenable
//! to displaying in a table.

use thiserror::Error;

use re_chunk_store::external::re_chunk::external::arrow2::{
    array::{
        Array as ArrowArray, DictionaryArray as ArrowDictionaryArray, ListArray as ArrowListArray,
        PrimitiveArray as ArrowPrimitiveArray, StructArray as ArrowStructArray,
    },
    datatypes::DataType,
    datatypes::DataType as ArrowDataType,
};
use re_chunk_store::{ColumnDescriptor, ComponentColumnDescriptor, LatestAtQuery, RowId};
use re_dataframe::RecordBatch;
use re_log_types::{EntityPath, TimeInt, Timeline};
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

pub(crate) enum ComponentData {
    Null,
    ListArray(ArrowListArray<i32>),
    DictionaryArray {
        dict: ArrowDictionaryArray<u32>,
        values: ArrowListArray<i32>,
    },
}

impl ComponentData {
    fn try_new(
        descriptor: &ComponentColumnDescriptor,
        column_data: &Box<dyn ArrowArray>,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_data.data_type() {
            DataType::Null => Ok(Self::Null),
            DataType::List(_) => Ok(Self::ListArray(
                column_data
                    .as_any()
                    .downcast_ref::<ArrowListArray<i32>>()
                    .expect("sanity checked")
                    .clone(),
            )),
            DataType::Dictionary(IntegerType::UInt32, _, _) => {
                let dict = column_data
                    .as_any()
                    .downcast_ref::<ArrowDictionaryArray<u32>>()
                    .expect("sanity checked")
                    .clone();
                let values = dict
                    .values()
                    .as_any()
                    .downcast_ref::<ArrowListArray<i32>>()
                    .expect("sanity checked")
                    .clone();
                Ok(Self::DictionaryArray { dict, values })
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
            Self::Null => {
                ui.label("null");
                return;
            }
            Self::ListArray(list_array) => {
                list_array.is_valid(index).then(|| list_array.value(index))
            }
            Self::DictionaryArray { dict, values } => dict
                .is_valid(index)
                .then(|| values.value(dict.key_value(index))),
        };

        if let Some(data) = data {
            ctx.component_ui_registry.ui_raw(
                ctx,
                ui,
                UiLayout::List,
                latest_at_query,
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

pub(crate) enum DisplayColumn {
    RowId {
        row_id_times: ArrowPrimitiveArray<u64>,
        row_id_counters: ArrowPrimitiveArray<u64>,
    },
    Timeline {
        timeline: Timeline,
        time_data: ArrowPrimitiveArray<i64>,
    },
    Component {
        entity_path: EntityPath,
        component_name: ComponentName,
        component_data: ComponentData,
    },
}

impl DisplayColumn {
    fn try_new(
        column_schema: &ColumnDescriptor,
        column_data: &Box<dyn ArrowArray>,
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
                    let row_id_times = times
                        .as_any()
                        .downcast_ref::<ArrowPrimitiveArray<u64>>()
                        .expect("sanity checked")
                        .clone();

                    #[allow(clippy::unwrap_used)]
                    let row_id_counters = counters
                        .as_any()
                        .downcast_ref::<ArrowPrimitiveArray<u64>>()
                        .expect("sanity checked")
                        .clone();

                    Ok(Self::RowId {
                        //descriptor: desc,
                        row_id_times,
                        row_id_counters,
                    })
                } else {
                    Err(DisplayRecordBatchError::UnknownControlColumn(
                        desc.component_name.to_string(),
                    ))
                }
            }
            ColumnDescriptor::Time(desc) => {
                let time_data = column_data
                    .as_any()
                    .downcast_ref::<ArrowPrimitiveArray<i64>>()
                    .ok_or_else(|| {
                        DisplayRecordBatchError::UnexpectedTimeColumnDataType(
                            desc.timeline.name().as_str().to_owned(),
                            column_data.data_type().to_owned(),
                        )
                    })?
                    .clone();

                Ok(Self::Timeline {
                    timeline: desc.timeline,
                    time_data,
                })
            }
            ColumnDescriptor::Component(desc) => Ok(Self::Component {
                entity_path: desc.entity_path.clone(),
                component_name: desc.component_name,
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
            Self::RowId {
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
            Self::Timeline {
                timeline,
                time_data,
            } => {
                let timestamp = TimeInt::try_from(time_data.value(index));
                match timestamp {
                    Ok(timestamp) => {
                        ui.label(timeline.typ().format(timestamp, ctx.app_options.time_zone));
                    }
                    Err(err) => {
                        ui.error_label(&format!("{err}"));
                    }
                }
            }
            Self::Component {
                entity_path,
                component_name,
                component_data,
            } => {
                component_data.data_ui(
                    ctx,
                    ui,
                    row_id,
                    latest_at_query,
                    entity_path,
                    *component_name,
                    index,
                );
            }
        }
    }
}

pub(crate) struct DisplayRecordBatch {
    num_rows: usize,
    columns: Vec<DisplayColumn>,
}

impl DisplayRecordBatch {
    /// Create a new `DisplayRecordBatch` from a `RecordBatch` and its schema.
    ///
    /// The columns in the record batch must match the schema. This is guaranteed by `re_datastore`.
    pub(crate) fn try_new(
        record_batch: &RecordBatch,
        schema: &[ColumnDescriptor],
    ) -> Result<Self, DisplayRecordBatchError> {
        let columns: Result<Vec<_>, _> = schema
            .iter()
            .zip(record_batch.all_columns())
            .map(|(column_schema, (_, column_data))| {
                DisplayColumn::try_new(column_schema, column_data)
            })
            .collect();

        Ok(Self {
            num_rows: record_batch.num_rows(),
            columns: columns?,
        })
    }

    pub(crate) fn num_rows(&self) -> usize {
        self.num_rows
    }

    pub(crate) fn columns(&self) -> &[DisplayColumn] {
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
