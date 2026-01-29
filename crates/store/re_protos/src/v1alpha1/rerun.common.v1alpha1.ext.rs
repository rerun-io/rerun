use std::hash::Hasher as _;

use arrow::datatypes::Schema as ArrowSchema;
use arrow::error::ArrowError;

use re_log_types::external::re_types_core::ComponentDescriptor;
use re_log_types::{RecordingId, StoreKind, TableId};

use crate::{TypeConversionError, invalid_field, missing_field};

// --- Arrow ---

impl TryFrom<&crate::common::v1alpha1::Schema> for ArrowSchema {
    type Error = ArrowError;

    fn try_from(value: &crate::common::v1alpha1::Schema) -> Result<Self, Self::Error> {
        let schema_bytes = value
            .arrow_schema
            .as_ref()
            .ok_or(ArrowError::InvalidArgumentError(
                "missing schema bytes".to_owned(),
            ))?;
        Ok(Self::clone(
            re_sorbet::migrated_schema_from_ipc(schema_bytes)?.as_ref(),
        ))
    }
}

impl TryFrom<&ArrowSchema> for crate::common::v1alpha1::Schema {
    type Error = ArrowError;

    fn try_from(value: &ArrowSchema) -> Result<Self, Self::Error> {
        Ok(Self {
            arrow_schema: Some(re_sorbet::ipc_from_schema(value)?.into()),
        })
    }
}

impl TryFrom<crate::common::v1alpha1::Schema> for ArrowSchema {
    type Error = ArrowError;

    fn try_from(value: crate::common::v1alpha1::Schema) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

// --- EntryId ---

impl From<re_log_types::EntryId> for crate::common::v1alpha1::EntryId {
    #[inline]
    fn from(value: re_log_types::EntryId) -> Self {
        Self {
            id: Some(value.id.into()),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::EntryId> for re_log_types::EntryId {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::EntryId) -> Result<Self, Self::Error> {
        let id = value
            .id
            .ok_or(missing_field!(crate::common::v1alpha1::EntryId, "id"))?;
        Ok(Self { id: id.try_into()? })
    }
}

// shortcuts

impl From<re_tuid::Tuid> for crate::common::v1alpha1::EntryId {
    fn from(id: re_tuid::Tuid) -> Self {
        let id: re_log_types::EntryId = id.into();
        Self {
            id: Some(id.id.into()),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::Tuid> for crate::common::v1alpha1::EntryId {
    type Error = TypeConversionError;

    fn try_from(id: crate::common::v1alpha1::Tuid) -> Result<Self, Self::Error> {
        let id: re_tuid::Tuid = id.try_into()?;
        Ok(Self {
            id: Some(id.into()),
        })
    }
}

// --- SegmentId ---

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct SegmentId {
    pub id: String,
}

impl SegmentId {
    #[inline]
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl From<String> for SegmentId {
    fn from(id: String) -> Self {
        Self { id }
    }
}

impl From<&str> for SegmentId {
    fn from(id: &str) -> Self {
        Self { id: id.to_owned() }
    }
}

impl std::fmt::Display for SegmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl TryFrom<crate::common::v1alpha1::SegmentId> for SegmentId {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::SegmentId) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(crate::common::v1alpha1::SegmentId, "id"))?,
        })
    }
}

impl From<SegmentId> for crate::common::v1alpha1::SegmentId {
    fn from(value: SegmentId) -> Self {
        Self { id: Some(value.id) }
    }
}

impl AsRef<str> for SegmentId {
    fn as_ref(&self) -> &str {
        self.id.as_str()
    }
}

// shortcuts

impl From<String> for crate::common::v1alpha1::SegmentId {
    fn from(id: String) -> Self {
        Self { id: Some(id) }
    }
}

impl From<&str> for crate::common::v1alpha1::SegmentId {
    fn from(id: &str) -> Self {
        Self {
            id: Some(id.to_owned()),
        }
    }
}

// --- DatasetHandle ---

#[derive(Debug, Clone)]
pub struct DatasetHandle {
    pub id: Option<re_log_types::EntryId>,
    pub store_kind: StoreKind,
    pub url: url::Url,
}

impl DatasetHandle {
    /// Create a new dataset handle
    pub fn new(url: url::Url, store_kind: StoreKind) -> Self {
        Self {
            id: None,
            store_kind,
            url,
        }
    }
}

impl TryFrom<crate::common::v1alpha1::DatasetHandle> for DatasetHandle {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::DatasetHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.entry_id.map(|id| id.try_into()).transpose()?,
            store_kind: crate::common::v1alpha1::StoreKind::try_from(value.store_kind)?.into(),
            url: value
                .dataset_url
                .ok_or(missing_field!(
                    crate::common::v1alpha1::DatasetHandle,
                    "dataset_url"
                ))?
                .parse()
                .map_err(|err| {
                    invalid_field!(crate::common::v1alpha1::DatasetHandle, "dataset_url", err)
                })?,
        })
    }
}

