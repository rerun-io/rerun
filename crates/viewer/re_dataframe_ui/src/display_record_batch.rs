//! Intermediate data structures to make `re_datastore`'s row data more amenable to displaying in a
//! table.

use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef as ArrowArrayRef, Int32DictionaryArray as ArrowInt32DictionaryArray,
    ListArray as ArrowListArray,
};
use arrow::buffer::{NullBuffer as ArrowNullBuffer, ScalarBuffer as ArrowScalarBuffer};
use arrow::datatypes::DataType as ArrowDataType;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::LatestAtQuery;
use re_component_ui::REDAP_THUMBNAIL_VARIANT;
use re_dataframe::external::re_chunk::{TimeColumn, TimeColumnError};
use re_log_types::hash::Hash64;
use re_log_types::{EntityPath, TimeInt, Timeline};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::components::{Blob, MediaType};
use re_sorbet::ColumnDescriptorRef;
use re_types_core::{Component as _, DeserializationError, Loggable as _, RowId};
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, VariantName, ViewerContext};

use crate::table_blueprint::ColumnBlueprint;

#[derive(thiserror::Error, Debug)]
pub enum DisplayRecordBatchError {
    #[error("Bad column for timeline '{timeline}': {error}")]
    BadTimeColumn {
        timeline: String,
        error: TimeColumnError,
    },

    #[error(transparent)]
    DeserializationError(#[from] DeserializationError),
}

/// Wrapper over the arrow data of a component column.
///
/// Abstracts over the different possible arrow representations of component data.
#[derive(Debug)]
enum ComponentData {
    Null,
    ListArray(ArrowListArray),
    DictionaryArray {
        dict: ArrowInt32DictionaryArray,
        values: ArrowListArray,
    },
    Scalar(ArrowArrayRef),
}

impl ComponentData {
    fn new(column_data: &ArrowArrayRef) -> Self {
        match column_data.data_type() {
            ArrowDataType::Null => Self::Null,
            ArrowDataType::List(_) => Self::ListArray(
                column_data
                    .downcast_array_ref::<ArrowListArray>()
                    .expect("`data_type` checked, failure is a bug in re_dataframe")
                    .clone(),
            ),
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
                Self::DictionaryArray { dict, values }
            }
            _ => Self::Scalar(Arc::clone(column_data)),
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
            Self::Scalar(_) => 1,
        }
    }

