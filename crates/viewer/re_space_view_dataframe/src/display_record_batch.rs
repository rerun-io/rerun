//! Intermediate data structures to make `re_datastore`'s row data more amenable to displaying in a
//! table.

use thiserror::Error;

use re_chunk_store::external::arrow2::{
    array::{
        Array as ArrowArray, DictionaryArray as ArrowDictionaryArray, ListArray as ArrowListArray,
        PrimitiveArray as ArrowPrimitiveArray, StructArray as ArrowStructArray,
    },
    datatypes::DataType,
    datatypes::DataType as ArrowDataType,
};
use re_chunk_store::{ColumnDescriptor, ComponentColumnDescriptor, LatestAtQuery, RowId};
use re_log_types::{EntityPath, TimeInt, TimeType, Timeline};
use re_types::external::arrow2::datatypes::IntegerType;
use re_types_core::{ComponentName, Loggable as _};
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

/// A single column of component data.
///
/// Abstracts over the different possible arrow representation of component data.
#[derive(Debug)]
pub(crate) enum ComponentData {
    Null,
    ListArray(ArrowListArray<i32>),
    DictionaryArray {
        dict: ArrowDictionaryArray<i32>,
        values: ArrowListArray<i32>,
    },
}

impl ComponentData {
    #[allow(clippy::borrowed_box)] // https://github.com/rust-lang/rust-clippy/issues/11940
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
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone(),
            )),
            DataType::Dictionary(IntegerType::Int32, _, _) => {
                let dict = column_data
                    .as_any()
                    .downcast_ref::<ArrowDictionaryArray<i32>>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone();
                let values = dict
                    .values()
                    .as_any()
                    .downcast_ref::<ArrowListArray<i32>>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone();
                Ok(Self::DictionaryArray { dict, values })
            }
            _ => Err(DisplayRecordBatchError::UnexpectedComponentColumnDataType(
                descriptor.component_name.to_string(),
                column_data.data_type().to_owned(),
            )),
        }
    }

    /// Returns the number of instances for the given row index.
    ///
    /// For [`Self::Null`] columns, or for invalid `row_index`, this will return 0.
    fn instance_count(&self, row_index: usize) -> u64 {
        match self {
            Self::Null => 0,
            Self::ListArray(list_array) => {
                if list_array.is_valid(row_index) {
                    list_array.value(row_index).len() as u64
                } else {
                    0
                }
            }
            Self::DictionaryArray { dict, values } => {
                if dict.is_valid(row_index) {
                    values.value(dict.key_value(row_index)).len() as u64
                } else {
                    0
                }
            }
        }
    }

    /// Display some data from the column.
    ///
    /// - Argument `row_index` is the row index within the batch column.
    /// - Argument `instance_index` is the specific instance within the specified row. If `None`, a
    ///   summary of all existing instances is displayed.
    ///
    /// # Panic
    ///
    /// Panics if `instance_index` is out-of-bound. Use [`Self::instance_count`] to ensure
    /// correctness.
    #[allow(clippy::too_many_arguments)]
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        row_id: RowId,
        latest_at_query: &LatestAtQuery,
        entity_path: &EntityPath,
        component_name: ComponentName,
        row_index: usize,
        instance_index: Option<u64>,
    ) {
        let data = match self {
            Self::Null => {
                // don't repeat the null value when expanding instances
                if instance_index.is_none() {
                    ui.label("null");
                }
                return;
            }
            Self::ListArray(list_array) => list_array
                .is_valid(row_index)
                .then(|| list_array.value(row_index)),
            Self::DictionaryArray { dict, values } => dict
                .is_valid(row_index)
                .then(|| values.value(dict.key_value(row_index))),
        };

        if let Some(data) = data {
            let data_to_display = if let Some(instance_index) = instance_index {
                // Panics if the instance index is out of bound. This is checked in
                // `DisplayColumn::data_ui`.
                data.sliced(instance_index as usize, 1)
            } else {
                data
            };

            ctx.component_ui_registry.ui_raw(
                ctx,
                ui,
                UiLayout::List,
                latest_at_query,
                ctx.recording(),
                entity_path,
                component_name,
                Some(row_id),
                &*data_to_display,
            );
        } else {
            ui.label("-");
        }
    }
}