impl From<DatasetHandle> for crate::common::v1alpha1::DatasetHandle {
    fn from(value: DatasetHandle) -> Self {
        Self {
            entry_id: value.id.map(Into::into),
            store_kind: crate::common::v1alpha1::StoreKind::from(value.store_kind) as i32,
            dataset_url: Some(value.url.to_string()),
        }
    }
}

// --- TaskId ---

//TODO(ab): we should migrate the full `TaskId` wrapper from `redap_tasks`
impl crate::common::v1alpha1::TaskId {
    pub fn new() -> Self {
        Self::from_hashable(re_tuid::Tuid::new())
    }

    pub fn from_hashable<H: std::hash::Hash>(hashable: H) -> Self {
        let mut hasher = std::hash::DefaultHasher::new();
        hashable.hash(&mut hasher);
        let id = hasher.finish();

        Self {
            id: format!("task_{id:016x}"),
        }
    }
}

// ---

impl TryFrom<crate::common::v1alpha1::Tuid> for re_tuid::Tuid {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::Tuid) -> Result<Self, Self::Error> {
        let time_ns = value
            .time_ns
            .ok_or(missing_field!(crate::common::v1alpha1::Tuid, "time_ns"))?;
        let inc = value
            .inc
            .ok_or(missing_field!(crate::common::v1alpha1::Tuid, "inc"))?;

        Ok(Self::from_nanos_and_inc(time_ns, inc))
    }
}

impl From<re_tuid::Tuid> for crate::common::v1alpha1::Tuid {
    fn from(value: re_tuid::Tuid) -> Self {
        Self {
            time_ns: Some(value.nanos_since_epoch()),
            inc: Some(value.inc()),
        }
    }
}

