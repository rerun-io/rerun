use std::collections::BTreeMap;

use arrow::array::{
    Array as _, ArrayRef, BooleanArray, FixedSizeBinaryArray, RecordBatch, StringArray,
};
use arrow::error::ArrowError;
use itertools::izip;
use re_arrow_util::{ArrowArrayDowncastRef as _, RecordBatchExt as _, WrongDatatypeError};

use crate::{
    ArchetypeName, ChunkId, ComponentDescriptor, ComponentIdentifier, ComponentType, TimelineName,
};

pub const FIELD_CHUNK_ID: &str = "chunk_id";
pub const FIELD_CHUNK_IS_STATIC: &str = "chunk_is_static";
pub const FIELD_CHUNK_ENTITY_PATH: &str = "chunk_entity_path";
pub const FIELD_CHUNK_BYTE_OFFSET: &str = "chunk_byte_offset";
pub const FIELD_CHUNK_BYTE_SIZE: &str = "chunk_byte_size";
pub const FIELD_CHUNK_KEY: &str = "chunk_key";

// -----------------------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum RrdManifestError {
    #[error(transparent)]
    Arrow(#[from] ArrowError),

    #[error(transparent)]
    MissingColumn(#[from] re_arrow_util::MissingColumnError),

    #[error(transparent)]
    WrongDatatype(#[from] WrongDatatypeError),

    #[error("Found nulls in column {column_name:?}")]
    UnexpectedNulls { column_name: String },

    #[error("{0}")]
    Custom(String),
}

impl RrdManifestError {
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }
}

// -----------------------------------------------------------------------------------------

/// Start/end times (inclusive).
#[derive(Clone)]
pub struct TimeRange {
    /// Inclusive
    pub start: ArrayRef,

    /// Inclusive
    pub end: ArrayRef,
}

/// Used during building of [`TimeRange`].
#[derive(Default)]
struct TimeRangeOpt {
    start: Option<ArrayRef>,
    end: Option<ArrayRef>,
}

impl TryFrom<TimeRangeOpt> for TimeRange {
    type Error = RrdManifestError;

    fn try_from(value: TimeRangeOpt) -> Result<Self, Self::Error> {
        Ok(Self {
            start: value
                .start
                .ok_or_else(|| RrdManifestError::custom("Missing start array in TimeRange"))?,
            end: value
                .end
                .ok_or_else(|| RrdManifestError::custom("Missing end array in TimeRange"))?,
        })
    }
}

/// Communicates the chunks in a store (recording) without actually holding the chunks.
///
/// This is sent from the server to the client/viewer.
///
///
/// ## Example (transposed)
/// See schema in `crates/store/re_log_encoding/tests/snapshots/footers_and_manifests__rrd_manifest_blueprint_schema.snap`
///
/// ```text
/// ┌─────────────────────────────────────────┬──────────────────────────────────────────┬──────────────────────────────────────────┐
/// │ chunk_entity_path                       ┆ /my/entity                               ┆ /my/entity                               │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_id                                ┆ 00000000000000010000000000000001         ┆ 00000000000000010000000000000002         │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_is_static                         ┆ false                                    ┆ true                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:colors:has_static_data ┆ false                                    ┆ false                                    │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:labels:has_static_data ┆ false                                    ┆ true                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ example_MyPoints:points:has_static_data ┆ false                                    ┆ false                                    │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:start                          ┆ 10                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:end                            ┆ 40                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:colors:start  ┆ 10                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:colors:end    ┆ 40                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:points:start  ┆ 10                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ frame_nr:example_MyPoints:points:end    ┆ 40                                       ┆ null                                     │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ chunk_key                               ┆ 010000000000000001000000000000000a00000… ┆ 010000000000000002000000000000000a00000… │
/// └─────────────────────────────────────────┴──────────────────────────────────────────┴──────────────────────────────────────────┘
/// ```
#[derive(Clone)]
pub struct RrdManifestMessage {
    rb: RecordBatch,

    chunk_entity_path: StringArray,

    chunk_id: FixedSizeBinaryArray,

    chunk_is_static: BooleanArray,

    chunk_range: BTreeMap<TimelineName, TimeRange>,
    has_static_data: BTreeMap<ComponentDescriptor, BooleanArray>,
    per_comp_range: BTreeMap<(TimelineName, ComponentDescriptor), TimeRange>,
}

impl std::fmt::Debug for RrdManifestMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkIndexMessage")
            .field("num_chunks", &self.num_rows())
            .finish()
    }
}

