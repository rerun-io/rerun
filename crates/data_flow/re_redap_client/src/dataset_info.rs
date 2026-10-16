//! An overview of a dataset, aggregated from its segment table and schema.

use std::collections::{BTreeMap, BTreeSet};

use arrow::array::{Array as _, Int64Array, RecordBatch};
use arrow::datatypes::Schema as ArrowSchema;
use re_chunk::TimeColumn;
use re_chunk_index::RawRrdManifest;
use re_log_types::{AbsoluteTimeRange, TimeInt, TimeType, TimelineName};
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_sorbet::{ColumnDescriptor, ComponentColumnDescriptor, SorbetColumnDescriptors};
use re_types_core::LayerName;

use crate::{ApiError, ApiResult, TraceId};

/// An overview of a dataset: its segments, layers, timelines, contents and properties.
///
/// Built from the segment table and the dataset schema, without reading any recording data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetInfo {
    pub num_segments: u64,

    /// Sum of the segment table's per-segment sizes.
    pub size_bytes: u64,

    /// Sorted by name.
    pub layers: Vec<LayerInfo>,

    /// Sorted by name.
    pub timelines: Vec<TimelineInfo>,

    /// Entity paths in the dataset schema, excluding properties.
    pub num_entities: u64,

    /// Component columns in the dataset schema, excluding properties.
    pub num_component_columns: u64,

    /// The segment table's property columns, sorted.
    pub property_columns: Vec<ComponentColumnDescriptor>,
}

/// One layer of a dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerInfo {
    pub name: LayerName,

    /// Distinct segments that have this layer.
    pub num_segments: u64,
}

/// One timeline of a dataset, aggregated over its segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineInfo {
    pub name: TimelineName,
    pub typ: TimeType,

    /// Segments with data on this timeline. May be 0.
    pub num_segments: u64,

    /// Union of the per-segment ranges. `None` when `num_segments == 0`.
    pub range: Option<AbsoluteTimeRange>,
}

/// Folds `/ScanSegmentTable` batches into a [`DatasetInfo`].
///
/// Scan the segment table with [`Self::columns`] as the projection, feed every batch to
/// [`Self::add_batch`], then call [`Self::finish`].
pub struct DatasetInfoBuilder {
    timelines: Vec<TimelineAccumulator>,
    property_columns: Vec<ComponentColumnDescriptor>,

    num_segments: u64,
    size_bytes: u64,
    layers: BTreeMap<LayerName, u64>,
}

struct TimelineAccumulator {
    name: TimelineName,
    typ: TimeType,
    start_column: String,
    end_column: String,
    num_segments: u64,
    min: Option<i64>,
    max: Option<i64>,
}

impl DatasetInfoBuilder {
    pub fn new(segment_table_schema: &ArrowSchema) -> Self {
        let timelines = RawRrdManifest::global_index_ranges(segment_table_schema.fields())
            .filter_map(|(name, start_field, end_field)| {
                let Some(typ) = TimeType::from_arrow_datatype(start_field.data_type()) else {
                    re_log::warn_once!(
                        "Ignoring timeline {name:?} with unsupported datatype {}",
                        start_field.data_type()
                    );
                    return None;
                };
                let name = match TimelineName::try_new(name) {
                    Ok(name) => name,
                    Err(err) => {
                        re_log::warn_once!("Ignoring timeline with invalid name: {err}");
                        return None;
                    }
                };
                Some(TimelineAccumulator {
                    name,
                    typ,
                    start_column: start_field.name().clone(),
                    end_column: end_field.name().clone(),
                    num_segments: 0,
                    min: None,
                    max: None,
                })
            })
            .collect();

        let mut property_columns: Vec<_> = segment_table_schema
            .fields()
            .iter()
            .filter(|field| !RawRrdManifest::is_index(field))
            .filter_map(
                |field| match ColumnDescriptor::try_from_arrow_field(None, field) {
                    Ok(ColumnDescriptor::Component(column)) if column.entity_path.is_property() => {
                        Some(column)
                    }
                    _ => None,
                },
            )
            .collect();
        property_columns.sort();

        Self {
            timelines,
            property_columns,
            num_segments: 0,
            size_bytes: 0,
            layers: BTreeMap::new(),
        }
    }

