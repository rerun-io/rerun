//! Request and response ext types for querying chunks.
//!
//! Covers `QueryDataset` (deprecated) and `FindChunks`, its replacement. The adapter that
//! answers `QueryDataset` through `FindChunks` lives in the sibling `legacy_adapter` module.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use arrow::array::{Int64Array, RecordBatch};
use arrow::datatypes::{DataType, Field, FieldRef};
use arrow::error::ArrowError;
use itertools::Itertools as _;
use re_chunk::TimelineName;
use re_log_types::{EntityPath, TimeInt};
use re_types_core::LayerName;

use crate::TypeConversionError;
use crate::cloud::v1alpha1::ext::{Query, QueryDatasetDataframe};
use crate::cloud::v1alpha1::{
    FetchChunksRequest, QueryDatasetResponse, ScanDatasetManifestRequest, ScanSegmentTableRequest,
};
use crate::common::v1alpha1::ext as common_ext;
use crate::common::v1alpha1::ext::SegmentId;

// --- QueryDatasetRequest ---

#[derive(Debug, Clone)]
pub struct QueryDatasetRequest {
    pub segment_ids: Vec<common_ext::SegmentId>,
    pub generate_direct_urls: bool,
    pub unsigned_direct_urls: bool,
    pub chunk_ids: Vec<re_chunk::ChunkId>,
    pub entity_paths: Vec<EntityPath>,
    pub select_all_entity_paths: bool,
    pub fuzzy_descriptors: Vec<String>,
    pub exclude_static_data: bool,
    pub exclude_temporal_data: bool,
    pub scan_parameters: Option<common_ext::ScanParameters>,
    pub query: Option<Query>,
}

impl Default for QueryDatasetRequest {
    fn default() -> Self {
        Self {
            segment_ids: vec![],
            chunk_ids: vec![],
            entity_paths: vec![],
            select_all_entity_paths: true,
            fuzzy_descriptors: vec![],
            exclude_static_data: false,
            exclude_temporal_data: false,
            scan_parameters: None,
            query: None,
            generate_direct_urls: false,
            unsigned_direct_urls: false,
        }
    }
}

impl From<QueryDatasetRequest> for crate::cloud::v1alpha1::QueryDatasetRequest {
    fn from(value: QueryDatasetRequest) -> Self {
        Self {
            segment_ids: value.segment_ids.into_iter().map(Into::into).collect(),
            chunk_ids: value
                .chunk_ids
                .into_iter()
                .map(|chunk_id| chunk_id.as_tuid().into())
                .collect(),
            entity_paths: value.entity_paths.into_iter().map(Into::into).collect(),
            select_all_entity_paths: value.select_all_entity_paths,
            fuzzy_descriptors: value.fuzzy_descriptors,
            exclude_static_data: value.exclude_static_data,
            exclude_temporal_data: value.exclude_temporal_data,
            scan_parameters: value.scan_parameters.map(Into::into),
            query: value.query.map(Into::into),
            generate_direct_urls: value.generate_direct_urls,
            unsigned_direct_urls: value.unsigned_direct_urls,
        }
    }
}

impl TryFrom<crate::cloud::v1alpha1::QueryDatasetRequest> for QueryDatasetRequest {
    type Error = tonic::Status;

    fn try_from(value: crate::cloud::v1alpha1::QueryDatasetRequest) -> Result<Self, Self::Error> {
        // Support both segment_ids (new) and partition_ids (deprecated) for backward compatibility
        let segment_ids = value
            .segment_ids
            .into_iter()
            .map(TryInto::try_into)
            .try_collect()?;

        let result = Self {
            segment_ids,

            chunk_ids: value
                .chunk_ids
                .into_iter()
                .map(|tuid| {
                    let id: re_tuid::Tuid = tuid.try_into()?;
                    Ok::<_, tonic::Status>(re_chunk::ChunkId::from_u128(id.as_u128()))
                })
                .try_collect()?,

            entity_paths: value
                .entity_paths
                .into_iter()
                .map(|path| {
                    path.try_into().map_err(|err| {
                        tonic::Status::invalid_argument(format!("invalid entity path: {err}"))
                    })
                })
                .try_collect()?,

            select_all_entity_paths: value.select_all_entity_paths,

            fuzzy_descriptors: value.fuzzy_descriptors,

            exclude_static_data: value.exclude_static_data,
            exclude_temporal_data: value.exclude_temporal_data,

            scan_parameters: value
                .scan_parameters
                .map(|params| params.try_into())
                .transpose()?,

            query: value.query.map(|q| q.try_into()).transpose()?,

            generate_direct_urls: value.generate_direct_urls,
            unsigned_direct_urls: value.unsigned_direct_urls,
        };

        if let Some(query) = result.query.as_ref()
            && let Some(la) = query.latest_at.as_ref()
            && !la.per_segment_values.is_empty()
        {
            // Per `cloud.proto`: `per_segment_values` is mutually exclusive
            // with both `latest_at.at` and `query.range`, and requires
            // `latest_at.index` to be set. The server reconstructs global
            // bounds internally from the per-segment values, so a
            // caller-supplied `at` or `range` would be ambiguous, and a
            // missing `index` would leave the per-segment values without a
            // timeline to apply to. Without these checks, the two backends
            // diverged: dataplatform errored on missing `index` while OSS
            // silently degraded to a static-only fallback (and OSS kept
            // `range` while dataplatform overwrote it) — same illegal
            // request, backend-divergent behavior.
            if la.index.is_none() {
                return Err(tonic::Status::invalid_argument(
                    "`latest_at.index` must be set when `per_segment_values` is non-empty",
                ));
            }
            if la.at != re_log_types::TimeInt::STATIC {
                return Err(tonic::Status::invalid_argument(
                    "`latest_at.at` must be unset when `per_segment_values` is non-empty",
                ));
            }
            if query.range.is_some() {
                return Err(tonic::Status::invalid_argument(
                    "`query.range` must be unset when `per_segment_values` is non-empty",
                ));
            }
            if result.segment_ids.is_empty() {
                return Err(tonic::Status::invalid_argument(
                    "`per_segment_values` requires `segment_ids` to be non-empty",
                ));
            }
            if la.per_segment_values.len() != result.segment_ids.len() {
                return Err(tonic::Status::invalid_argument(format!(
                    "`per_segment_values.len()` ({}) must equal `segment_ids.len()` ({})",
                    la.per_segment_values.len(),
                    result.segment_ids.len(),
                )));
            }
            let mut seen = HashSet::with_capacity(result.segment_ids.len());
            for sid in &result.segment_ids {
                if !seen.insert(sid) {
                    return Err(tonic::Status::invalid_argument(
                        "`segment_ids` must contain no duplicates when `per_segment_values` is set",
                    ));
                }
            }
        }

        Ok(result)
    }
}

// --- FindChunksRequest ---

/// Selects the entities and components a `FindChunks` request returns candidates for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentSelection {
    /// Every entity and component.
    All,

    /// Exactly the listed entities. An empty list selects nothing.
    Explicit(Vec<EntitySelection>),
}

/// One entity and the components to include for it, within a [`ContentSelection::Explicit`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySelection {
    /// The entity to select. An entity the dataset does not have selects nothing.
    ///
    /// The wire conversion rejects a path listed twice in one [`ContentSelection::Explicit`].
    pub entity_path: EntityPath,

    /// The components of `entity_path` to select.
    pub components: ComponentSelection,
}

/// Selects the components of one entity, within an [`EntitySelection`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentSelection {
    /// Every component logged on this entity.
    All,

    /// Exactly the listed component identifiers, e.g. `Points3D:positions`.
    ///
    /// An identifier the entity does not log selects nothing. An empty set selects nothing for
    /// this entity.
    Explicit(BTreeSet<String>),
}

