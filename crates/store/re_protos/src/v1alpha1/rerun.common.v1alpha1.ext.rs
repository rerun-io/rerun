use std::hash::Hasher;
use std::sync::Arc;

use arrow::{datatypes::Schema as ArrowSchema, error::ArrowError};

use prost::bytes::Bytes;
use re_log_types::{StoreKind, TableId, external::re_types_core::ComponentDescriptor};

use crate::v1alpha1::rerun_common_v1alpha1::TaskId;
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
            re_sorbet::schema_from_ipc(schema_bytes)?.as_ref(),
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

// --- PartitionId ---

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct PartitionId {
    pub id: String,
}

impl PartitionId {
    #[inline]
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl From<String> for PartitionId {
    fn from(id: String) -> Self {
        Self { id }
    }
}

impl From<&str> for PartitionId {
    fn from(id: &str) -> Self {
        Self { id: id.to_owned() }
    }
}

impl std::fmt::Display for PartitionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl TryFrom<crate::common::v1alpha1::PartitionId> for PartitionId {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::PartitionId) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value
                .id
                .ok_or(missing_field!(crate::common::v1alpha1::PartitionId, "id"))?,
        })
    }
}

impl From<PartitionId> for crate::common::v1alpha1::PartitionId {
    fn from(value: PartitionId) -> Self {
        Self { id: Some(value.id) }
    }
}

// shortcuts

impl From<String> for crate::common::v1alpha1::PartitionId {
    fn from(id: String) -> Self {
        Self { id: Some(id) }
    }
}

