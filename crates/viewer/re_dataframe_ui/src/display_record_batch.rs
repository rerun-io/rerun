//! Intermediate data structures to make `re_datastore`'s row data more amenable to displaying in a
//! table.

use std::sync::Arc;

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
use re_log_types::hash::Hash64;
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_sorbet::{ColumnDescriptorRef, ComponentColumnDescriptor};
use re_types_core::{ComponentName, DeserializationError, Loggable as _, RowId};
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
        row_ids: Option<&[RowId]>,
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

            let mut cache_key = row_ids
                .and_then(|row_ids| row_ids.get(row_index))
                .copied()
                .map(Hash64::hash);

            // TODO(ab): we should find an alternative to using content-hashing to generate cache
            // keys.
            //
            // Generate a content-based cache key if we don't have one already. This is needed
            // because without cache key, the image thumbnail will no be displayed by the component
            // ui.
            if cache_key.is_none() && (component_name.as_str() == "rerun.components.Blob") {
                re_tracing::profile_scope!("Blob hash");

                if let Ok(Some(buffer)) = re_types::components::Blob::from_arrow(&data_to_display)
                    .as_ref()
                    .map(|blob| blob.first().map(|blob| blob as &[u8]))
                {
                    // cap the max amount of data to hash to 9 KiB
                    const SECTION_LENGTH: usize = 3 * 1024;

                    cache_key = Some(quick_partial_hash(buffer, SECTION_LENGTH));
                }
            }

            ctx.component_ui_registry().ui_raw(
                ctx,
                ui,
                UiLayout::List,
                latest_at_query,
                ctx.recording(),
                entity_path,
                component_name,
                cache_key,
                data_to_display.as_ref(),
            );
        } else {
            ui.label("-");
        }
    }
}

/// Compute a quick partial hash of an image data buffer, capping the max amount of hashed data to
/// `3*section_length`.
///
/// If the buffer is smaller or equal to than `3*section_length`, the entire buffer is hashed.
/// If the buffer is larger, the first, middle, and last sections, each of size `section_length`,
/// are hashed.
fn quick_partial_hash(data: &[u8], section_length: usize) -> Hash64 {
    use ahash::AHasher;
    use std::hash::{Hash as _, Hasher as _};

    re_tracing::profile_function!();

    let mut hasher = AHasher::default();
    data.len().hash(&mut hasher);

    if data.len() <= 3 * section_length {
        data.hash(&mut hasher);
    } else {
        let first_section = &data[..section_length];
        let last_section = &data[data.len() - section_length..];

        let middle_offset = (data.len() - section_length) / 2;
        let middle_section = &data[middle_offset..middle_offset + section_length];

        first_section.hash(&mut hasher);
        middle_section.hash(&mut hasher);
        last_section.hash(&mut hasher);
    }

    Hash64::from_u64(hasher.finish())
}

/// A single column of data in a record batch.
#[derive(Debug)]
pub enum DisplayColumn {
    RowId {
        row_ids: Arc<Vec<RowId>>,
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

        // if available, used to pass a row id to the component UI (e.g. to cache image)
        row_ids: Option<Arc<Vec<RowId>>>,
    },
}

impl DisplayColumn {
    fn try_new(
        column_descriptor: &ColumnDescriptorRef<'_>,
        column_data: &ArrowArrayRef,
    ) -> Result<Self, DisplayRecordBatchError> {
        match column_descriptor {
            ColumnDescriptorRef::RowId(_desc) => Ok(Self::RowId {
                row_ids: Arc::new(RowId::from_arrow(column_data)?),
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
                row_ids: None,
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
                    .is_none_or(|nulls| nulls.is_valid(row_index));

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
                row_ids,
            } => {
                component_data.data_ui(
                    ctx,
                    ui,
                    latest_at_query,
                    entity_path,
                    *component_name,
                    row_ids.as_deref().map(|row_ids| row_ids.as_slice()),
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
        let mut batch_row_ids = None;

        let mut columns = data
            .map(|(column_descriptor, column_data)| {
                if num_rows.is_none() {
                    num_rows = Some(column_data.len());
                }

                let column = DisplayColumn::try_new(&column_descriptor, &column_data);

                // find the batch row ids, if any
                if batch_row_ids.is_none() {
                    if let Ok(DisplayColumn::RowId { row_ids }) = &column {
                        batch_row_ids = Some(Arc::clone(row_ids));
                    }
                }

                column
            })
            .collect::<Result<Vec<DisplayColumn>, _>>()?;

        // If we have row_ids, give a reference to all component columns.
        if let Some(batch_row_ids) = batch_row_ids {
            for column in &mut columns {
                if let DisplayColumn::Component { row_ids, .. } = column {
                    *row_ids = Some(Arc::clone(&batch_row_ids));
                }
            }
        }

        Ok(Self {
            num_rows: num_rows.unwrap_or(0),
            columns,
        })
    }

    pub fn num_rows(&self) -> usize {
        self.num_rows
    }

    pub fn columns(&self) -> &[DisplayColumn] {
        &self.columns
    }
}
