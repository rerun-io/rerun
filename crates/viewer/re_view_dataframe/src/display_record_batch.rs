//! Intermediate data structures to make `re_datastore`'s row data more amenable to displaying in a
//! table.

use arrow::buffer::ScalarBuffer as ArrowScalarBuffer;
use arrow::{
    array::{
        Array as ArrowArray, ArrayRef as ArrowArrayRef,
        Int32DictionaryArray as ArrowInt32DictionaryArray, ListArray as ArrowListArray,
    },
    datatypes::DataType as ArrowDataType,
};
use thiserror::Error;

use re_chunk_store::{ColumnDescriptor, ComponentColumnDescriptor, LatestAtQuery};
use re_dataframe::external::re_chunk::{TimeColumn, TimeColumnError};
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_types_core::ComponentName;
use re_ui::UiExt;
use re_viewer_context::{UiLayout, ViewerContext};

#[derive(Error, Debug)]
pub(crate) enum DisplayRecordBatchError {
    #[error("Bad column for timeline '{timeline}': {error}")]
    BadTimeColumn {
        timeline: String,
        error: TimeColumnError,
    },

    #[error("Unexpected column data type for component '{0}': {1:?}")]
    UnexpectedComponentColumnDataType(String, ArrowDataType),
}

/// A single column of component data.
///
/// Abstracts over the different possible arrow representation of component data.
#[derive(Debug)]
pub(crate) enum ComponentData {
    Null,
    ListArray(ArrowListArray),
    DictionaryArray {
        dict: ArrowInt32DictionaryArray,
        values: ArrowListArray,
    },
}

impl ComponentData {
    fn try_new(
        descriptor: &ComponentColumnDescriptor,
        column_data: &ArrowArrayRef,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_data.data_type() {
            ArrowDataType::Null => Ok(Self::Null),
            ArrowDataType::List(_) => Ok(Self::ListArray(
                column_data
                    .as_any()
                    .downcast_ref::<ArrowListArray>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone(),
            )),
            ArrowDataType::Dictionary(_, _) => {
                let dict = column_data
                    .as_any()
                    .downcast_ref::<ArrowInt32DictionaryArray>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone();
                let values = dict
                    .values()
                    .as_any()
                    .downcast_ref::<ArrowListArray>()
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
                if let Some(key) = dict.key(row_index) {
                    values.value(key).len() as u64
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
            Self::DictionaryArray { dict, values } => {
                dict.key(row_index).map(|key| values.value(key))
            }
        };

        if let Some(data) = data {
            let data_to_display = if let Some(instance_index) = instance_index {
                // Panics if the instance index is out of bound. This is checked in
                // `DisplayColumn::data_ui`.
                data.slice(instance_index as usize, 1)
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
                None,
                data_to_display.as_ref(),
            );
        } else {
            ui.label("-");
        }
    }
}

/// A single column of data in a record batch.
#[derive(Debug)]
pub(crate) enum DisplayColumn {
    Timeline {
        timeline: Timeline,
        time_data: ArrowScalarBuffer<i64>,
    },
    Component {
        entity_path: EntityPath,
        component_name: ComponentName,
        component_data: ComponentData,
    },
}

impl DisplayColumn {
    fn try_new(
        column_descriptor: &ColumnDescriptor,
        column_data: &ArrowArrayRef,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_descriptor {
            ColumnDescriptor::Time(desc) => {
                let timeline = desc.timeline;

                let time_data = TimeColumn::read_array(column_data).map_err(|err| {
                    DisplayRecordBatchError::BadTimeColumn {
                        timeline: timeline.name().as_str().to_owned(),
                        error: err,
                    }
                })?;

                Ok(Self::Timeline {
                    timeline,
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
            Self::Timeline { .. } => 1,
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
            Self::Timeline {
                timeline,
                time_data,
            } => {
                if instance_index.is_some() {
                    // we only ever display the row id on the summary line
                    return;
                }

                if let Some(value) = time_data.get(row_index) {
                    match TimeInt::try_from(*value) {
                        Ok(timestamp) => {
                            ui.label(timeline.typ().format(timestamp, ctx.app_options.time_zone));
                        }
                        Err(err) => {
                            ui.error_with_details_on_hover(err.to_string());
                        }
                    }
                } else {
                    ui.label("-");
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
                    latest_at_query,
                    entity_path,
                    *component_name,
                    row_index,
                    instance_index,
                );
            }
        }
    }

    /// Try to decode the time from the given row index.
    ///
    /// Succeeds only if the column is a `Timeline` column.
    pub(crate) fn try_decode_time(&self, row_index: usize) -> Option<TimeInt> {
        match self {
            Self::Timeline { time_data, .. } => {
                let timestamp = time_data.get(row_index)?;
                TimeInt::try_from(*timestamp).ok()
            }
            Self::Component { .. } => None,
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
        row_data: &Vec<ArrowArrayRef>,
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