/// Whether a [`ChunkMode::Range`] or [`ChunkMode::Samples`] selection also selects the chunks
/// that resolve state from data outside the selected times.
///
/// Ext type for [`crate::cloud::v1alpha1::FillMode`]. The wire conversion accepts every defined
/// mode, including the ones no server implements: see [`FindChunksRequest::ensure_supported`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillMode {
    /// Select only the chunks with data in the range, or at the sample times.
    ///
    /// `FILL_MODE_UNSPECIFIED` converts to this mode.
    None,

    /// Also select the chunks that resolve latest-at state at a range's `start_inclusive`, or at
    /// each sample time. A range with this mode requires `start_inclusive`.
    LatestAt,

    /// Also select the chunks that resolve state from later data, at a range's `end_inclusive`, or
    /// at each sample time.
    ///
    /// No server implements this mode yet: [`FindChunksRequest::ensure_supported`] rejects it.
    EarliestAt,
}

impl FillMode {
    /// Whether this mode also selects the chunks that resolve latest-at state.
    ///
    /// Fails with `Unimplemented` naming `field` for a mode no server implements.
    pub fn includes_latest_at(self, field: &str) -> tonic::Result<bool> {
        match self {
            Self::None => Ok(false),
            Self::LatestAt => Ok(true),
            Self::EarliestAt => Err(earliest_at_unimplemented(field)),
        }
    }
}

fn earliest_at_unimplemented(field: &str) -> tonic::Status {
    tonic::Status::unimplemented(format!(
        "`{field}` `FILL_MODE_EARLIEST_AT` is not implemented"
    ))
}

impl From<FillMode> for crate::cloud::v1alpha1::FillMode {
    fn from(value: FillMode) -> Self {
        match value {
            FillMode::None => Self::None,
            FillMode::LatestAt => Self::LatestAt,
            FillMode::EarliestAt => Self::EarliestAt,
        }
    }
}

/// Temporal bounds of a [`ChunkMode::Range`] selection.
///
/// Both bounds are inclusive, and a missing bound is unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeSelection {
    /// The timeline the bounds apply to.
    ///
    /// Servers reject a timeline the dataset does not have, unless the dataset has no data.
    pub timeline: TimelineName,

    /// The inclusive lower bound, or unbounded if `None`. Never [`TimeInt::STATIC`].
    pub start_inclusive: Option<TimeInt>,

    /// The inclusive upper bound, or unbounded if `None`. Never before `start_inclusive`, and never
    /// [`TimeInt::STATIC`].
    pub end_inclusive: Option<TimeInt>,

    /// Whether to also select the chunks that resolve state at a bound.
    pub fill_mode: FillMode,
}

/// Selects every segment of the dataset, or an explicit list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentSelection {
    /// Every segment of the dataset.
    All,

    /// Exactly the listed segments. An empty set selects nothing, and an id the dataset does not
    /// have selects nothing.
    Explicit(BTreeSet<SegmentId>),
}

/// The selection mode of a [`FindChunksRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkMode {
    /// The static chunks of the selected segments.
    Static {
        /// The segments to select from.
        segments: SegmentSelection,
    },

    /// The chunks that overlap a time range on one timeline, plus every static chunk of the
    /// selected contents and segments, whatever the bounds and the fill mode.
    Range {
        /// The segments to select from.
        segments: SegmentSelection,

        /// The timeline and bounds of the range.
        range: RangeSelection,
    },

    /// The chunks with data at each sample time, per segment, plus every static chunk of the
    /// selected contents in each segment of the map, whatever the fill mode.
    ///
    /// Only segments in the map are selected.
    Samples {
        /// The timeline of the sample times. Servers reject a timeline the dataset does not have,
        /// unless the dataset has no data.
        timeline: TimelineName,

        /// The sample times of each selected segment. Sample times are never [`TimeInt::STATIC`].
        ///
        /// A segment with no sample times selects nothing, not even its static chunks.
        segments: BTreeMap<SegmentId, BTreeSet<TimeInt>>,

        /// Whether to also select the chunks that resolve state at each sample time.
        fill_mode: FillMode,
    },
}

impl ChunkMode {
    /// Whether this mode selects chunks from `segment_id`.
    pub fn selects_segment(&self, segment_id: &SegmentId) -> bool {
        match self {
            Self::Static { segments } | Self::Range { segments, .. } => match segments {
                SegmentSelection::All => true,
                SegmentSelection::Explicit(ids) => ids.contains(segment_id),
            },
            Self::Samples { segments, .. } => segments.contains_key(segment_id),
        }
    }
}

/// Which direct chunk URLs a `FindChunks` response carries, and how they are signed.
///
/// The mode is an upper bound: a server may return fewer URLs than requested, or none.
///
/// Ext type for [`crate::cloud::v1alpha1::DirectUrlMode`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DirectUrlMode {
    /// No direct URLs and no expiration times.
    Omit,

    /// Unsigned URLs, without expiration times. The client supplies object-store credentials.
    Unsigned,

    /// Server-signed URLs, usable without object-store credentials, with their expiration times.
    #[default]
    Signed,
}

/// Ext type for [`crate::cloud::v1alpha1::FindChunksRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindChunksRequest {
    /// The entities and components to select.
    pub contents: ContentSelection,

    /// Which chunks of the selected contents to select, and from which segments.
    pub mode: ChunkMode,

    /// Which direct chunk URLs the response carries.
    pub direct_url_mode: DirectUrlMode,
}

impl FindChunksRequest {
    /// The selection timeline of this request, or `None` for a [`ChunkMode::Static`] selection.
    pub fn timeline(&self) -> Option<TimelineName> {
        match &self.mode {
            ChunkMode::Static { .. } => None,
            ChunkMode::Range { range, .. } => Some(range.timeline),
            ChunkMode::Samples { timeline, .. } => Some(*timeline),
        }
    }

    /// Fails with `Unimplemented` if this request uses a mode that no server implements yet.
    ///
    /// The wire conversion accepts every defined mode, so that a malformed request fails with
    /// `InvalidArgument` first. Servers must call this right after the conversion, before any
    /// shortcut for an empty selection or any dataset work, so an unsupported request fails the
    /// same way whatever it selects.
    pub fn ensure_supported(&self) -> tonic::Result<()> {
        match &self.mode {
            ChunkMode::Static { .. } => {}
            ChunkMode::Range { range, .. } => {
                range.fill_mode.includes_latest_at("range.fill_mode")?;
            }
            ChunkMode::Samples { fill_mode, .. } => {
                fill_mode.includes_latest_at("samples.fill_mode")?;
            }
        }
        Ok(())
    }

    /// Whether this request selects no chunks at all.
    ///
    /// Explicit contents select nothing when every entity in them has an empty component set,
    /// including when they list no entity.
    pub fn is_empty(&self) -> bool {
        let contents_empty = match &self.contents {
            ContentSelection::All => false,
            ContentSelection::Explicit(entities) => entities.iter().all(|entity| {
                matches!(&entity.components, ComponentSelection::Explicit(ids) if ids.is_empty())
            }),
        };
        if contents_empty {
            return true;
        }

        match &self.mode {
            ChunkMode::Static { segments } | ChunkMode::Range { segments, .. } => {
                matches!(segments, SegmentSelection::Explicit(ids) if ids.is_empty())
            }
            ChunkMode::Samples { segments, .. } => segments.is_empty(),
        }
    }
}

/// Fails with `InvalidArgument` naming `field` if `value` is `None`.
fn required_field<T>(value: Option<T>, field: &str) -> Result<T, tonic::Status> {
    value.ok_or_else(|| tonic::Status::invalid_argument(format!("`{field}` is required")))
}

/// Rejects `i64::MIN`, the wire sentinel for [`TimeInt::STATIC`], which is illegal wherever a
/// temporal time value is expected.
fn try_time_value(raw: i64, field: &str) -> Result<TimeInt, tonic::Status> {
    if raw == i64::MIN {
        return Err(tonic::Status::invalid_argument(format!(
            "`{field}` must not be `i64::MIN` (the static sentinel)"
        )));
    }
    Ok(TimeInt::new_temporal(raw))
}

