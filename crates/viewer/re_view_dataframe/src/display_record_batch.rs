//! Intermediate data structures to make `re_datastore`'s row data more amenable to displaying in a
//! table.

use arrow::{
    array::{
        Array as _, ArrayRef as ArrowArrayRef, Int32DictionaryArray as ArrowInt32DictionaryArray,
        ListArray as ArrowListArray,
    },
    buffer::NullBuffer as ArrowNullBuffer,
    buffer::ScalarBuffer as ArrowScalarBuffer,
    datatypes::DataType as ArrowDataType,
};
use thiserror::Error;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::LatestAtQuery;
use re_dataframe::external::re_chunk::{TimeColumn, TimeColumnError};
use re_log_types::external::re_tuid::Tuid;
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_sorbet::{ColumnDescriptorRef, ComponentColumnDescriptor};
use re_types_core::{ComponentName, DeserializationError, Loggable as _};
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

#[derive(Error, Debug)]
pub enum DisplayRecordBatchError {
    #[error("Bad column for timeline '{timeline}': {error}")]
    BadTimeColumn {
        timeline: String,
        error: TimeColumnError,
    },

    #[error("Unexpected column data type for component '{0}': {1:?}")]
    UnexpectedComponentColumnDataType(String, ArrowDataType),

    #[error(transparent)]
    DeserializationError(#[from] DeserializationError),
}

/// A single column of component data.
///
/// Abstracts over the different possible arrow representations of component data.
#[derive(Debug)]
pub enum ComponentData {
    Null,
    ListArray(ArrowListArray),
    DictionaryArray {
        dict: ArrowInt32DictionaryArray,
        values: ArrowListArray,
    },
    SomethingElse(ArrowArrayRef),
}

impl ComponentData {
    fn try_new(
        _descriptor: &ComponentColumnDescriptor,
        column_data: &ArrowArrayRef,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_data.data_type() {
            ArrowDataType::Null => Ok(Self::Null),
            ArrowDataType::List(_) => Ok(Self::ListArray(
                column_data
                    .downcast_array_ref::<ArrowListArray>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone(),
            )),
            ArrowDataType::Dictionary(_, _) => {
                let dict = column_data
                    .downcast_array_ref::<ArrowInt32DictionaryArray>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone();
                let values = dict
                    .values()
                    .downcast_array_ref::<ArrowListArray>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone();
                Ok(Self::DictionaryArray { dict, values })
            }
            // _ => Err(DisplayRecordBatchError::UnexpectedComponentColumnDataType(
            //     descriptor.component_name.to_string(),
            //     column_data.data_type().to_owned(),
            // )),
            _ => Ok(Self::SomethingElse(column_data.clone())),
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
            Self::SomethingElse(array) => 1,
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
            Self::SomethingElse(array_ref) => {
                re_ui::arrow_ui(ui, UiLayout::List, array_ref);
                return;
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

            ctx.component_ui_registry().ui_raw(
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
pub enum DisplayColumn {
    RowId {
        row_ids: Vec<Tuid>,
    },
    Timeline {
        timeline: Timeline,
        time_data: ArrowScalarBuffer<i64>,
        time_nulls: Option<ArrowNullBuffer>,
    },
    Component {
        entity_path: EntityPath,
        component_name: ComponentName,
        component_data: ComponentData,
    },
}

impl DisplayColumn {
    fn try_new(
        column_descriptor: &ColumnDescriptorRef<'_>,
        column_data: &ArrowArrayRef,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_descriptor {
            ColumnDescriptorRef::RowId(_desc) => Ok(Self::RowId {
                row_ids: Tuid::from_arrow(column_data)?,
            }),

            ColumnDescriptorRef::Time(desc) => {
                let timeline = desc.timeline();

                let (time_data, time_nulls) = TimeColumn::read_nullable_array(column_data)
                    .map_err(|err| DisplayRecordBatchError::BadTimeColumn {
                        timeline: timeline.name().as_str().to_owned(),
                        error: err,
                    })?;

                Ok(Self::Timeline {
                    timeline,
                    time_data,
                    time_nulls,
                })
            }
            ColumnDescriptorRef::Component(desc) => Ok(Self::Component {
                entity_path: desc.entity_path.clone(),
                component_name: desc.component_name,
                component_data: ComponentData::try_new(desc, column_data)?,
            }),
        }
    }

    pub fn instance_count(&self, row_index: usize) -> u64 {
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
    pub fn data_ui(
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
            Self::RowId { row_ids } => {
                if instance_index.is_some() {
                    // we only ever display the row id on the summary line
                    return;
                }

                ui.label(row_ids[row_index].to_string());
            }
            Self::Timeline {
                timeline,
                time_data,
                time_nulls,
            } => {
                if instance_index.is_some() {
                    // we only ever display the row id on the summary line
                    return;
                }

                let is_valid = time_nulls
                    .as_ref()
                    .map_or(true, |nulls| nulls.is_valid(row_index));

                if let (true, Some(value)) = (is_valid, time_data.get(row_index)) {
                    match TimeInt::try_from(*value) {
                        Ok(timestamp) => {
                            ui.label(
                                timeline
                                    .typ()
                                    .format(timestamp, ctx.app_options().timestamp_format),
                            );
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
    pub fn try_decode_time(&self, row_index: usize) -> Option<TimeInt> {
        match self {
            Self::Timeline { time_data, .. } => {
                let timestamp = time_data.get(row_index)?;
                TimeInt::try_from(*timestamp).ok()
            }
            Self::RowId { .. } | Self::Component { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct DisplayRecordBatch {
    num_rows: usize,
    columns: Vec<DisplayColumn>,
}

impl DisplayRecordBatch {
    /// Create a new `DisplayRecordBatch` from a `RecordBatch` and its list of selected columns.
    ///
    /// The columns in the record batch must match the selected columns. This is guaranteed by
    /// `re_datastore`.
    pub fn try_new<'a>(
        data: impl Iterator<Item = (ColumnDescriptorRef<'a>, ArrowArrayRef)>,
    ) -> Result<Self, DisplayRecordBatchError> {
        let mut num_rows = None;

        let columns: Result<Vec<_>, _> = data
            .map(|(column_descriptor, column_data)| {
                if num_rows.is_none() {
                    num_rows = Some(column_data.len());
                }
                DisplayColumn::try_new(&column_descriptor, &column_data)
            })
            .collect();

        Ok(Self {
            num_rows: num_rows.unwrap_or(0),
            columns: columns?,
        })
    }

    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    pub fn columns(&self) -> &[DisplayColumn] {
        &self.columns
    }
}