impl RrdManifestMessage {
    pub fn try_from_record_batch(rb: RecordBatch) -> Result<Self, RrdManifestError> {
        let chunk_entity_path = rb
            .try_get_column(FIELD_CHUNK_ENTITY_PATH)?
            .try_downcast_array::<StringArray>()?;

        let chunk_id = rb
            .try_get_column("chunk_id")?
            .try_downcast_array::<FixedSizeBinaryArray>()?;
        ChunkId::try_slice_from_arrow(&chunk_id)?; // Validate once!

        let chunk_is_static = rb
            .try_get_column(FIELD_CHUNK_IS_STATIC)?
            .try_downcast_array::<BooleanArray>()?;
        if chunk_is_static.null_count() != 0 {
            return Err(RrdManifestError::UnexpectedNulls {
                column_name: FIELD_CHUNK_IS_STATIC.into(),
            });
        }

        let mut chunk_range: BTreeMap<TimelineName, TimeRangeOpt> = Default::default();
        let mut has_static_data: BTreeMap<ComponentDescriptor, BooleanArray> = Default::default();
        let mut per_comp_range: BTreeMap<(TimelineName, ComponentDescriptor), TimeRangeOpt> =
            Default::default();

        // TODO(emilk): parse all the other columns
        for (field, column) in izip!(rb.schema().fields(), rb.columns()) {
            let is_special_field = matches!(
                field.name().as_str(),
                FIELD_CHUNK_ENTITY_PATH
                    | FIELD_CHUNK_ID
                    | FIELD_CHUNK_IS_STATIC
                    | FIELD_CHUNK_BYTE_OFFSET
                    | FIELD_CHUNK_BYTE_SIZE
                    | FIELD_CHUNK_KEY
            );
            if !is_special_field {
                let archetype = field.metadata().get("rerun:archetype");
                let component = field.metadata().get("rerun:component");
                let component_type = field.metadata().get("rerun:component_type");
                let index = field
                    .metadata()
                    .get("rerun:index")
                    .map(|t| TimelineName::from(t.clone()));

                let comp_descr = component.map(|component| ComponentDescriptor {
                    archetype: archetype.map(|a| ArchetypeName::from(a.clone())),
                    component: ComponentIdentifier::new(component),
                    component_type: component_type.map(|c| ComponentType::from(c.clone())),
                });

                if let Some(index) = index {
                    if let Some(comp_descr) = comp_descr {
                        let range = per_comp_range
                            .entry((index, comp_descr.clone()))
                            .or_default();
                        if field.name().ends_with(":start") {
                            range.start = Some(column.clone());
                        } else if field.name().ends_with(":end") {
                            range.end = Some(column.clone());
                        } else if field.name().ends_with(":has_static_data") {
                            has_static_data.insert(comp_descr, column.try_downcast_array()?);
                        } else {
                            re_log::warn_once!("Unknown RrdManifest column: {field}");
                        }
                    } else {
                        let timeline_range = chunk_range.entry(index).or_default();
                        if field.name().ends_with(":start") {
                            timeline_range.start = Some(column.clone());
                        } else if field.name().ends_with(":end") {
                            timeline_range.end = Some(column.clone());
                        } else {
                            re_log::warn_once!("Unknown RrdManifest column: {field}");
                        }
                    }
                } else {
                    re_log::warn_once!("Unknown RrdManifest column: {field}");
                }
            }
        }

        let chunk_range = chunk_range
            .into_iter()
            .map(|(k, v)| Ok((k, v.try_into()?)))
            .collect::<Result<_, RrdManifestError>>()?;
        let per_comp_range = per_comp_range
            .into_iter()
            .map(|(k, v)| Ok((k, v.try_into()?)))
            .collect::<Result<_, RrdManifestError>>()?;

        Ok(Self {
            rb,
            chunk_entity_path,
            chunk_id,
            chunk_is_static,
            chunk_range,
            has_static_data,
            per_comp_range,
        })
    }

    /// Give the inner record batch.
    pub fn record_batch(&self) -> &RecordBatch {
        &self.rb
    }

    /// How many chunks are there in the manifest?
    pub fn num_rows(&self) -> usize {
        self.rb.num_rows()
    }

    /// What components is in the RRD?
    pub fn components(&self) -> impl Iterator<Item = &ComponentDescriptor> {
        self.has_static_data.keys()
    }

    /// What timelines are in the RRD?
    pub fn timelines(&self) -> impl Iterator<Item = &TimelineName> {
        self.chunk_range.keys()
    }

    /// All the chunks in this index
    pub fn chunk_id(&self) -> &[ChunkId] {
        #[expect(clippy::unwrap_used)] // Validated in constructor
        ChunkId::try_slice_from_arrow(&self.chunk_id).unwrap()
    }

    /// The entity path of each chunk
    pub fn chunk_entity_path(&self) -> &StringArray {
        &self.chunk_entity_path
    }

    /// Is a given chunk static (as opposed to temporal)?
    pub fn chunk_is_static(&self) -> impl Iterator<Item = bool> {
        self.chunk_is_static.iter().map(|b| b.unwrap_or_default()) // we've validated that there are no nulls
    }

    /// When does a given chunk have data?
    pub fn timeline_range(&self, timeline: &TimelineName) -> Option<&TimeRange> {
        self.chunk_range.get(timeline)
    }

    /// Does this component have static data?
    pub fn has_static_data(&self, component: &ComponentDescriptor) -> Option<&BooleanArray> {
        self.has_static_data.get(component)
    }

    /// What time range does a given chunk have data for this particular component?
    pub fn component_time_range(
        &self,
        timeline: &TimelineName,
        component: &ComponentDescriptor,
    ) -> Option<&TimeRange> {
        self.per_comp_range.get(&(*timeline, component.clone()))
    }
}