    /// Is this as [`ArrowDataType::Null`] column?
    fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns the content at the given row index.
    fn row_data(&self, row_index: usize) -> Option<ArrowArrayRef> {
        match self {
            Self::Null => None,

            Self::ListArray(list_array) => list_array
                .is_valid(row_index)
                .then(|| list_array.value(row_index)),

            Self::DictionaryArray { dict, values } => {
                dict.key(row_index).map(|key| values.value(key))
            }

            Self::Scalar(scalar_array) => {
                if row_index < scalar_array.len() {
                    Some(scalar_array.slice(row_index, 1))
                } else {
                    None
                }
            }
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
    use std::hash::{Hash as _, Hasher as _};

    use ahash::AHasher;

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

/// Data related to a single component column.
#[derive(Debug)]
pub struct DisplayComponentColumn {
    entity_path: EntityPath,
    component_descr: ComponentDescriptor,
    component_data: ComponentData,

    // if available, used to pass a row id to the component UI (e.g. to cache image)
    row_ids: Option<Arc<Vec<RowId>>>,

    /// The UI variant to use for this column, if any.
    variant_name: Option<VariantName>,
}

impl DisplayComponentColumn {
    fn blobs(&self, row: usize) -> Option<Vec<Blob>> {
        if self.component_descr.component_type != Some(re_sdk_types::components::Blob::name()) {
            return None;
        }

        self.component_data
            .row_data(row)
            .as_ref()
            .and_then(|data| Blob::from_arrow(data).ok())
    }

    fn is_blob_image(blob: &Blob) -> bool {
        MediaType::guess_from_data(blob.as_ref()).is_some_and(|t| t.starts_with("image/"))
    }

    pub fn is_image(&self) -> bool {
        self.component_descr.component_type == Some(re_sdk_types::components::Blob::name())
            && self
                .blobs(0)
                .as_ref()
                .and_then(|blobs| blobs.first())
                .is_some_and(Self::is_blob_image)
    }

    pub fn string_value_at(&self, row: usize) -> Option<String> {
        let data = self.component_data.row_data(row)?;

        let string_component = data.downcast_array_ref::<arrow::array::StringArray>()?;

        Some(string_component.value(0).to_owned())
    }

    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        latest_at_query: &LatestAtQuery,
        row_index: usize,
        instance_index: Option<u64>,
    ) {
        // handle null columns
        if self.component_data.is_null() {
            // don't repeat the null value when expanding instances
            if instance_index.is_none() {
                ui.label("null");
            }
            return;
        }

        let data = self.component_data.row_data(row_index);

        if let Some(data) = data {
            let data_to_display = if let Some(instance_index) = instance_index {
                // Panics if the instance index is out of bound. This is checked in
                // `DisplayColumn::data_ui`.
                data.slice(instance_index as usize, 1)
            } else {
                data
            };

            let mut row_id = self
                .row_ids
                .as_deref()
                .and_then(|row_ids| row_ids.get(row_index))
                .copied();

            let mut variant_name = self.variant_name;

            let blob = Blob::from_arrow(&data_to_display).ok();

            if let Some(blob) = blob.as_ref().and_then(|b| b.first())
                && Self::is_blob_image(blob)
            {
                variant_name = Some(VariantName::from(REDAP_THUMBNAIL_VARIANT));

                // TODO(ab): we should find an alternative to using content-hashing to generate cache
                // keys.
                //
                // Generate a content-based cache key if we don't have one already. This is needed
                // because without cache key, the image thumbnail will no be displayed by the component
                // ui.
                if row_id.is_none() {
                    re_tracing::profile_scope!("Blob hash");

                    // cap the max amount of data to hash to 9 KiB
                    const SECTION_LENGTH: usize = 3 * 1024;

                    // TODO(andreas, ab): This is a hack to create a pretend-row-id from the content hash.
                    row_id = Some(RowId::from_u128(
                        quick_partial_hash(blob.as_ref(), SECTION_LENGTH).hash64() as _,
                    ));
                }
            }

            if let Some(variant_name) = variant_name {
                ctx.component_ui_registry().variant_ui_raw(
                    ctx,
                    ui,
                    UiLayout::List,
                    variant_name,
                    &self.component_descr,
                    row_id,
                    data_to_display.as_ref(),
                );
            } else {
                ctx.component_ui_registry().component_ui_raw(
                    ctx,
                    ui,
                    UiLayout::List,
                    latest_at_query,
                    ctx.recording(),
                    &self.entity_path,
                    &self.component_descr,
                    row_id,
                    data_to_display.as_ref(),
                );
            }
        } else {
            ui.label("-");
        }
    }
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

    // Boxed for size reasons.
    Component(Box<DisplayComponentColumn>),
}

impl DisplayColumn {
    fn try_new(
        column_descriptor: &ColumnDescriptorRef<'_>,
        column_blueprint: &ColumnBlueprint,
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
            ColumnDescriptorRef::Component(desc) => {
                Ok(Self::Component(Box::new(DisplayComponentColumn {
                    entity_path: desc.entity_path.clone(),
                    component_descr: desc.component_descriptor(),
                    component_data: ComponentData::new(column_data),
                    row_ids: None,
                    variant_name: column_blueprint.variant_ui,
                })))
            }
        }
    }

    pub fn instance_count(&self, row_index: usize) -> u64 {
        match self {
            Self::RowId { .. } | Self::Timeline { .. } => 1,
            Self::Component(component_column) => {
                component_column.component_data.instance_count(row_index)
            }
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
        if let Some(instance_index) = instance_index
            && instance_index >= self.instance_count(row_index)
        {
            // do not display anything for out-of-bound instance index
            return;
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

                if is_valid && let Some(value) = time_data.get(row_index) {
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

            Self::Component(component_column) => {
                component_column.data_ui(ctx, ui, latest_at_query, row_index, instance_index);
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
        data: impl Iterator<Item = (ColumnDescriptorRef<'a>, &'a ColumnBlueprint, ArrowArrayRef)>,
    ) -> Result<Self, DisplayRecordBatchError> {
        let mut num_rows = None;
        let mut batch_row_ids = None;

        let mut columns = data
            .map(|(column_descriptor, column_blueprint, column_data)| {
                if num_rows.is_none() {
                    num_rows = Some(column_data.len());
                }

                let column =
                    DisplayColumn::try_new(&column_descriptor, column_blueprint, &column_data);

                // find the batch row ids, if any
                if batch_row_ids.is_none()
                    && let Ok(DisplayColumn::RowId { row_ids }) = &column
                {
                    batch_row_ids = Some(Arc::clone(row_ids));
                }

                column
            })
            .collect::<Result<Vec<DisplayColumn>, _>>()?;

        // If we have row_ids, give a reference to all component columns.
        if let Some(batch_row_ids) = batch_row_ids {
            for column in &mut columns {
                if let DisplayColumn::Component(column) = column {
                    column.row_ids = Some(Arc::clone(&batch_row_ids));
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