impl From<re_log_types::EntityPath> for crate::common::v1alpha1::EntityPath {
    fn from(value: re_log_types::EntityPath) -> Self {
        Self {
            path: value.to_string(),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::EntityPath> for re_log_types::EntityPath {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::EntityPath) -> Result<Self, Self::Error> {
        Self::parse_strict(&value.path)
            .map_err(|err| invalid_field!(crate::common::v1alpha1::EntityPath, "path", err))
    }
}

impl From<re_log_types::AbsoluteTimeRange> for crate::common::v1alpha1::TimeRange {
    fn from(value: re_log_types::AbsoluteTimeRange) -> Self {
        Self {
            start: value.min().as_i64(),
            end: value.max().as_i64(),
        }
    }
}

impl From<crate::common::v1alpha1::TimeRange> for re_log_types::AbsoluteTimeRange {
    fn from(value: crate::common::v1alpha1::TimeRange) -> Self {
        Self::new(
            re_log_types::TimeInt::new_temporal(value.start),
            re_log_types::TimeInt::new_temporal(value.end),
        )
    }
}

impl From<re_log_types::AbsoluteTimeRange> for crate::common::v1alpha1::IndexRange {
    fn from(value: re_log_types::AbsoluteTimeRange) -> Self {
        Self {
            time_range: Some(value.into()),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::IndexRange> for re_log_types::AbsoluteTimeRange {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::IndexRange) -> Result<Self, Self::Error> {
        value
            .time_range
            .ok_or(missing_field!(
                crate::common::v1alpha1::IndexRange,
                "time_range"
            ))
            .map(|time_range| Self::new(time_range.start, time_range.end))
    }
}

impl From<crate::common::v1alpha1::Timeline> for re_log_types::TimelineName {
    fn from(value: crate::common::v1alpha1::Timeline) -> Self {
        Self::new(&value.name)
    }
}

impl From<re_log_types::TimelineName> for crate::common::v1alpha1::Timeline {
    fn from(value: re_log_types::TimelineName) -> Self {
        Self {
            name: value.to_string(),
        }
    }
}

impl From<re_log_types::Timeline> for crate::common::v1alpha1::Timeline {
    fn from(value: re_log_types::Timeline) -> Self {
        Self {
            name: value.name().to_string(),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::IndexColumnSelector> for re_log_types::TimelineName {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::IndexColumnSelector) -> Result<Self, Self::Error> {
        let timeline = value.timeline.ok_or(missing_field!(
            crate::common::v1alpha1::IndexColumnSelector,
            "timeline"
        ))?;
        Ok(timeline.into())
    }
}

impl From<re_log_types::TimelineName> for crate::common::v1alpha1::IndexColumnSelector {
    fn from(value: re_log_types::TimelineName) -> Self {
        Self {
            timeline: Some(value.into()),
        }
    }
}

impl From<crate::common::v1alpha1::ApplicationId> for re_log_types::ApplicationId {
    #[inline]
    fn from(value: crate::common::v1alpha1::ApplicationId) -> Self {
        Self::from(value.id)
    }
}

impl From<re_log_types::ApplicationId> for crate::common::v1alpha1::ApplicationId {
    #[inline]
    fn from(value: re_log_types::ApplicationId) -> Self {
        Self {
            id: value.to_string(),
        }
    }
}

impl From<crate::common::v1alpha1::StoreKind> for re_log_types::StoreKind {
    #[inline]
    fn from(value: crate::common::v1alpha1::StoreKind) -> Self {
        match value {
            crate::common::v1alpha1::StoreKind::Unspecified
            | crate::common::v1alpha1::StoreKind::Recording => Self::Recording,
            crate::common::v1alpha1::StoreKind::Blueprint => Self::Blueprint,
        }
    }
}

impl From<re_log_types::StoreKind> for crate::common::v1alpha1::StoreKind {
    #[inline]
    fn from(value: re_log_types::StoreKind) -> Self {
        match value {
            re_log_types::StoreKind::Recording => Self::Recording,
            re_log_types::StoreKind::Blueprint => Self::Blueprint,
        }
    }
}

/// The store id failed to deserialize due to missing application id.
///
/// This may happen when migrating older RRD (before application ID was moved to `StoreId`). This
/// error can be recovered from if the application ID is known.
//TODO(#10730): this is specifically for 0.24 back compat. Switch to `TypeConversionError` when cleaning up.
#[derive(Debug, Clone)]
pub struct StoreIdMissingApplicationIdError {
    pub store_kind: re_log_types::StoreKind,
    pub recording_id: RecordingId,
}

impl StoreIdMissingApplicationIdError {
    /// Recover the store ID by providing the application ID.
    pub fn recover(self, application_id: re_log_types::ApplicationId) -> re_log_types::StoreId {
        re_log_types::StoreId::new(self.store_kind, application_id, self.recording_id)
    }

    #[inline]
    pub fn into_type_conversion_error(self, msg: impl Into<String>) -> TypeConversionError {
        TypeConversionError::LegacyStoreIdError(format!(
            "{} (kind: {}, recording_id: {})",
            msg.into(),
            self.store_kind,
            self.recording_id
        ))
    }
}

/// Convert a store id
impl TryFrom<crate::common::v1alpha1::StoreId> for re_log_types::StoreId {
    type Error = StoreIdMissingApplicationIdError;

    #[inline]
    fn try_from(value: crate::common::v1alpha1::StoreId) -> Result<Self, Self::Error> {
        let store_kind = value.kind().into();
        let recording_id = RecordingId::from(value.recording_id);

        //TODO(#10730): switch to `TypeConversionError` when cleaning up 0.24 back compat
        match value.application_id {
            None => Err(StoreIdMissingApplicationIdError {
                store_kind,
                recording_id,
            }),
            Some(application_id) => Ok(re_log_types::StoreId::new(
                store_kind,
                application_id,
                recording_id,
            )),
        }
    }
}

impl From<re_log_types::StoreId> for crate::common::v1alpha1::StoreId {
    #[inline]
    fn from(value: re_log_types::StoreId) -> Self {
        let kind: crate::common::v1alpha1::StoreKind = value.kind().into();
        Self {
            kind: kind as i32,
            recording_id: value.recording_id().as_str().to_owned(),
            application_id: Some(value.application_id().clone().into()),
        }
    }
}

impl From<re_log_types::TableId> for crate::common::v1alpha1::TableId {
    #[inline]
    fn from(value: re_log_types::TableId) -> Self {
        Self {
            id: value.as_str().to_owned(),
        }
    }
}

impl From<crate::common::v1alpha1::TableId> for re_log_types::TableId {
    #[inline]
    fn from(value: crate::common::v1alpha1::TableId) -> Self {
        TableId::from(value.id)
    }
}

// --- Scanning & Querying ---

#[derive(Debug, Default, Clone)]
pub struct ScanParameters {
    pub columns: Vec<String>,
    pub on_missing_columns: IfMissingBehavior,
    pub filter: Option<String>,
    pub limit_offset: Option<i64>,
    pub limit_len: Option<i64>,
    pub order_by: Vec<ScanParametersOrderClause>,
    pub explain_plan: bool,
    pub explain_filter: bool,
}

impl TryFrom<crate::common::v1alpha1::ScanParameters> for ScanParameters {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::ScanParameters) -> Result<Self, Self::Error> {
        Ok(Self {
            columns: value.columns,
            on_missing_columns: crate::common::v1alpha1::IfMissingBehavior::try_from(
                value.on_missing_columns,
            )?
            .into(),
            filter: value.filter,
            limit_offset: value.limit_offset,
            limit_len: value.limit_len,
            order_by: value
                .order_by
                .into_iter()
                .map(|ob| ob.try_into())
                .collect::<Result<Vec<_>, _>>()?,
            explain_plan: value.explain_plan,
            explain_filter: value.explain_filter,
        })
    }
}

impl From<ScanParameters> for crate::common::v1alpha1::ScanParameters {
    fn from(value: ScanParameters) -> Self {
        Self {
            columns: value.columns,
            on_missing_columns: crate::common::v1alpha1::IfMissingBehavior::from(
                value.on_missing_columns,
            ) as _,
            filter: value.filter,
            limit_offset: value.limit_offset,
            limit_len: value.limit_len,
            order_by: value.order_by.into_iter().map(|ob| ob.into()).collect(),
            explain_plan: value.explain_plan,
            explain_filter: value.explain_filter,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ScanParametersOrderClause {
    pub descending: bool,
    pub nulls_last: bool,
    pub column_name: String,
}

impl TryFrom<crate::common::v1alpha1::ScanParametersOrderClause> for ScanParametersOrderClause {
    type Error = TypeConversionError;

    fn try_from(
        value: crate::common::v1alpha1::ScanParametersOrderClause,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            descending: value.descending,
            nulls_last: value.nulls_last,
            column_name: value.column_name.ok_or(missing_field!(
                crate::common::v1alpha1::ScanParametersOrderClause,
                "column_name"
            ))?,
        })
    }
}

impl From<ScanParametersOrderClause> for crate::common::v1alpha1::ScanParametersOrderClause {
    fn from(value: ScanParametersOrderClause) -> Self {
        Self {
            descending: value.descending,
            nulls_last: value.nulls_last,
            column_name: Some(value.column_name),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IfMissingBehavior {
    Skip,
    Error,
}

impl Default for IfMissingBehavior {
    fn default() -> Self {
        Self::Skip
    }
}

impl From<crate::common::v1alpha1::IfMissingBehavior> for IfMissingBehavior {
    fn from(value: crate::common::v1alpha1::IfMissingBehavior) -> Self {
        use crate::common::v1alpha1 as common;
        match value {
            common::IfMissingBehavior::Unspecified | common::IfMissingBehavior::Skip => Self::Skip,
            common::IfMissingBehavior::Error => Self::Error,
        }
    }
}

impl From<IfMissingBehavior> for crate::common::v1alpha1::IfMissingBehavior {
    fn from(value: IfMissingBehavior) -> Self {
        match value {
            IfMissingBehavior::Skip => Self::Skip,
            IfMissingBehavior::Error => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IfDuplicateBehavior {
    Overwrite,
    Skip,
    Error,
}

impl Default for IfDuplicateBehavior {
    fn default() -> Self {
        Self::Skip
    }
}

impl TryFrom<i32> for IfDuplicateBehavior {
    type Error = TypeConversionError;

    fn try_from(value: i32) -> Result<Self, TypeConversionError> {
        let proto_value = crate::common::v1alpha1::IfDuplicateBehavior::try_from(value)?;
        Ok(Self::from(proto_value))
    }
}

impl From<crate::common::v1alpha1::IfDuplicateBehavior> for IfDuplicateBehavior {
    fn from(value: crate::common::v1alpha1::IfDuplicateBehavior) -> Self {
        use crate::common::v1alpha1 as common;
        match value {
            common::IfDuplicateBehavior::Unspecified | common::IfDuplicateBehavior::Skip => {
                Self::Skip
            }
            common::IfDuplicateBehavior::Overwrite => Self::Overwrite,
            common::IfDuplicateBehavior::Error => Self::Error,
        }
    }
}

impl From<IfDuplicateBehavior> for crate::common::v1alpha1::IfDuplicateBehavior {
    fn from(value: IfDuplicateBehavior) -> Self {
        match value {
            IfDuplicateBehavior::Overwrite => Self::Overwrite,
            IfDuplicateBehavior::Skip => Self::Skip,
            IfDuplicateBehavior::Error => Self::Error,
        }
    }
}

// ---

impl From<ComponentDescriptor> for crate::common::v1alpha1::ComponentDescriptor {
    fn from(value: ComponentDescriptor) -> Self {
        Self {
            archetype: value.archetype.map(|n| n.full_name().to_owned()),
            component: Some(value.component.to_string()),
            component_type: value.component_type.map(|c| c.full_name().to_owned()),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::ComponentDescriptor> for ComponentDescriptor {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::ComponentDescriptor) -> Result<Self, Self::Error> {
        let crate::common::v1alpha1::ComponentDescriptor {
            archetype,
            component,
            component_type,
        } = value;

        let component = component.ok_or(missing_field!(
            crate::common::v1alpha1::ComponentDescriptor,
            "component"
        ))?;

        Ok(ComponentDescriptor {
            archetype: archetype.map(Into::into),
            component: component.into(),
            component_type: component_type.map(Into::into),
        })
    }
}

// ---

impl From<re_build_info::BuildInfo> for crate::common::v1alpha1::BuildInfo {
    fn from(build_info: re_build_info::BuildInfo) -> Self {
        Self {
            crate_name: Some(build_info.crate_name.to_string()),
            features: Some(build_info.features.to_string()),
            version: Some(build_info.version.into()),
            rustc_version: Some(build_info.rustc_version.to_string()),
            llvm_version: Some(build_info.llvm_version.to_string()),
            git_hash: Some(build_info.git_hash.to_string()),
            git_branch: Some(build_info.git_branch.to_string()),
            target_triple: Some(build_info.target_triple.to_string()),
            build_time: Some(build_info.datetime.to_string()),
            is_debug_build: Some(build_info.is_debug_build),
        }
    }
}

impl From<crate::common::v1alpha1::BuildInfo> for re_build_info::BuildInfo {
    fn from(build_info: crate::common::v1alpha1::BuildInfo) -> Self {
        Self {
            crate_name: build_info.crate_name().to_owned().into(),
            features: build_info.features().to_owned().into(),
            version: build_info.version.clone().unwrap_or_default().into(),
            rustc_version: build_info.rustc_version().to_owned().into(),
            llvm_version: build_info.llvm_version().to_owned().into(),
            git_hash: build_info.git_hash().to_owned().into(),
            git_branch: build_info.git_branch().to_owned().into(),
            is_in_rerun_workspace: false,
            target_triple: build_info.target_triple().to_owned().into(),
            datetime: build_info.build_time().to_owned().into(),
            is_debug_build: build_info.is_debug_build(),
        }
    }
}

impl From<re_build_info::CrateVersion> for crate::common::v1alpha1::SemanticVersion {
    fn from(version: re_build_info::CrateVersion) -> Self {
        crate::common::v1alpha1::SemanticVersion {
            major: Some(version.major.into()),
            minor: Some(version.minor.into()),
            patch: Some(version.patch.into()),
            meta: version.meta.map(Into::into),
        }
    }
}

impl From<crate::common::v1alpha1::SemanticVersion> for re_build_info::CrateVersion {
    fn from(version: crate::common::v1alpha1::SemanticVersion) -> Self {
        Self {
            major: version.major() as u8,
            minor: version.minor() as u8,
            patch: version.patch() as u8,
            meta: version.meta.map(Into::into),
        }
    }
}

impl From<re_build_info::Meta> for crate::common::v1alpha1::semantic_version::Meta {
    fn from(version_meta: re_build_info::Meta) -> Self {
        match version_meta {
            re_build_info::Meta::Rc(v) => Self::Rc(v.into()),

            re_build_info::Meta::Alpha(v) => Self::Alpha(v.into()),

            re_build_info::Meta::DevAlpha { alpha, commit } => {
                Self::DevAlpha(crate::common::v1alpha1::DevAlpha {
                    alpha: Some(alpha.into()),
                    commit: commit.map(|s| String::from_utf8_lossy(s).to_string()),
                })
            }
        }
    }
}

impl From<crate::common::v1alpha1::semantic_version::Meta> for re_build_info::Meta {
    fn from(version_meta: crate::common::v1alpha1::semantic_version::Meta) -> Self {
        match version_meta {
            crate::common::v1alpha1::semantic_version::Meta::Rc(v) => Self::Rc(v as _),

            crate::common::v1alpha1::semantic_version::Meta::Alpha(v) => Self::Alpha(v as _),

            crate::common::v1alpha1::semantic_version::Meta::DevAlpha(dev_alpha) => {
                Self::DevAlpha {
                    alpha: dev_alpha.alpha() as u8,
                    // TODO(cmc): support this, but that means DevAlpha is not-const
                    // anymore, which trigger a chain reaction of changes that I really
                    // don't want to get in right now.
                    commit: None,
                }
            }
        }
    }
}

// --- DataframePart / RerunChunk <-> RecordBatch ---

impl TryFrom<crate::common::v1alpha1::RerunChunk> for arrow::array::RecordBatch {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::RerunChunk) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl TryFrom<&crate::common::v1alpha1::RerunChunk> for arrow::array::RecordBatch {
    type Error = TypeConversionError;

    fn try_from(value: &crate::common::v1alpha1::RerunChunk) -> Result<Self, Self::Error> {
        match value.encoder_version() {
            crate::common::v1alpha1::EncoderVersion::Unspecified => {
                return Err(missing_field!(
                    crate::common::v1alpha1::RerunChunk,
                    "encoder_version"
                ));
            }

            crate::common::v1alpha1::EncoderVersion::V0 => {
                let Some(bytes) = value.payload.as_ref() else {
                    return Err(missing_field!(
                        crate::common::v1alpha1::DataframePart,
                        "payload"
                    ));
                };
                let Some(batch) = record_batch_from_ipc_bytes(bytes)? else {
                    return Err(invalid_field!(
                        crate::common::v1alpha1::RerunChunk,
                        "payload",
                        "empty"
                    ));
                };
                Ok(batch)
            }
        }
    }
}

impl From<arrow::array::RecordBatch> for crate::common::v1alpha1::RerunChunk {
    fn from(value: arrow::array::RecordBatch) -> Self {
        Self::from(&value)
    }
}

impl From<&arrow::array::RecordBatch> for crate::common::v1alpha1::RerunChunk {
    fn from(value: &arrow::array::RecordBatch) -> Self {
        let version = crate::common::v1alpha1::EncoderVersion::V0;
        Self {
            encoder_version: version as i32,
            payload: Some(record_batch_to_ipc_bytes(value).into()),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::DataframePart> for arrow::array::RecordBatch {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::DataframePart) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl TryFrom<&crate::common::v1alpha1::DataframePart> for arrow::array::RecordBatch {
    type Error = TypeConversionError;

    fn try_from(value: &crate::common::v1alpha1::DataframePart) -> Result<Self, Self::Error> {
        match value.encoder_version() {
            crate::common::v1alpha1::EncoderVersion::Unspecified => {
                return Err(missing_field!(
                    crate::common::v1alpha1::RerunChunk,
                    "encoder_version"
                ));
            }

            crate::common::v1alpha1::EncoderVersion::V0 => {
                let Some(bytes) = value.payload.as_ref() else {
                    return Err(missing_field!(
                        crate::common::v1alpha1::DataframePart,
                        "payload"
                    ));
                };
                let Some(batch) = record_batch_from_ipc_bytes(bytes)? else {
                    return Err(invalid_field!(
                        crate::common::v1alpha1::RerunChunk,
                        "payload",
                        "empty"
                    ));
                };
                Ok(batch)
            }
        }
    }
}

impl From<arrow::array::RecordBatch> for crate::common::v1alpha1::DataframePart {
    fn from(value: arrow::array::RecordBatch) -> Self {
        Self::from(&value)
    }
}

impl From<&arrow::array::RecordBatch> for crate::common::v1alpha1::DataframePart {
    fn from(value: &arrow::array::RecordBatch) -> Self {
        let version = crate::common::v1alpha1::EncoderVersion::V0;
        Self {
            encoder_version: version as i32,
            payload: Some(record_batch_to_ipc_bytes(value).into()),
        }
    }
}

/// `RecordBatch` to IPC bytes. No I/O, no failures.
///
/// Note that this is never compressed in any way. It is assumed that for `DataframePart`s and
/// `RerunChunk`s, transport-level compression will be used where needed.
#[tracing::instrument(level = "debug", skip_all)]
fn record_batch_to_ipc_bytes(batch: &arrow::array::RecordBatch) -> Vec<u8> {
    let schema = batch.schema_ref().as_ref();

    let mut out = Vec::new();

    let mut sw = {
        let _span = tracing::trace_span!("schema").entered();
        arrow::ipc::writer::StreamWriter::try_new(&mut out, schema)
            .expect("encoding the schema of a valid RecordBatch as IPC bytes into a growable in-memory buffer cannot possibly fail")
    };

    {
        let _span = tracing::trace_span!("data").entered();
        sw.write(batch)
            .expect("encoding the data of a valid RecordBatch as IPC bytes into a growable in-memory buffer cannot possibly fail");
    }

    sw.finish()
        .expect("encoding a valid RecordBatch as IPC bytes into a growable in-memory buffer cannot possibly fail");

    out
}

/// IPC bytes to `RecordBatch`. `Ok(None)` if there's no data.
#[tracing::instrument(level = "debug", skip_all)]
fn record_batch_from_ipc_bytes(
    bytes: &[u8],
) -> Result<Option<arrow::array::RecordBatch>, ArrowError> {
    let mut stream = {
        let _span = tracing::trace_span!("schema").entered();
        arrow::ipc::reader::StreamReader::try_new(bytes, None)?
    };

    let _span = tracing::trace_span!("data").entered();
    stream.next().transpose()
}

// ---

#[cfg(test)]
mod tests {

    #[test]
    fn entity_path_conversion() {
        let entity_path = re_log_types::EntityPath::parse_strict("a/b/c").unwrap();
        let proto_entity_path: crate::common::v1alpha1::EntityPath = entity_path.clone().into();
        let entity_path2: re_log_types::EntityPath = proto_entity_path.try_into().unwrap();
        assert_eq!(entity_path, entity_path2);
    }

    #[test]
    fn time_range_conversion() {
        let time_range = re_log_types::AbsoluteTimeRange::new(
            re_log_types::TimeInt::new_temporal(123456789),
            re_log_types::TimeInt::new_temporal(987654321),
        );
        let proto_time_range: crate::common::v1alpha1::TimeRange = time_range.into();
        let time_range2: re_log_types::AbsoluteTimeRange = proto_time_range.into();
        assert_eq!(time_range, time_range2);
    }

    #[test]
    fn index_range_conversion() {
        let time_range = re_log_types::AbsoluteTimeRange::new(
            re_log_types::TimeInt::new_temporal(123456789),
            re_log_types::TimeInt::new_temporal(987654321),
        );
        let proto_index_range: crate::common::v1alpha1::IndexRange = time_range.into();
        let time_range2: re_log_types::AbsoluteTimeRange = proto_index_range.try_into().unwrap();
        assert_eq!(time_range, time_range2);
    }

    #[test]
    fn index_column_selector_conversion() {
        let timeline = re_log_types::TimelineName::log_time();
        let proto_index_column_selector: crate::common::v1alpha1::IndexColumnSelector =
            crate::common::v1alpha1::IndexColumnSelector {
                timeline: Some(timeline.into()),
            };
        let timeline2: re_log_types::TimelineName = proto_index_column_selector.try_into().unwrap();
        assert_eq!(timeline, timeline2);
    }

    #[test]
    fn application_id_conversion() {
        let application_id = re_log_types::ApplicationId::from("test");
        let proto_application_id: crate::common::v1alpha1::ApplicationId =
            application_id.clone().into();
        let application_id2: re_log_types::ApplicationId = proto_application_id.into();
        assert_eq!(application_id, application_id2);
    }

    #[test]
    fn store_kind_conversion() {
        let store_kind = re_log_types::StoreKind::Recording;
        let proto_store_kind: crate::common::v1alpha1::StoreKind = store_kind.into();
        let store_kind2: re_log_types::StoreKind = proto_store_kind.into();
        assert_eq!(store_kind, store_kind2);
    }

    #[test]
    fn store_id_conversion() {
        let store_id = re_log_types::StoreId::new(
            re_log_types::StoreKind::Recording,
            "test_app_id",
            "test_recording",
        );
        let proto_store_id: crate::common::v1alpha1::StoreId = store_id.clone().into();
        let store_id2: re_log_types::StoreId = proto_store_id.try_into().unwrap();
        assert_eq!(store_id, store_id2);
    }

    #[test]
    fn test_tuid_conversion() {
        let tuid = re_tuid::Tuid::new();
        let proto_tuid: crate::common::v1alpha1::Tuid = tuid.into();
        let tuid2: re_tuid::Tuid = proto_tuid.try_into().unwrap();
        assert_eq!(tuid, tuid2);
    }
}