    /// The segment table columns to project the scan onto.
    pub fn columns(&self) -> Vec<&str> {
        let mut columns = vec![
            ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES_NAME,
            ScanSegmentTableDataframe::COLUMN_RERUN_SIZE_BYTES_NAME,
        ];
        for timeline in &self.timelines {
            columns.push(&timeline.start_column);
            columns.push(&timeline.end_column);
        }
        columns
    }

    pub fn add_batch(
        &mut self,
        origin: &re_uri::Origin,
        trace_id: Option<TraceId>,
        batch: &RecordBatch,
    ) -> ApiResult<()> {
        let decode_failed =
            |err| ApiError::deserialization_quiver_from(origin, trace_id, err, "/ScanSegmentTable");

        self.num_segments += batch.num_rows() as u64;

        let layer_names = ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES
            .extract(batch)
            .map_err(decode_failed)?;
        for segment_layers in layer_names.iter_owned() {
            for layer in segment_layers.into_iter().collect::<BTreeSet<_>>() {
                *self.layers.entry(layer).or_default() += 1;
            }
        }

        let sizes = ScanSegmentTableDataframe::COLUMN_RERUN_SIZE_BYTES
            .extract(batch)
            .map_err(decode_failed)?;
        self.size_bytes += sizes.iter_owned().sum::<u64>();

        for timeline in &mut self.timelines {
            let starts = index_values(origin, trace_id, batch, &timeline.start_column)?;
            let ends = index_values(origin, trace_id, batch, &timeline.end_column)?;

            timeline.num_segments += (starts.len() - starts.null_count()) as u64;
            timeline.min = std::iter::chain(timeline.min, arrow::compute::min(&starts)).min();
            timeline.max = std::iter::chain(timeline.max, arrow::compute::max(&ends)).max();
        }

        Ok(())
    }

    pub fn finish(
        self,
        origin: &re_uri::Origin,
        dataset_schema: &ArrowSchema,
    ) -> ApiResult<DatasetInfo> {
        let Self {
            timelines,
            property_columns,
            num_segments,
            size_bytes,
            layers,
        } = self;

        let columns = SorbetColumnDescriptors::try_from_arrow_fields(None, dataset_schema.fields())
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    origin,
                    None,
                    err,
                    "failed parsing the dataset schema",
                )
            })?;
        let component_columns: Vec<_> = columns
            .component_columns()
            .filter(|column| !column.entity_path.is_property())
            .collect();
        let entities: BTreeSet<_> = component_columns
            .iter()
            .map(|column| &column.entity_path)
            .collect();

        Ok(DatasetInfo {
            num_segments,
            size_bytes,
            layers: layers
                .into_iter()
                .map(|(name, num_segments)| LayerInfo { name, num_segments })
                .collect(),
            timelines: timelines
                .into_iter()
                .map(|timeline| TimelineInfo {
                    name: timeline.name,
                    typ: timeline.typ,
                    num_segments: timeline.num_segments,
                    range: Option::zip(timeline.min, timeline.max).map(|(min, max)| {
                        AbsoluteTimeRange::new(
                            TimeInt::saturated_temporal_i64(min),
                            TimeInt::saturated_temporal_i64(max),
                        )
                    }),
                })
                .collect(),
            num_entities: entities.len() as u64,
            num_component_columns: component_columns.len() as u64,
            property_columns,
        })
    }
}

/// An index range column as raw `i64`, whatever its time type.
fn index_values(
    origin: &re_uri::Origin,
    trace_id: Option<TraceId>,
    batch: &RecordBatch,
    column_name: &str,
) -> ApiResult<Int64Array> {
    let column = batch.column_by_name(column_name).ok_or_else(|| {
        ApiError::deserialization(
            origin,
            trace_id,
            format!("/ScanSegmentTable response is missing column {column_name:?}"),
        )
    })?;

    let (values, nulls) = TimeColumn::read_nullable_array(column).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            format!("failed decoding /ScanSegmentTable column {column_name:?}"),
        )
    })?;

    Ok(Int64Array::new(values, nulls))
}