fn try_content_selection(
    proto: crate::cloud::v1alpha1::ContentSelection,
) -> Result<ContentSelection, tonic::Status> {
    use crate::cloud::v1alpha1::content_selection::Selection;

    let selection = required_field(proto.selection, "contents.selection")?;

    Ok(match selection {
        Selection::All(()) => ContentSelection::All,
        Selection::Explicit(entities) => {
            let mut seen = HashSet::with_capacity(entities.entities.len());
            let mut selections = Vec::with_capacity(entities.entities.len());

            for entity in entities.entities {
                let crate::cloud::v1alpha1::EntitySelection {
                    entity_path,
                    components,
                } = entity;

                let entity_path: EntityPath = required_field(entity_path, "contents.entity_path")?
                    .try_into()
                    .map_err(|err: TypeConversionError| {
                        tonic::Status::invalid_argument(format!("invalid entity path: {err}"))
                    })?;

                if !seen.insert(entity_path.clone()) {
                    return Err(tonic::Status::invalid_argument(format!(
                        "duplicate entity path in `contents`: {entity_path}"
                    )));
                }

                let components =
                    try_component_selection(required_field(components, "contents.components")?)?;

                selections.push(EntitySelection {
                    entity_path,
                    components,
                });
            }

            // Drop entities whose component set is empty only after every entry has been
            // validated, so a later duplicate or invalid id is still rejected.
            selections.retain(|entity| {
                !matches!(&entity.components, ComponentSelection::Explicit(ids) if ids.is_empty())
            });

            ContentSelection::Explicit(selections)
        }
    })
}

fn try_component_selection(
    proto: crate::cloud::v1alpha1::entity_selection::Components,
) -> Result<ComponentSelection, tonic::Status> {
    use crate::cloud::v1alpha1::entity_selection::Components;

    Ok(match proto {
        Components::All(()) => ComponentSelection::All,
        Components::Explicit(ids) => {
            let mut set = BTreeSet::new();
            for id in ids.ids {
                if id.is_empty() {
                    return Err(tonic::Status::invalid_argument(
                        "`contents.components.explicit.ids` entries must be non-empty",
                    ));
                }
                set.insert(id);
            }
            ComponentSelection::Explicit(set)
        }
    })
}

fn try_segment_selection(
    proto: crate::cloud::v1alpha1::SegmentSelection,
) -> Result<SegmentSelection, tonic::Status> {
    use crate::cloud::v1alpha1::segment_selection::Selection;

    let selection = required_field(proto.selection, "segments.selection")?;

    Ok(match selection {
        Selection::All(()) => SegmentSelection::All,
        Selection::Explicit(ids) => {
            let ids = ids.ids.into_iter().map(TryInto::try_into).try_collect()?;
            SegmentSelection::Explicit(ids)
        }
    })
}

fn try_range_bounds(
    timeline: String,
    start_inclusive: Option<i64>,
    end_inclusive: Option<i64>,
    fill_mode: FillMode,
) -> Result<RangeSelection, tonic::Status> {
    let timeline = TimelineName::try_new(timeline)
        .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;

    let start_inclusive = start_inclusive
        .map(|raw| try_time_value(raw, "range.start_inclusive"))
        .transpose()?;
    let end_inclusive = end_inclusive
        .map(|raw| try_time_value(raw, "range.end_inclusive"))
        .transpose()?;

    if let (Some(start), Some(end)) = (start_inclusive, end_inclusive)
        && end < start
    {
        return Err(tonic::Status::invalid_argument(
            "`range.end_inclusive` must not be before `range.start_inclusive`",
        ));
    }

    let requires_start = match fill_mode {
        FillMode::LatestAt => true,
        FillMode::None | FillMode::EarliestAt => false,
    };
    if requires_start && start_inclusive.is_none() {
        return Err(tonic::Status::invalid_argument(
            "`range.fill_mode` `FILL_MODE_LATEST_AT` requires `range.start_inclusive` to be set",
        ));
    }

    Ok(RangeSelection {
        timeline,
        start_inclusive,
        end_inclusive,
        fill_mode,
    })
}

/// Parses the wire fill mode of the selection field `field`.
///
/// Accepts every defined mode, including the ones no server implements, so a malformed request
/// fails with `InvalidArgument` before [`FindChunksRequest::ensure_supported`] runs.
fn try_fill_mode(raw: i32, field: &str) -> Result<FillMode, tonic::Status> {
    use crate::cloud::v1alpha1::FillMode as WireFillMode;

    let fill_mode = WireFillMode::try_from(raw).map_err(|_err| {
        tonic::Status::invalid_argument(format!("`{field}` has unknown value {raw}"))
    })?;

    Ok(match fill_mode {
        WireFillMode::Unspecified | WireFillMode::None => FillMode::None,
        WireFillMode::LatestAt => FillMode::LatestAt,
        WireFillMode::EarliestAt => FillMode::EarliestAt,
    })
}

fn try_direct_url_mode(raw: i32) -> Result<DirectUrlMode, tonic::Status> {
    use crate::cloud::v1alpha1::DirectUrlMode as WireMode;

    let mode = WireMode::try_from(raw).map_err(|_err| {
        tonic::Status::invalid_argument(format!("`direct_url_mode` has unknown value {raw}"))
    })?;

    Ok(match mode {
        WireMode::Unspecified | WireMode::Signed => DirectUrlMode::Signed,
        WireMode::Omit => DirectUrlMode::Omit,
        WireMode::Unsigned => DirectUrlMode::Unsigned,
    })
}

fn try_samples_mode(
    proto: crate::cloud::v1alpha1::SegmentSamplesSelection,
) -> Result<ChunkMode, tonic::Status> {
    let crate::cloud::v1alpha1::SegmentSamplesSelection {
        timeline,
        segments,
        fill_mode,
    } = proto;

    let fill_mode = try_fill_mode(fill_mode, "samples.fill_mode")?;
    let timeline = TimelineName::try_new(timeline)
        .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;

    let mut seen = HashSet::with_capacity(segments.len());
    let mut entries = Vec::with_capacity(segments.len());

    for entry in segments {
        let crate::cloud::v1alpha1::SegmentSamples { segment_id, values } = entry;

        let segment_id: SegmentId = required_field(segment_id, "samples.segment_id")?.try_into()?;

        if !seen.insert(segment_id.clone()) {
            return Err(tonic::Status::invalid_argument(format!(
                "duplicate segment id in `samples`: {segment_id}"
            )));
        }

        let values = values
            .into_iter()
            .map(|raw| try_time_value(raw, "samples.values"))
            .collect::<Result<BTreeSet<_>, tonic::Status>>()?;

        entries.push((segment_id, values));
    }

    // Drop segments with an empty value list only after every entry has been validated, so a
    // later duplicate or invalid value is still rejected.
    let segments = entries
        .into_iter()
        .filter(|(_, values)| !values.is_empty())
        .collect();

    Ok(ChunkMode::Samples {
        timeline,
        segments,
        fill_mode,
    })
}

impl TryFrom<crate::cloud::v1alpha1::FindChunksRequest> for FindChunksRequest {
    type Error = tonic::Status;