/// A single column of data in a record batch.
#[derive(Debug)]
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
    #[allow(clippy::borrowed_box)] // https://github.com/rust-lang/rust-clippy/issues/11940
    fn try_new(
        column_descriptor: &ColumnDescriptor,
        column_data: &Box<dyn ArrowArray>,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_descriptor {
            ColumnDescriptor::Control(desc) => {
                if desc.component_name == RowId::name() {
                    let row_ids = column_data
                        .as_any()
                        .downcast_ref::<ArrowStructArray>()
                        .expect("expected format for RowId, failure is a bug in re_dataframe");
                    let [times, counters] = row_ids.values() else {
                        panic!("RowIds are corrupt -- this should be impossible (sanity checked)");
                    };

                    #[allow(clippy::unwrap_used)]
                    let row_id_times = times
                        .as_any()
                        .downcast_ref::<ArrowPrimitiveArray<u64>>()
                        .expect("expected format for RowId, failure is a bug in re_dataframe")
                        .clone();

                    #[allow(clippy::unwrap_used)]
                    let row_id_counters = counters
                        .as_any()
                        .downcast_ref::<ArrowPrimitiveArray<u64>>()
                        .expect("expected format for RowId, failure is a bug in re_dataframe")
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

    pub(crate) fn instance_count(&self, row_index: usize) -> u64 {
        match self {
            Self::RowId { .. } | Self::Timeline { .. } => 1,
            Self::Component { component_data, .. } => component_data.instance_count(row_index),
        }
    }

    /// Display some data in the column.
    ///
    /// - Argument `row_index` is the row index within the batch column.
    /// - Argument `instance_index` is the specific instance within the row to display. If `None`,
    ///   a summary of all instances is displayed. If the instance is out-of-bound (aka greater than
    ///   [`Self::instance_count`]), nothing is displayed.
    pub(crate) fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        row_id: RowId,
        latest_at_query: &LatestAtQuery,
        row_index: usize,
        instance_index: Option<u64>,
    ) {
        if let Some(instance_index) = instance_index {
            if instance_index >= self.instance_count(row_index) {
                // do not display anything for out-of-bound instance index
                return;
            }
        }

        match self {
            Self::RowId {
                row_id_times,
                row_id_counters,
                ..
            } => {
                if instance_index.is_some() {
                    // we only ever display the row id on the summary line
                    return;
                }

                let row_id = RowId::from_u128(
                    (row_id_times.value(row_index) as u128) << 64
                        | (row_id_counters.value(row_index) as u128),
                );
                row_id_ui(ctx, ui, &row_id);
            }
            Self::Timeline {
                timeline,
                time_data,
            } => {
                if instance_index.is_some() {
                    // we only ever display the row id on the summary line
                    return;
                }

                let timestamp = TimeInt::try_from(time_data.value(row_index));
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
                    row_index,
                    instance_index,
                );
            }
        }
    }

    /// Try to decode the row ID from the given row index.
    ///
    /// Succeeds only if the column is a `RowId` column.
    pub(crate) fn try_decode_row_id(&self, row_index: usize) -> Option<RowId> {
        match self {
            Self::RowId {
                row_id_times,
                row_id_counters,
            } => {
                let time = row_id_times.value(row_index);
                let counter = row_id_counters.value(row_index);
                Some(RowId::from_u128((time as u128) << 64 | (counter as u128)))
            }
            _ => None,
        }
    }

    /// Try to decode the time from the given row index.
    ///
    /// Succeeds only if the column is a `Timeline` column.
    pub(crate) fn try_decode_time(&self, row_index: usize) -> Option<TimeInt> {
        match self {
            Self::Timeline { time_data, .. } => {
                let timestamp = time_data.value(row_index);
                TimeInt::try_from(timestamp).ok()
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct DisplayRecordBatch {
    num_rows: usize,
    columns: Vec<DisplayColumn>,
}

impl DisplayRecordBatch {
    /// Create a new `DisplayRecordBatch` from a `RecordBatch` and its list of selected columns.
    ///
    /// The columns in the record batch must match the selected columns. This is guaranteed by
    /// `re_datastore`.
    pub(crate) fn try_new(
        row_data: &Vec<Box<dyn ArrowArray>>,
        selected_columns: &[ColumnDescriptor],
    ) -> Result<Self, DisplayRecordBatchError> {
        let num_rows = row_data.first().map(|arr| arr.len()).unwrap_or(0);

        let columns: Result<Vec<_>, _> = selected_columns
            .iter()
            .zip(row_data)
            .map(|(column_descriptor, column_data)| {
                DisplayColumn::try_new(column_descriptor, column_data)
            })
            .collect();

        Ok(Self {
            num_rows,
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

fn row_id_ui(ctx: &ViewerContext<'_>, ui: &mut egui::Ui, row_id: &RowId) {
    let s = row_id.to_string();
    let split_pos = s.char_indices().nth_back(5);

    ui.label(match split_pos {
        Some((pos, _)) => &s[pos..],
        None => &s,
    })
    .on_hover_ui(|ui| {
        let text = format!(
            "{}\n\nTimestamp: {}\nIncrement: {}",
            s,
            (row_id.nanoseconds_since_epoch() as i64)
                .try_into()
                .map(|t| TimeType::Time.format(TimeInt::from_nanos(t), ctx.app_options.time_zone))
                .unwrap_or("error decoding timestamp".to_owned()),
            row_id.inc()
        );

        ui.label(text);
    });
}