impl From<&str> for crate::common::v1alpha1::PartitionId {
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

impl From<re_log_types::ResolvedTimeRange> for crate::common::v1alpha1::TimeRange {
    fn from(value: re_log_types::ResolvedTimeRange) -> Self {
        Self {
            start: value.min().as_i64(),
            end: value.max().as_i64(),
        }
    }
}

impl From<crate::common::v1alpha1::TimeRange> for re_log_types::ResolvedTimeRange {
    fn from(value: crate::common::v1alpha1::TimeRange) -> Self {
        Self::new(
            re_log_types::TimeInt::new_temporal(value.start),
            re_log_types::TimeInt::new_temporal(value.end),
        )
    }
}

impl From<re_log_types::ResolvedTimeRange> for crate::common::v1alpha1::IndexRange {
    fn from(value: re_log_types::ResolvedTimeRange) -> Self {
        Self {
            time_range: Some(value.into()),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::IndexRange> for re_log_types::ResolvedTimeRange {
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
        Self(value.id)
    }
}

impl From<re_log_types::ApplicationId> for crate::common::v1alpha1::ApplicationId {
    #[inline]
    fn from(value: re_log_types::ApplicationId) -> Self {
        Self { id: value.0 }
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

impl From<crate::common::v1alpha1::StoreId> for re_log_types::StoreId {
    #[inline]
    fn from(value: crate::common::v1alpha1::StoreId) -> Self {
        Self {
            kind: value.kind().into(),
            id: Arc::new(value.id),
        }
    }
}

impl From<re_log_types::StoreId> for crate::common::v1alpha1::StoreId {
    #[inline]
    fn from(value: re_log_types::StoreId) -> Self {
        let kind: crate::common::v1alpha1::StoreKind = value.kind.into();
        Self {
            kind: kind as i32,
            id: String::clone(&*value.id),
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

impl From<re_log_types::StoreSource> for crate::log_msg::v1alpha1::StoreSource {
    #[inline]
    fn from(value: re_log_types::StoreSource) -> Self {
        use crate::external::prost::Message as _;

        let (kind, payload) = match value {
            re_log_types::StoreSource::Unknown => (
                crate::log_msg::v1alpha1::StoreSourceKind::Unspecified as i32,
                Bytes::new(),
            ),
            re_log_types::StoreSource::CSdk => (
                crate::log_msg::v1alpha1::StoreSourceKind::CSdk as i32,
                Bytes::new(),
            ),
            re_log_types::StoreSource::PythonSdk(python_version) => (
                crate::log_msg::v1alpha1::StoreSourceKind::PythonSdk as i32,
                crate::log_msg::v1alpha1::PythonVersion::from(python_version)
                    .encode_to_vec()
                    .into(),
            ),
            re_log_types::StoreSource::RustSdk {
                rustc_version,
                llvm_version,
            } => (
                crate::log_msg::v1alpha1::StoreSourceKind::RustSdk as i32,
                crate::log_msg::v1alpha1::CrateInfo {
                    rustc_version,
                    llvm_version,
                }
                .encode_to_vec()
                .into(),
            ),
            re_log_types::StoreSource::File { file_source } => (
                crate::log_msg::v1alpha1::StoreSourceKind::File as i32,
                crate::log_msg::v1alpha1::FileSource::from(file_source)
                    .encode_to_vec()
                    .into(),
            ),
            re_log_types::StoreSource::Viewer => (
                crate::log_msg::v1alpha1::StoreSourceKind::Viewer as i32,
                Bytes::new(),
            ),
            re_log_types::StoreSource::Other(description) => (
                crate::log_msg::v1alpha1::StoreSourceKind::Other as i32,
                description.into_bytes().into(),
            ),
        };

        Self {
            kind,
            extra: Some(crate::log_msg::v1alpha1::StoreSourceExtra { payload }),
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::StoreSource> for re_log_types::StoreSource {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::StoreSource) -> Result<Self, Self::Error> {
        use crate::external::prost::Message as _;
        use crate::log_msg::v1alpha1::StoreSourceKind;

        match value.kind() {
            StoreSourceKind::Unspecified => Ok(Self::Unknown),
            StoreSourceKind::CSdk => Ok(Self::CSdk),
            StoreSourceKind::PythonSdk => {
                let extra = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let python_version =
                    crate::log_msg::v1alpha1::PythonVersion::decode(&mut &extra.payload[..])?;
                Ok(Self::PythonSdk(re_log_types::PythonVersion::try_from(
                    python_version,
                )?))
            }
            StoreSourceKind::RustSdk => {
                let extra = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let crate_info =
                    crate::log_msg::v1alpha1::CrateInfo::decode(&mut &extra.payload[..])?;
                Ok(Self::RustSdk {
                    rustc_version: crate_info.rustc_version,
                    llvm_version: crate_info.llvm_version,
                })
            }
            StoreSourceKind::File => {
                let extra = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let file_source =
                    crate::log_msg::v1alpha1::FileSource::decode(&mut &extra.payload[..])?;
                Ok(Self::File {
                    file_source: re_log_types::FileSource::try_from(file_source)?,
                })
            }
            StoreSourceKind::Viewer => Ok(Self::Viewer),
            StoreSourceKind::Other => {
                let description = value.extra.ok_or(missing_field!(
                    crate::log_msg::v1alpha1::StoreSource,
                    "extra"
                ))?;
                let description =
                    String::from_utf8(description.payload.to_vec()).map_err(|err| {
                        invalid_field!(crate::log_msg::v1alpha1::StoreSource, "extra", err)
                    })?;
                Ok(Self::Other(description))
            }
        }
    }
}

impl From<re_log_types::PythonVersion> for crate::log_msg::v1alpha1::PythonVersion {
    #[inline]
    fn from(value: re_log_types::PythonVersion) -> Self {
        Self {
            major: value.major as i32,
            minor: value.minor as i32,
            patch: value.patch as i32,
            suffix: value.suffix,
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::PythonVersion> for re_log_types::PythonVersion {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::PythonVersion) -> Result<Self, Self::Error> {
        Ok(Self {
            major: value.major as u8,
            minor: value.minor as u8,
            patch: value.patch as u8,
            suffix: value.suffix,
        })
    }
}

impl From<re_log_types::FileSource> for crate::log_msg::v1alpha1::FileSource {
    #[inline]
    fn from(value: re_log_types::FileSource) -> Self {
        let kind = match value {
            re_log_types::FileSource::Cli => crate::log_msg::v1alpha1::FileSourceKind::Cli as i32,
            re_log_types::FileSource::Uri => crate::log_msg::v1alpha1::FileSourceKind::Uri as i32,
            re_log_types::FileSource::DragAndDrop { .. } => {
                crate::log_msg::v1alpha1::FileSourceKind::DragAndDrop as i32
            }
            re_log_types::FileSource::FileDialog { .. } => {
                crate::log_msg::v1alpha1::FileSourceKind::FileDialog as i32
            }
            re_log_types::FileSource::Sdk => crate::log_msg::v1alpha1::FileSourceKind::Sdk as i32,
        };

        Self { kind }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::FileSource> for re_log_types::FileSource {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::FileSource) -> Result<Self, Self::Error> {
        use crate::log_msg::v1alpha1::FileSourceKind;

        match value.kind() {
            FileSourceKind::Cli => Ok(Self::Cli),
            FileSourceKind::Uri => Ok(Self::Uri),
            FileSourceKind::DragAndDrop => Ok(Self::DragAndDrop {
                recommended_application_id: None,
                recommended_recording_id: None,
                force_store_info: false,
            }),
            FileSourceKind::FileDialog => Ok(Self::FileDialog {
                recommended_application_id: None,
                recommended_recording_id: None,
                force_store_info: false,
            }),
            FileSourceKind::Sdk => Ok(Self::Sdk),
            FileSourceKind::Unspecified => Err(invalid_field!(
                crate::log_msg::v1alpha1::FileSource,
                "kind",
                "unknown kind",
            )),
        }
    }
}

impl From<re_log_types::StoreInfo> for crate::log_msg::v1alpha1::StoreInfo {
    #[inline]
    fn from(value: re_log_types::StoreInfo) -> Self {
        Self {
            application_id: Some(value.application_id.into()),
            store_id: Some(value.store_id.into()),
            store_source: Some(value.store_source.into()),
            store_version: value
                .store_version
                .map(|v| crate::log_msg::v1alpha1::StoreVersion {
                    crate_version_bits: i32::from_le_bytes(v.to_bytes()),
                }),
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::StoreInfo> for re_log_types::StoreInfo {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::StoreInfo) -> Result<Self, Self::Error> {
        let application_id: re_log_types::ApplicationId = value
            .application_id
            .ok_or(missing_field!(
                crate::log_msg::v1alpha1::StoreInfo,
                "application_id",
            ))?
            .into();
        let store_id: re_log_types::StoreId = value
            .store_id
            .ok_or(missing_field!(
                crate::log_msg::v1alpha1::StoreInfo,
                "store_id",
            ))?
            .into();
        let store_source: re_log_types::StoreSource = value
            .store_source
            .ok_or(missing_field!(
                crate::log_msg::v1alpha1::StoreInfo,
                "store_source",
            ))?
            .try_into()?;
        let store_version = value
            .store_version
            .map(|v| re_build_info::CrateVersion::from_bytes(v.crate_version_bits.to_le_bytes()));

        Ok(Self {
            application_id,
            store_id,
            cloned_from: None,
            store_source,
            store_version,
        })
    }
}

impl From<re_log_types::SetStoreInfo> for crate::log_msg::v1alpha1::SetStoreInfo {
    #[inline]
    fn from(value: re_log_types::SetStoreInfo) -> Self {
        Self {
            row_id: Some(value.row_id.into()),
            info: Some(value.info.into()),
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::SetStoreInfo> for re_log_types::SetStoreInfo {
    type Error = TypeConversionError;

    #[inline]
    fn try_from(value: crate::log_msg::v1alpha1::SetStoreInfo) -> Result<Self, Self::Error> {
        Ok(Self {
            row_id: value
                .row_id
                .ok_or(missing_field!(
                    crate::log_msg::v1alpha1::SetStoreInfo,
                    "row_id",
                ))?
                .try_into()?,
            info: value
                .info
                .ok_or(missing_field!(
                    crate::log_msg::v1alpha1::SetStoreInfo,
                    "info"
                ))?
                .try_into()?,
        })
    }
}

impl From<re_log_types::BlueprintActivationCommand>
    for crate::log_msg::v1alpha1::BlueprintActivationCommand
{
    #[inline]
    fn from(value: re_log_types::BlueprintActivationCommand) -> Self {
        Self {
            blueprint_id: Some(value.blueprint_id.into()),
            make_active: value.make_active,
            make_default: value.make_default,
        }
    }
}

impl TryFrom<crate::log_msg::v1alpha1::BlueprintActivationCommand>
    for re_log_types::BlueprintActivationCommand
{
    type Error = TypeConversionError;

    #[inline]
    fn try_from(
        value: crate::log_msg::v1alpha1::BlueprintActivationCommand,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            blueprint_id: value
                .blueprint_id
                .ok_or(missing_field!(
                    crate::log_msg::v1alpha1::BlueprintActivationCommand,
                    "blueprint_id",
                ))?
                .into(),
            make_active: value.make_active,
            make_default: value.make_default,
        })
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
            archetype_name: value.archetype_name.map(|n| n.full_name().to_owned()),
            archetype_field_name: Some(value.archetype_field_name.to_string()),
            component_type: value.component_type.map(|c| c.full_name().to_owned()),
        }
    }
}

impl TryFrom<crate::common::v1alpha1::ComponentDescriptor> for ComponentDescriptor {
    type Error = TypeConversionError;

    fn try_from(value: crate::common::v1alpha1::ComponentDescriptor) -> Result<Self, Self::Error> {
        let crate::common::v1alpha1::ComponentDescriptor {
            archetype_name,
            archetype_field_name,
            component_type,
        } = value;

        let mut descriptor = Self::partial(archetype_field_name.ok_or(missing_field!(
            crate::common::v1alpha1::ComponentDescriptor,
            "archetype_field_name"
        ))?);

        if let Some(archetype_name) = archetype_name {
            descriptor = descriptor.with_archetype_name(archetype_name.into());
        }

        if let Some(component_type) = component_type {
            descriptor = descriptor.with_component_type(component_type.into());
        }

        Ok(descriptor)
    }
}

// ---

impl Eq for TaskId {}

impl std::hash::Hash for TaskId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.as_str().hash(state)
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
        let time_range = re_log_types::ResolvedTimeRange::new(
            re_log_types::TimeInt::new_temporal(123456789),
            re_log_types::TimeInt::new_temporal(987654321),
        );
        let proto_time_range: crate::common::v1alpha1::TimeRange = time_range.into();
        let time_range2: re_log_types::ResolvedTimeRange = proto_time_range.into();
        assert_eq!(time_range, time_range2);
    }

    #[test]
    fn index_range_conversion() {
        let time_range = re_log_types::ResolvedTimeRange::new(
            re_log_types::TimeInt::new_temporal(123456789),
            re_log_types::TimeInt::new_temporal(987654321),
        );
        let proto_index_range: crate::common::v1alpha1::IndexRange = time_range.into();
        let time_range2: re_log_types::ResolvedTimeRange = proto_index_range.try_into().unwrap();
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
        let application_id = re_log_types::ApplicationId("test".to_owned());
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
        let store_id = re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            "test_recording".to_owned(),
        );
        let proto_store_id: crate::common::v1alpha1::StoreId = store_id.clone().into();
        let store_id2: re_log_types::StoreId = proto_store_id.into();
        assert_eq!(store_id, store_id2);
    }

    #[test]
    fn store_source_conversion() {
        let store_source = re_log_types::StoreSource::PythonSdk(re_log_types::PythonVersion {
            major: 3,
            minor: 8,
            patch: 0,
            suffix: "a".to_owned(),
        });
        let proto_store_source: crate::log_msg::v1alpha1::StoreSource = store_source.clone().into();
        let store_source2: re_log_types::StoreSource = proto_store_source.try_into().unwrap();
        assert_eq!(store_source, store_source2);
    }

    #[test]
    fn file_source_conversion() {
        let file_source = re_log_types::FileSource::Uri;
        let proto_file_source: crate::log_msg::v1alpha1::FileSource = file_source.clone().into();
        let file_source2: re_log_types::FileSource = proto_file_source.try_into().unwrap();
        assert_eq!(file_source, file_source2);
    }

    #[test]
    fn store_info_conversion() {
        let store_info = re_log_types::StoreInfo {
            application_id: re_log_types::ApplicationId("test".to_owned()),
            store_id: re_log_types::StoreId::from_string(
                re_log_types::StoreKind::Recording,
                "test_recording".to_owned(),
            ),
            cloned_from: None,
            store_source: re_log_types::StoreSource::PythonSdk(re_log_types::PythonVersion {
                major: 3,
                minor: 8,
                patch: 0,
                suffix: "a".to_owned(),
            }),
            store_version: None,
        };
        let proto_store_info: crate::log_msg::v1alpha1::StoreInfo = store_info.clone().into();
        let store_info2: re_log_types::StoreInfo = proto_store_info.try_into().unwrap();
        assert_eq!(store_info, store_info2);
    }

    #[test]
    fn set_store_info_conversion() {
        let set_store_info = re_log_types::SetStoreInfo {
            row_id: re_tuid::Tuid::new(),
            info: re_log_types::StoreInfo {
                application_id: re_log_types::ApplicationId("test".to_owned()),
                store_id: re_log_types::StoreId::from_string(
                    re_log_types::StoreKind::Recording,
                    "test_recording".to_owned(),
                ),
                cloned_from: None,
                store_source: re_log_types::StoreSource::PythonSdk(re_log_types::PythonVersion {
                    major: 3,
                    minor: 8,
                    patch: 0,
                    suffix: "a".to_owned(),
                }),
                store_version: None,
            },
        };
        let proto_set_store_info: crate::log_msg::v1alpha1::SetStoreInfo =
            set_store_info.clone().into();
        let set_store_info2: re_log_types::SetStoreInfo = proto_set_store_info.try_into().unwrap();
        assert_eq!(set_store_info, set_store_info2);
    }

    #[test]
    fn blueprint_activation_command_conversion() {
        let blueprint_activation_command = re_log_types::BlueprintActivationCommand {
            blueprint_id: re_log_types::StoreId::from_string(
                re_log_types::StoreKind::Blueprint,
                "test".to_owned(),
            ),
            make_active: true,
            make_default: false,
        };
        let proto_blueprint_activation_command: crate::log_msg::v1alpha1::BlueprintActivationCommand =
            blueprint_activation_command.clone().into();
        let blueprint_activation_command2: re_log_types::BlueprintActivationCommand =
            proto_blueprint_activation_command.try_into().unwrap();
        assert_eq!(blueprint_activation_command, blueprint_activation_command2);
    }

    #[test]
    fn test_tuid_conversion() {
        let tuid = re_tuid::Tuid::new();
        let proto_tuid: crate::common::v1alpha1::Tuid = tuid.into();
        let tuid2: re_tuid::Tuid = proto_tuid.try_into().unwrap();
        assert_eq!(tuid, tuid2);
    }
}