    fn try_from(value: crate::cloud::v1alpha1::FindChunksRequest) -> Result<Self, Self::Error> {
        use crate::cloud::v1alpha1::find_chunks_request::Mode;

        let direct_url_mode = try_direct_url_mode(value.direct_url_mode)?;
        let contents = try_content_selection(required_field(value.contents, "contents")?)?;

        let mode = required_field(value.mode, "mode")?;
        let mode = match mode {
            Mode::Static(static_selection) => ChunkMode::Static {
                segments: try_segment_selection(required_field(
                    static_selection.segments,
                    "static.segments",
                )?)?,
            },

            Mode::Range(range_selection) => {
                let crate::cloud::v1alpha1::RangeSelection {
                    timeline,
                    start_inclusive,
                    end_inclusive,
                    fill_mode,
                    segments,
                } = range_selection;

                let fill_mode = try_fill_mode(fill_mode, "range.fill_mode")?;
                let segments = try_segment_selection(required_field(segments, "range.segments")?)?;
                let range = try_range_bounds(timeline, start_inclusive, end_inclusive, fill_mode)?;

                ChunkMode::Range { segments, range }
            }

            Mode::Samples(samples_selection) => try_samples_mode(samples_selection)?,
        };

        Ok(Self {
            contents,
            mode,
            direct_url_mode,
        })
    }
}

impl From<ContentSelection> for crate::cloud::v1alpha1::ContentSelection {
    fn from(value: ContentSelection) -> Self {
        use crate::cloud::v1alpha1::content_selection::Selection;

        let selection = match value {
            ContentSelection::All => Selection::All(()),
            ContentSelection::Explicit(entities) => {
                Selection::Explicit(crate::cloud::v1alpha1::EntitySelections {
                    entities: entities.into_iter().map(Into::into).collect(),
                })
            }
        };

        Self {
            selection: Some(selection),
        }
    }
}

impl From<EntitySelection> for crate::cloud::v1alpha1::EntitySelection {
    fn from(value: EntitySelection) -> Self {
        Self {
            entity_path: Some(value.entity_path.into()),
            components: Some(value.components.into()),
        }
    }
}

impl From<ComponentSelection> for crate::cloud::v1alpha1::entity_selection::Components {
    fn from(value: ComponentSelection) -> Self {
        use crate::cloud::v1alpha1::entity_selection::Components;

        match value {
            ComponentSelection::All => Components::All(()),
            ComponentSelection::Explicit(ids) => {
                Components::Explicit(crate::cloud::v1alpha1::ComponentIds {
                    ids: ids.into_iter().collect(),
                })
            }
        }
    }
}

impl From<SegmentSelection> for crate::cloud::v1alpha1::SegmentSelection {
    fn from(value: SegmentSelection) -> Self {
        use crate::cloud::v1alpha1::segment_selection::Selection;

        let selection = match value {
            SegmentSelection::All => Selection::All(()),
            SegmentSelection::Explicit(ids) => {
                Selection::Explicit(crate::cloud::v1alpha1::SegmentIds {
                    ids: ids.into_iter().map(Into::into).collect(),
                })
            }
        };

        Self {
            selection: Some(selection),
        }
    }
}

impl From<ChunkMode> for crate::cloud::v1alpha1::find_chunks_request::Mode {
    fn from(value: ChunkMode) -> Self {
        use crate::cloud::v1alpha1::find_chunks_request::Mode;

        match value {
            ChunkMode::Static { segments } => {
                Mode::Static(crate::cloud::v1alpha1::StaticSelection {
                    segments: Some(segments.into()),
                })
            }

            ChunkMode::Range { segments, range } => {
                Mode::Range(crate::cloud::v1alpha1::RangeSelection {
                    timeline: range.timeline.to_string(),
                    start_inclusive: range.start_inclusive.map(TimeInt::as_i64),
                    end_inclusive: range.end_inclusive.map(TimeInt::as_i64),
                    fill_mode: crate::cloud::v1alpha1::FillMode::from(range.fill_mode) as i32,
                    segments: Some(segments.into()),
                })
            }

            ChunkMode::Samples {
                timeline,
                segments,
                fill_mode,
            } => Mode::Samples(crate::cloud::v1alpha1::SegmentSamplesSelection {
                timeline: timeline.to_string(),
                segments: segments
                    .into_iter()
                    .map(
                        |(segment_id, values)| crate::cloud::v1alpha1::SegmentSamples {
                            segment_id: Some(segment_id.into()),
                            values: values.into_iter().map(TimeInt::as_i64).collect(),
                        },
                    )
                    .collect(),
                fill_mode: crate::cloud::v1alpha1::FillMode::from(fill_mode) as i32,
            }),
        }
    }
}

impl From<FindChunksRequest> for crate::cloud::v1alpha1::FindChunksRequest {
    fn from(value: FindChunksRequest) -> Self {
        Self {
            contents: Some(value.contents.into()),
            direct_url_mode: match value.direct_url_mode {
                DirectUrlMode::Omit => crate::cloud::v1alpha1::DirectUrlMode::Omit as i32,
                DirectUrlMode::Unsigned => crate::cloud::v1alpha1::DirectUrlMode::Unsigned as i32,
                DirectUrlMode::Signed => crate::cloud::v1alpha1::DirectUrlMode::Signed as i32,
            },
            mode: Some(value.mode.into()),
        }
    }
}

// --- QueryDatasetResponse ---

impl QueryDatasetResponse {
    /// Per-timeline `{timeline_name}:start` column that carries `time_min` for each chunk.
    ///
    /// Consumed by the client's `build_segment_manifests` to compute the per-segment safe
    /// horizon; no other downstream consumer reads the time range, so there is no matching
    /// `:end` column.
    ///
    /// The column type is `Int64` because all rerun time types store `i64` internally.
    pub fn field_timeline_start(timeline_name: &str) -> FieldRef {
        let metadata = std::collections::HashMap::from([
            ("rerun:index".to_owned(), timeline_name.to_owned()),
            (
                re_sorbet::metadata::RERUN_KIND.to_owned(),
                "index".to_owned(),
            ),
            ("rerun:index_marker".to_owned(), "start".to_owned()),
        ]);
        Arc::new(
            Field::new(format!("{timeline_name}:start"), DataType::Int64, true)
                .with_metadata(metadata),
        )
    }

    pub fn create_dataframe(
        chunk_ids: Vec<re_chunk::ChunkId>,
        chunk_segment_ids: Vec<SegmentId>,
        chunk_layer_names: Vec<LayerName>,
        chunk_keys: Vec<&[u8]>,
        chunk_entity_paths: Vec<EntityPath>,
        chunk_is_static: Vec<bool>,
        chunk_byte_lengths: Vec<u64>,
        chunk_byte_lengths_uncompressed: Vec<Option<u64>>,
        chunk_direct_urls: Vec<Option<String>>,
        chunk_direct_urls_expiry: Vec<Option<i64>>,
    ) -> arrow::error::Result<RecordBatch> {
        Self::create_dataframe_with_timelines(
            chunk_ids,
            chunk_segment_ids,
            chunk_layer_names,
            chunk_keys,
            chunk_entity_paths,
            chunk_is_static,
            chunk_byte_lengths,
            chunk_byte_lengths_uncompressed,
            chunk_direct_urls,
            chunk_direct_urls_expiry,
            &Default::default(),
        )
    }

    pub fn create_dataframe_with_timelines(
        chunk_ids: Vec<re_chunk::ChunkId>,
        chunk_segment_ids: Vec<SegmentId>,
        chunk_layer_names: Vec<LayerName>,
        chunk_keys: Vec<&[u8]>,
        chunk_entity_paths: Vec<EntityPath>,
        chunk_is_static: Vec<bool>,
        chunk_byte_lengths: Vec<u64>,
        chunk_byte_lengths_uncompressed: Vec<Option<u64>>,
        chunk_direct_urls: Vec<Option<String>>,
        chunk_direct_urls_expiry: Vec<Option<i64>>,
        timelines: &std::collections::BTreeMap<String, (DataType, Vec<Option<i64>>)>,
    ) -> arrow::error::Result<RecordBatch> {
        type Df = QueryDatasetDataframe;
        Df {
            chunk_id: Df::COLUMN_CHUNK_ID.new_from_values(chunk_ids),
            chunk_segment_id: Df::COLUMN_CHUNK_SEGMENT_ID.new_from_values(chunk_segment_ids),
            rerun_segment_layer: Df::COLUMN_RERUN_SEGMENT_LAYER.new_from_values(chunk_layer_names),
            chunk_key: Df::COLUMN_CHUNK_KEY.new_from_values(chunk_keys),
            chunk_entity_path: Df::COLUMN_CHUNK_ENTITY_PATH.new_from_values(chunk_entity_paths),
            chunk_is_static: Df::COLUMN_CHUNK_IS_STATIC.new_from_values(chunk_is_static),
            chunk_byte_len: Df::COLUMN_CHUNK_BYTE_LEN.new_from_values(chunk_byte_lengths),
            chunk_byte_size_uncompressed: Df::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED
                .new_from_values(chunk_byte_lengths_uncompressed),
            rerun_layer_direct_url: Df::COLUMN_RERUN_LAYER_DIRECT_URL
                .try_new_from_values(chunk_direct_urls)?,
            rerun_layer_direct_url_expires_at: Df::COLUMN_RERUN_LAYER_DIRECT_URL_EXPIRES_AT
                .try_new_from_values(chunk_direct_urls_expiry)?,

            // Caller is responsible for producing the same `timelines` set for every response of a
            // single query, so all batches share a schema and the client can concatenate them.
            extra_columns: timelines
                .iter()
                .map(|(timeline_name, (_data_type, mins))| {
                    quiver::DynColumn::try_new(
                        Self::field_timeline_start(timeline_name),
                        Arc::new(Int64Array::from(mins.clone())),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| ArrowError::InvalidArgumentError(err.to_string()))?,
        }
        .into_record_batch()
        .map_err(|err| ArrowError::InvalidArgumentError(err.to_string()))
    }
}

impl ScanSegmentTableRequest {
    /// Request every segment-table column with no filter hint.
    pub fn all() -> Self {
        Self {
            columns: Vec::new(),
            segment_id_filter: None,
        }
    }

    /// Request selected segment-table columns with no filter hint.
    pub fn with_columns(columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            columns: columns.into_iter().map(Into::into).collect(),
            segment_id_filter: None,
        }
    }
}

impl ScanDatasetManifestRequest {
    /// Request every dataset-manifest column with no filter hint.
    pub fn all() -> Self {
        Self {
            columns: Vec::new(),
            segment_id_filter: None,
        }
    }

    /// Request selected dataset-manifest columns with no filter hint.
    pub fn with_columns(columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            columns: columns.into_iter().map(Into::into).collect(),
            segment_id_filter: None,
        }
    }
}

impl FetchChunksRequest {
    /// The only column a `FetchChunks` request truly requires.
    pub const COLUMN_CHUNK_KEY: quiver::ColumnDesc<quiver::Binary> =
        QueryDatasetDataframe::COLUMN_CHUNK_KEY;

    pub fn required_column_names() -> Vec<String> {
        [
            Self::COLUMN_CHUNK_KEY.name,
            //TODO(RR-2677): remove these, they are only required for old clients.
            QueryDatasetDataframe::COLUMN_CHUNK_ID.name,
            QueryDatasetDataframe::COLUMN_CHUNK_SEGMENT_ID.name,
            QueryDatasetDataframe::COLUMN_RERUN_SEGMENT_LAYER.name,
            QueryDatasetDataframe::COLUMN_CHUNK_BYTE_LEN.name,
        ]
        .map(str::to_owned)
        .to_vec()
    }
}

#[cfg(test)]
mod tests {
    use arrow::datatypes::ToByteSlice as _;

    use super::*;
    use crate::cloud::v1alpha1 as cloud_proto;
    use crate::cloud::v1alpha1::ext::QueryLatestAt;

    #[test]
    fn test_query_dataset_response_create_dataframe() {
        let chunk_ids = vec![re_chunk::ChunkId::new(), re_chunk::ChunkId::new()];
        let chunk_segment_ids = vec![
            SegmentId::from("segment_id_1"),
            SegmentId::from("segment_id_2"),
        ];
        let chunk_layer_names = vec![LayerName::from("layer1"), LayerName::from("layer2")];
        let chunk_keys = vec![b"key1".to_byte_slice(), b"key2".to_byte_slice()];
        let chunk_entity_paths = vec![EntityPath::root(), EntityPath::root()];
        let chunk_is_static = vec![true, false];
        let chunk_byte_lengths = vec![1024u64, 2048u64];
        let direct_urls = vec![None, None];
        let direct_urls_expiry = vec![None, None];

        let chunk_byte_lengths_uncompressed = vec![Some(2048u64), Some(4096u64)];

        let batch = QueryDatasetResponse::create_dataframe(
            chunk_ids,
            chunk_segment_ids,
            chunk_layer_names,
            chunk_keys,
            chunk_entity_paths,
            chunk_is_static,
            chunk_byte_lengths,
            chunk_byte_lengths_uncompressed,
            direct_urls,
            direct_urls_expiry,
        )
        .unwrap();

        assert_eq!(
            batch.schema().as_ref(),
            &QueryDatasetDataframe::max_schema(),
            "the builder must produce exactly the declared columns (no stray extra columns)"
        );
    }

    #[test]
    fn test_query_latest_at_per_segment_values_round_trip() {
        let ext = QueryLatestAt {
            index: Some("frame".into()),
            at: TimeInt::STATIC,
            per_segment_values: vec![vec![1, 2, 3], vec![10], vec![]],
        };
        let wire: crate::cloud::v1alpha1::QueryLatestAt = ext.clone().into();
        assert!(wire.at.is_none(), "STATIC must serialize as None");
        assert_eq!(wire.per_segment_values.len(), 3);
        assert_eq!(wire.per_segment_values[0].values, vec![1, 2, 3]);
        assert_eq!(wire.per_segment_values[1].values, vec![10]);
        assert!(wire.per_segment_values[2].values.is_empty());
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_length_validates() {
        // Mismatched lengths → invalid_argument
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![
                crate::common::v1alpha1::SegmentId {
                    id: Some("00000000-0000-0000-0000-000000000001".to_owned()),
                },
                crate::common::v1alpha1::SegmentId {
                    id: Some("00000000-0000-0000-0000-000000000002".to_owned()),
                },
            ],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: Some(re_log_types::TimelineName::from("log_time").into()),
                    at: None,
                    per_segment_values: vec![crate::cloud::v1alpha1::IndexValueList {
                        values: vec![1],
                    }],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("per_segment_values.len()"));
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_requires_segment_ids() {
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: Some(re_log_types::TimelineName::from("log_time").into()),
                    at: None,
                    per_segment_values: vec![crate::cloud::v1alpha1::IndexValueList {
                        values: vec![1],
                    }],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("segment_ids"));
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_rejects_duplicate_segments() {
        let dup = crate::common::v1alpha1::SegmentId {
            id: Some("00000000-0000-0000-0000-000000000007".to_owned()),
        };
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![dup.clone(), dup],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: Some(re_log_types::TimelineName::from("log_time").into()),
                    at: None,
                    per_segment_values: vec![
                        crate::cloud::v1alpha1::IndexValueList { values: vec![1] },
                        crate::cloud::v1alpha1::IndexValueList { values: vec![2] },
                    ],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("duplicates"));
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_empty_is_allowed() {
        // per_segment_values absent → no validation triggered
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![],
            query: Some(crate::cloud::v1alpha1::Query::default()),
            ..Default::default()
        };
        QueryDatasetRequest::try_from(req).expect("empty per_segment_values must be accepted");
    }

    #[test]
    fn test_query_dataset_request_per_segment_values_requires_index() {
        // Regression: previously the dataplatform handler errored on a missing
        // `latest_at.index` while OSS silently degraded to a static-only
        // fallback. Both backends must now reject identically at request decode.
        let req = crate::cloud::v1alpha1::QueryDatasetRequest {
            segment_ids: vec![crate::common::v1alpha1::SegmentId {
                id: Some("00000000-0000-0000-0000-000000000001".to_owned()),
            }],
            query: Some(crate::cloud::v1alpha1::Query {
                latest_at: Some(crate::cloud::v1alpha1::QueryLatestAt {
                    index: None,
                    at: None,
                    per_segment_values: vec![crate::cloud::v1alpha1::IndexValueList {
                        values: vec![1],
                    }],
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let err = QueryDatasetRequest::try_from(req).unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("latest_at.index"));
    }

    // --- FindChunksRequest ---

    fn fcr_all_segments() -> cloud_proto::SegmentSelection {
        cloud_proto::SegmentSelection {
            selection: Some(cloud_proto::segment_selection::Selection::All(())),
        }
    }

    fn fcr_explicit_segments(ids: &[&str]) -> cloud_proto::SegmentSelection {
        cloud_proto::SegmentSelection {
            selection: Some(cloud_proto::segment_selection::Selection::Explicit(
                cloud_proto::SegmentIds {
                    ids: ids.iter().map(|id| (*id).into()).collect(),
                },
            )),
        }
    }

    fn fcr_all_contents() -> cloud_proto::ContentSelection {
        cloud_proto::ContentSelection {
            selection: Some(cloud_proto::content_selection::Selection::All(())),
        }
    }

    fn fcr_explicit_contents(
        entities: Vec<cloud_proto::EntitySelection>,
    ) -> cloud_proto::ContentSelection {
        cloud_proto::ContentSelection {
            selection: Some(cloud_proto::content_selection::Selection::Explicit(
                cloud_proto::EntitySelections { entities },
            )),
        }
    }

    fn fcr_entity(
        path: &str,
        components: Option<cloud_proto::entity_selection::Components>,
    ) -> cloud_proto::EntitySelection {
        cloud_proto::EntitySelection {
            entity_path: Some(crate::common::v1alpha1::EntityPath {
                path: path.to_owned(),
            }),
            components,
        }
    }

    fn fcr_components_all() -> cloud_proto::entity_selection::Components {
        cloud_proto::entity_selection::Components::All(())
    }

    fn fcr_components_explicit(ids: &[&str]) -> cloud_proto::entity_selection::Components {
        cloud_proto::entity_selection::Components::Explicit(cloud_proto::ComponentIds {
            ids: ids.iter().map(|id| (*id).to_owned()).collect(),
        })
    }

    fn fcr_static_mode(
        segments: Option<cloud_proto::SegmentSelection>,
    ) -> cloud_proto::find_chunks_request::Mode {
        cloud_proto::find_chunks_request::Mode::Static(cloud_proto::StaticSelection { segments })
    }

    fn fcr_range_mode(
        timeline: &str,
        start_inclusive: Option<i64>,
        end_inclusive: Option<i64>,
        fill_mode: FillMode,
        segments: Option<cloud_proto::SegmentSelection>,
    ) -> cloud_proto::find_chunks_request::Mode {
        cloud_proto::find_chunks_request::Mode::Range(cloud_proto::RangeSelection {
            timeline: timeline.to_owned(),
            start_inclusive,
            end_inclusive,
            fill_mode: cloud_proto::FillMode::from(fill_mode) as i32,
            segments,
        })
    }

    fn fcr_range_mode_with_fill_mode(
        fill_mode: i32,
        segments: Option<cloud_proto::SegmentSelection>,
    ) -> cloud_proto::find_chunks_request::Mode {
        cloud_proto::find_chunks_request::Mode::Range(cloud_proto::RangeSelection {
            timeline: "frame".to_owned(),
            start_inclusive: Some(5),
            end_inclusive: Some(10),
            fill_mode,
            segments,
        })
    }

    fn fcr_samples_mode(
        timeline: &str,
        entries: Vec<(Option<&str>, Vec<i64>)>,
    ) -> cloud_proto::find_chunks_request::Mode {
        fcr_samples_mode_with_fill_mode(timeline, entries, cloud_proto::FillMode::None as i32)
    }

    fn fcr_samples_mode_with_fill_mode(
        timeline: &str,
        entries: Vec<(Option<&str>, Vec<i64>)>,
        fill_mode: i32,
    ) -> cloud_proto::find_chunks_request::Mode {
        cloud_proto::find_chunks_request::Mode::Samples(cloud_proto::SegmentSamplesSelection {
            timeline: timeline.to_owned(),
            segments: entries
                .into_iter()
                .map(|(id, values)| cloud_proto::SegmentSamples {
                    segment_id: id.map(Into::into),
                    values,
                })
                .collect(),
            fill_mode,
        })
    }

    fn fcr_request(
        contents: Option<cloud_proto::ContentSelection>,
        mode: Option<cloud_proto::find_chunks_request::Mode>,
    ) -> cloud_proto::FindChunksRequest {
        cloud_proto::FindChunksRequest {
            contents,
            direct_url_mode: cloud_proto::DirectUrlMode::Unspecified as i32,
            mode,
        }
    }

    /// Expected outcome of one [`FindChunksRequest`] validation case.
    enum FcrOutcome {
        /// `TryFrom` fails with `InvalidArgument`, whose message contains this substring.
        Invalid(&'static str),
        /// `TryFrom` succeeds with the expected `is_empty()`, then `ensure_supported` fails with
        /// `Unimplemented`, whose message contains `message`.
        Unsupported {
            is_empty: bool,
            message: &'static str,
        },
        /// `TryFrom` and `ensure_supported` succeed, and the result's `is_empty()` matches.
        Valid { is_empty: bool },
    }

    struct FcrCase {
        name: &'static str,
        proto: cloud_proto::FindChunksRequest,
        outcome: FcrOutcome,
    }

    fn fcr_check(case: FcrCase) {
        let FcrCase {
            name,
            proto,
            outcome,
        } = case;
        let result = FindChunksRequest::try_from(proto);

        match outcome {
            FcrOutcome::Invalid(substr) => {
                let err = result
                    .err()
                    .unwrap_or_else(|| panic!("{name}: expected an error"));
                assert_eq!(err.code(), tonic::Code::InvalidArgument, "{name}");
                assert!(
                    err.message().contains(substr),
                    "{name}: expected message to contain {substr:?}, got {}",
                    err.message()
                );
            }
            FcrOutcome::Unsupported { is_empty, message } => {
                let request = result.unwrap_or_else(|err| panic!("{name}: expected Ok, got {err}"));
                assert_eq!(request.is_empty(), is_empty, "{name}");
                let err = request
                    .ensure_supported()
                    .err()
                    .unwrap_or_else(|| panic!("{name}: expected an unsupported request"));
                assert_eq!(err.code(), tonic::Code::Unimplemented, "{name}");
                assert!(
                    err.message().contains(message),
                    "{name}: expected message to contain {message:?}, got {}",
                    err.message()
                );
            }
            FcrOutcome::Valid { is_empty } => {
                let request = result.unwrap_or_else(|err| panic!("{name}: expected Ok, got {err}"));
                assert_eq!(request.is_empty(), is_empty, "{name}");
                request.ensure_supported().unwrap_or_else(|err| {
                    panic!("{name}: expected a supported request, got {err}")
                });
            }
        }
    }

    /// Each protobuf request either fails conversion with `InvalidArgument`, or converts with the
    /// expected `is_empty()` and then passes or fails the support check.
    #[test]
    fn find_chunks_request_validation_and_emptiness() {
        let cases = vec![
            FcrCase {
                name: "contents missing",
                proto: fcr_request(None, Some(fcr_static_mode(Some(fcr_all_segments())))),
                outcome: FcrOutcome::Invalid("contents"),
            },
            FcrCase {
                name: "mode missing",
                proto: fcr_request(Some(fcr_all_contents()), None),
                outcome: FcrOutcome::Invalid("mode"),
            },
            FcrCase {
                name: "empty entities list",
                proto: fcr_request(
                    Some(fcr_explicit_contents(vec![])),
                    Some(fcr_static_mode(Some(fcr_all_segments()))),
                ),
                outcome: FcrOutcome::Valid { is_empty: true },
            },
            FcrCase {
                name: "duplicate entity path is caught even if one entry would later be dropped",
                proto: fcr_request(
                    Some(fcr_explicit_contents(vec![
                        fcr_entity("world/points", Some(fcr_components_explicit(&[]))),
                        fcr_entity("world/points", Some(fcr_components_all())),
                    ])),
                    Some(fcr_static_mode(Some(fcr_all_segments()))),
                ),
                outcome: FcrOutcome::Invalid("duplicate entity path"),
            },
            FcrCase {
                name: "component id empty string",
                proto: fcr_request(
                    Some(fcr_explicit_contents(vec![fcr_entity(
                        "world/points",
                        Some(fcr_components_explicit(&[""])),
                    )])),
                    Some(fcr_static_mode(Some(fcr_all_segments()))),
                ),
                outcome: FcrOutcome::Invalid("non-empty"),
            },
            FcrCase {
                name: "entity with only an empty component set is dropped",
                proto: fcr_request(
                    Some(fcr_explicit_contents(vec![fcr_entity(
                        "world/points",
                        Some(fcr_components_explicit(&[])),
                    )])),
                    Some(fcr_static_mode(Some(fcr_all_segments()))),
                ),
                outcome: FcrOutcome::Valid { is_empty: true },
            },
            FcrCase {
                name: "static: All segments",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_static_mode(Some(fcr_all_segments()))),
                ),
                outcome: FcrOutcome::Valid { is_empty: false },
            },
            FcrCase {
                name: "static: Explicit(empty) segments",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_static_mode(Some(fcr_explicit_segments(&[])))),
                ),
                outcome: FcrOutcome::Valid { is_empty: true },
            },
            FcrCase {
                name: "range: unknown fill mode",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_range_mode_with_fill_mode(99, Some(fcr_all_segments()))),
                ),
                outcome: FcrOutcome::Invalid("range.fill_mode"),
            },
            FcrCase {
                name: "range: reversed bounds win over an unimplemented fill mode",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(cloud_proto::find_chunks_request::Mode::Range(
                        cloud_proto::RangeSelection {
                            timeline: "frame".to_owned(),
                            start_inclusive: Some(10),
                            end_inclusive: Some(5),
                            fill_mode: cloud_proto::FillMode::EarliestAt as i32,
                            segments: Some(fcr_all_segments()),
                        },
                    )),
                ),
                outcome: FcrOutcome::Invalid("range.end_inclusive"),
            },
            FcrCase {
                name: "range: earliest-at",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_range_mode_with_fill_mode(
                        cloud_proto::FillMode::EarliestAt as i32,
                        Some(fcr_all_segments()),
                    )),
                ),
                outcome: FcrOutcome::Unsupported {
                    is_empty: false,
                    message: "range.fill_mode",
                },
            },
            FcrCase {
                name: "range: earliest-at with empty contents",
                proto: fcr_request(
                    Some(fcr_explicit_contents(vec![])),
                    Some(fcr_range_mode_with_fill_mode(
                        cloud_proto::FillMode::EarliestAt as i32,
                        Some(fcr_all_segments()),
                    )),
                ),
                outcome: FcrOutcome::Unsupported {
                    is_empty: true,
                    message: "range.fill_mode",
                },
            },
            FcrCase {
                name: "range: earliest-at with no segments",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_range_mode_with_fill_mode(
                        cloud_proto::FillMode::EarliestAt as i32,
                        Some(fcr_explicit_segments(&[])),
                    )),
                ),
                outcome: FcrOutcome::Unsupported {
                    is_empty: true,
                    message: "range.fill_mode",
                },
            },
            FcrCase {
                name: "range: reversed bounds",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_range_mode(
                        "frame",
                        Some(10),
                        Some(5),
                        FillMode::None,
                        Some(fcr_all_segments()),
                    )),
                ),
                outcome: FcrOutcome::Invalid("end_inclusive"),
            },
            FcrCase {
                name: "range: empty timeline",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_range_mode(
                        "",
                        None,
                        None,
                        FillMode::None,
                        Some(fcr_all_segments()),
                    )),
                ),
                outcome: FcrOutcome::Invalid("empty"),
            },
            FcrCase {
                name: "samples: no entries",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_samples_mode("frame", vec![])),
                ),
                outcome: FcrOutcome::Valid { is_empty: true },
            },
            FcrCase {
                name: "samples: earliest-at",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_samples_mode_with_fill_mode(
                        "frame",
                        vec![(Some("seg1"), vec![1])],
                        cloud_proto::FillMode::EarliestAt as i32,
                    )),
                ),
                outcome: FcrOutcome::Unsupported {
                    is_empty: false,
                    message: "samples.fill_mode",
                },
            },
            FcrCase {
                name: "samples: earliest-at with no entries",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_samples_mode_with_fill_mode(
                        "frame",
                        vec![],
                        cloud_proto::FillMode::EarliestAt as i32,
                    )),
                ),
                outcome: FcrOutcome::Unsupported {
                    is_empty: true,
                    message: "samples.fill_mode",
                },
            },
            FcrCase {
                name: "duplicate segment id is caught even if one entry would later be dropped",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_samples_mode(
                        "frame",
                        vec![(Some("seg1"), vec![]), (Some("seg1"), vec![1])],
                    )),
                ),
                outcome: FcrOutcome::Invalid("duplicate segment id"),
            },
            FcrCase {
                name: "samples: entry with an empty value list is dropped",
                proto: fcr_request(
                    Some(fcr_all_contents()),
                    Some(fcr_samples_mode("frame", vec![(Some("seg1"), vec![])])),
                ),
                outcome: FcrOutcome::Valid { is_empty: true },
            },
        ];

        for case in cases {
            fcr_check(case);
        }
    }

    /// The range bound rules, including the static sentinel and equal bounds.
    #[test]
    fn find_chunks_range_bounds_validation() {
        struct Case {
            name: &'static str,
            start: Option<i64>,
            end: Option<i64>,
            fill_mode: FillMode,
            expect_err: Option<&'static str>,
        }

        let cases = [
            Case {
                name: "no bounds",
                start: None,
                end: None,
                fill_mode: FillMode::None,
                expect_err: None,
            },
            Case {
                name: "equal bounds",
                start: Some(5),
                end: Some(5),
                fill_mode: FillMode::None,
                expect_err: None,
            },
            Case {
                name: "reversed bounds",
                start: Some(10),
                end: Some(5),
                fill_mode: FillMode::None,
                expect_err: Some("end_inclusive"),
            },
            Case {
                name: "start at TimeInt::MIN",
                start: Some(TimeInt::MIN.as_i64()),
                end: Some(TimeInt::MAX.as_i64()),
                fill_mode: FillMode::None,
                expect_err: None,
            },
            Case {
                name: "start at the static sentinel",
                start: Some(i64::MIN),
                end: None,
                fill_mode: FillMode::None,
                expect_err: Some("i64::MIN"),
            },
            Case {
                name: "earliest-at without bounds",
                start: None,
                end: None,
                fill_mode: FillMode::EarliestAt,
                expect_err: None,
            },
            Case {
                name: "latest-at without start",
                start: None,
                end: Some(5),
                fill_mode: FillMode::LatestAt,
                expect_err: Some("range.fill_mode"),
            },
        ];

        for case in cases {
            let result = try_range_bounds("frame".to_owned(), case.start, case.end, case.fill_mode);

            match case.expect_err {
                None => {
                    let range = result
                        .unwrap_or_else(|err| panic!("{}: expected Ok, got {err}", case.name));

                    if case.start.is_some() && case.start == case.end {
                        // Equal bounds select exactly one time, so they are never an empty
                        // selection.
                        let request = FindChunksRequest {
                            contents: ContentSelection::All,
                            mode: ChunkMode::Range {
                                segments: SegmentSelection::All,
                                range,
                            },
                            direct_url_mode: DirectUrlMode::Signed,
                        };
                        assert!(
                            !request.is_empty(),
                            "{}: equal bounds must not be empty",
                            case.name
                        );
                    }
                }
                Some(substr) => {
                    let err = result
                        .err()
                        .unwrap_or_else(|| panic!("{}: expected an error", case.name));
                    assert_eq!(err.code(), tonic::Code::InvalidArgument, "{}", case.name);
                    assert!(
                        err.message().contains(substr),
                        "{}: expected message to contain {substr:?}, got {}",
                        case.name,
                        err.message()
                    );
                }
            }
        }
    }

    /// Every mode converts to the protobuf type and back unchanged, with every fill mode, including
    /// the unsupported one.
    #[test]
    fn find_chunks_request_proto_round_trip() {
        let cases = [
            FindChunksRequest {
                contents: ContentSelection::All,
                mode: ChunkMode::Static {
                    segments: SegmentSelection::All,
                },
                direct_url_mode: DirectUrlMode::Signed,
            },
            FindChunksRequest {
                contents: ContentSelection::Explicit(vec![EntitySelection {
                    entity_path: EntityPath::parse_strict("world/points")
                        .expect("valid entity path literal"),
                    components: ComponentSelection::Explicit(BTreeSet::from([
                        "Points3D:positions".to_owned(),
                    ])),
                }]),
                mode: ChunkMode::Range {
                    segments: SegmentSelection::Explicit(BTreeSet::from([SegmentId::from("seg1")])),
                    range: RangeSelection {
                        timeline: TimelineName::from("frame"),
                        start_inclusive: Some(TimeInt::new_temporal(10)),
                        end_inclusive: Some(TimeInt::new_temporal(10)),
                        fill_mode: FillMode::LatestAt,
                    },
                },
                direct_url_mode: DirectUrlMode::Signed,
            },
            FindChunksRequest {
                contents: ContentSelection::All,
                mode: ChunkMode::Range {
                    segments: SegmentSelection::All,
                    range: RangeSelection {
                        timeline: TimelineName::from("frame"),
                        start_inclusive: None,
                        end_inclusive: Some(TimeInt::new_temporal(10)),
                        fill_mode: FillMode::EarliestAt,
                    },
                },
                direct_url_mode: DirectUrlMode::Signed,
            },
            FindChunksRequest {
                contents: ContentSelection::All,
                mode: ChunkMode::Samples {
                    timeline: TimelineName::from("frame"),
                    segments: BTreeMap::from([(
                        SegmentId::from("seg1"),
                        BTreeSet::from([TimeInt::new_temporal(1), TimeInt::new_temporal(2)]),
                    )]),
                    fill_mode: FillMode::None,
                },
                direct_url_mode: DirectUrlMode::Signed,
            },
            FindChunksRequest {
                contents: ContentSelection::All,
                mode: ChunkMode::Samples {
                    timeline: TimelineName::from("frame"),
                    segments: BTreeMap::from([(
                        SegmentId::from("seg1"),
                        BTreeSet::from([TimeInt::new_temporal(1)]),
                    )]),
                    fill_mode: FillMode::EarliestAt,
                },
                direct_url_mode: DirectUrlMode::Signed,
            },
        ];

        for original in cases {
            let proto: cloud_proto::FindChunksRequest = original.clone().into();
            let round_tripped = FindChunksRequest::try_from(proto)
                .unwrap_or_else(|err| panic!("round-tripped request must validate: {err}"));
            assert_eq!(round_tripped, original, "{original:?}");
        }
    }

    #[test]
    fn find_chunks_fill_mode_conversion() {
        for (wire_fill_mode, expected) in [
            (cloud_proto::FillMode::Unspecified, FillMode::None),
            (cloud_proto::FillMode::None, FillMode::None),
            (cloud_proto::FillMode::LatestAt, FillMode::LatestAt),
            (cloud_proto::FillMode::EarliestAt, FillMode::EarliestAt),
        ] {
            let proto = fcr_request(
                Some(fcr_all_contents()),
                Some(fcr_range_mode_with_fill_mode(
                    wire_fill_mode as i32,
                    Some(fcr_all_segments()),
                )),
            );
            let request = FindChunksRequest::try_from(proto)
                .unwrap_or_else(|err| panic!("{wire_fill_mode:?} must convert: {err}"));
            let ChunkMode::Range { range, .. } = request.mode else {
                panic!("expected range mode");
            };
            assert_eq!(range.fill_mode, expected);
        }
    }

    /// `SegmentSelection::All` (every segment) and `Explicit(empty)` (no segment) must stay
    /// distinct in the protobuf type, and convert back to themselves.
    #[test]
    fn segment_selection_all_and_explicit_empty_remain_distinct() {
        let all: cloud_proto::SegmentSelection = SegmentSelection::All.into();
        let explicit_empty: cloud_proto::SegmentSelection =
            SegmentSelection::Explicit(BTreeSet::new()).into();
        assert_ne!(all, explicit_empty);

        let all_ext = try_segment_selection(all).unwrap_or_else(|err| panic!("{err}"));
        let explicit_empty_ext =
            try_segment_selection(explicit_empty).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(all_ext, SegmentSelection::All);
        assert_eq!(
            explicit_empty_ext,
            SegmentSelection::Explicit(BTreeSet::new())
        );
        assert_ne!(all_ext, explicit_empty_ext);
    }

    /// Built directly as ext types, without the wire conversion that drops entities with an empty
    /// component set.
    #[test]
    fn find_chunks_request_is_empty_when_every_entity_selects_no_component() {
        let entity = |path: &str, ids: &[&str]| EntitySelection {
            entity_path: EntityPath::from(path),
            components: ComponentSelection::Explicit(ids.iter().map(|&id| id.to_owned()).collect()),
        };
        let request = |entities| FindChunksRequest {
            contents: ContentSelection::Explicit(entities),
            mode: ChunkMode::Static {
                segments: SegmentSelection::All,
            },
            direct_url_mode: DirectUrlMode::default(),
        };

        assert!(
            request(vec![entity("a", &[])]).is_empty(),
            "an entity with an empty component set must select nothing"
        );
        assert!(
            !request(vec![entity("a", &[]), entity("b", &["Points3D:positions"])]).is_empty(),
            "an entity with a nonempty component set must keep the request nonempty"
        );
    }

    #[test]
    fn direct_url_mode_conversion() {
        use cloud_proto::DirectUrlMode as WireMode;

        let base = fcr_request(
            Some(fcr_all_contents()),
            Some(fcr_static_mode(Some(fcr_all_segments()))),
        );
        for (wire_mode, expected_ext, expected_wire) in [
            (
                WireMode::Unspecified,
                DirectUrlMode::Signed,
                WireMode::Signed,
            ),
            (WireMode::Omit, DirectUrlMode::Omit, WireMode::Omit),
            (
                WireMode::Unsigned,
                DirectUrlMode::Unsigned,
                WireMode::Unsigned,
            ),
            (WireMode::Signed, DirectUrlMode::Signed, WireMode::Signed),
        ] {
            let proto = cloud_proto::FindChunksRequest {
                direct_url_mode: wire_mode as i32,
                ..base.clone()
            };
            let ext = FindChunksRequest::try_from(proto)
                .unwrap_or_else(|err| panic!("valid request: {err}"));
            assert_eq!(ext.direct_url_mode, expected_ext);

            let round_tripped: cloud_proto::FindChunksRequest = ext.into();
            assert_eq!(round_tripped.direct_url_mode, expected_wire as i32);
        }

        let err = FindChunksRequest::try_from(cloud_proto::FindChunksRequest {
            direct_url_mode: 99,
            contents: Some(fcr_all_contents()),
            mode: Some(fcr_static_mode(Some(fcr_explicit_segments(&[])))),
        })
        .expect_err("unknown direct URL mode must be rejected before empty-selection handling");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("direct_url_mode"));
    }
}
